use std::{
    collections::{BTreeMap, VecDeque},
    ffi::OsStr,
    ops::Deref,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU16, AtomicUsize, Ordering},
        Arc,
    },
    time::SystemTime,
};

use crate::store::{handle, types, RefreshMode};

pub(crate) struct Snapshot {
    /// Indices ready for object lookup or contains checks, ordered usually by modification data, recent ones first.
    pub(crate) indices: Vec<handle::IndexLookup>,
    /// A set of loose objects dbs to search once packed objects weren't found.
    pub(crate) loose_dbs: Arc<Vec<crate::loose::Store>>,
    /// remember what this state represents and to compare to other states.
    pub(crate) marker: types::SlotIndexMarker,
}

mod error {
    use std::path::PathBuf;

    /// Returned by [`crate::at_opts()`]
    #[derive(thiserror::Error, Debug)]
    #[allow(missing_docs)]
    pub enum Error {
        #[error("The objects directory at '{0}' is not an accessible directory")]
        Inaccessible(PathBuf),
        #[error(transparent)]
        Io(#[from] std::io::Error),
        #[error(transparent)]
        Alternate(#[from] crate::alternate::Error),
        #[error("The slotmap turned out to be too small with {} entries, would need {} more", .current, .needed)]
        InsufficientSlots { current: usize, needed: usize },
        /// The problem here is that some logic assumes that more recent generations are higher than previous ones. If we would overflow,
        /// we would break that invariant which can lead to the wrong object from being returned. It would probably be super rare, but…
        /// let's not risk it.
        #[error(
            "Would have overflown amount of max possible generations of {}",
            super::Generation::MAX
        )]
        GenerationOverflow,
    }
}

pub use error::Error;

use crate::store::types::{Generation, IndexAndPacks, MutableIndexAndPack, SlotMapIndex};

impl super::Store {
    /// If `None` is returned, there is new indices and the caller should give up. This is a possibility even if it's allowed to refresh
    /// as here might be no change to pick up.
    pub(crate) fn load_one_index(
        &self,
        refresh_mode: RefreshMode,
        marker: types::SlotIndexMarker,
    ) -> Result<Option<Snapshot>, Error> {
        let index = self.index.load();
        if !index.is_initialized() {
            return self.consolidate_with_disk_state(false /*load one new index*/);
        }

        if marker.generation != index.generation || marker.state_id != index.state_id() {
            // We have a more recent state already, provide it.
            Ok(Some(self.collect_snapshot()))
        } else {
            // always compare to the latest state
            // Nothing changed in the mean time, try to load another index…
            if self.load_next_index(index) {
                Ok(Some(self.collect_snapshot()))
            } else {
                // …and if that didn't yield anything new consider refreshing our disk state.
                match refresh_mode {
                    RefreshMode::Never => Ok(None),
                    RefreshMode::AfterAllIndicesLoaded => {
                        self.consolidate_with_disk_state(true /*load one new index*/)
                    }
                }
            }
        }
    }

    /// load a new index (if not yet loaded), and return true if one was indeed loaded (leading to a state_id() change) of the current index.
    /// Note that interacting with the slot-map is inherently racy and we have to deal with it, being conservative in what we even try to load
    /// as our index might already be out-of-date as we try to use it to learn what's next.
    fn load_next_index(&self, mut index: arc_swap::Guard<Arc<SlotMapIndex>>) -> bool {
        'retry_with_changed_index: loop {
            let previous_state_id = index.state_id();
            'retry_with_next_slot_index: loop {
                match index
                    .next_index_to_load
                    .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
                        (current != index.slot_indices.len()).then(|| current + 1)
                    }) {
                    Ok(slot_map_index) => {
                        // This slot-map index is in bounds and was only given to us.
                        let _ongoing_operation = IncOnNewAndDecOnDrop::new(&index.num_indices_currently_being_loaded);
                        let slot = &self.files[index.slot_indices[slot_map_index]];
                        let _lock = slot.write.lock();
                        if slot.generation.load(Ordering::SeqCst) > index.generation {
                            // There is a disk consolidation in progress which just overwrote a slot that cold be disposed with some other
                            // index, one we didn't intend to load.
                            // Continue with the next slot index in the hope there is something else we can do…
                            continue 'retry_with_next_slot_index;
                        }
                        let mut bundle = slot.files.load_full();
                        let bundle_mut = Arc::make_mut(&mut bundle);
                        if let Some(files) = bundle_mut.as_mut() {
                            // these are always expected to be set, unless somebody raced us. We handle this later by retrying.
                            let _loaded_count = IncOnDrop(&index.loaded_indices);
                            match files.load_index() {
                                Ok(_) => {
                                    slot.files.store(bundle);
                                    break 'retry_with_next_slot_index;
                                }
                                Err(_) => {
                                    slot.files.store(bundle);
                                    continue 'retry_with_next_slot_index;
                                }
                            }
                        }
                    }
                    Err(_nothing_more_to_load) => {
                        // There can be contention as many threads start working at the same time and take all the
                        // slots to load indices for. Some threads might just be left-over and have to wait for something
                        // to change.
                        let num_load_operations = index.num_indices_currently_being_loaded.deref();
                        // TODO: potentially hot loop - could this be a condition variable?
                        while num_load_operations.load(Ordering::Relaxed) != 0 {
                            std::thread::yield_now()
                        }
                        break 'retry_with_next_slot_index;
                    }
                }
            }
            if previous_state_id == index.state_id() {
                let potentially_new_index = self.index.load();
                if Arc::as_ptr(&potentially_new_index) == Arc::as_ptr(&index) {
                    // There isn't a new index with which to retry the whole ordeal, so nothing could be done here.
                    return false;
                } else {
                    // the index changed, worth trying again
                    index = potentially_new_index;
                    continue 'retry_with_changed_index;
                }
            } else {
                // something inarguably changed, probably an index was loaded. 'probably' because we consider failed loads valid attempts,
                // even they don't change anything for the caller which would then do a round for nothing.
                return true;
            }
        }
    }

    /// refresh and possibly clear out our existing data structures, causing all pack ids to be invalidated.
    /// `load_new_index` is an optimization to at least provide one newly loaded pack after refreshing the slot map.
    fn consolidate_with_disk_state(&self, load_new_index: bool) -> Result<Option<Snapshot>, Error> {
        let index = self.index.load();
        let previous_index_state = Arc::as_ptr(&index) as usize;

        // IMPORTANT: get a lock after we recorded the previous state.
        let write = self.write.lock();
        let objects_directory = &self.path;

        // Now we know the index isn't going to change anymore, even though threads might still load indices in the meantime.
        let index = self.index.load();
        if previous_index_state != Arc::as_ptr(&index) as usize {
            // Someone else took the look before and changed the index. Return it without doing any additional work.
            return Ok(Some(self.collect_snapshot()));
        }

        let was_uninitialized = !index.is_initialized();
        self.num_disk_state_consolidation.fetch_add(1, Ordering::Relaxed);

        let db_paths: Vec<_> = std::iter::once(objects_directory.to_owned())
            .chain(crate::alternate::resolve(objects_directory)?)
            .collect();

        // turn db paths into loose object databases. Reuse what's there, but only if it is in the right order.
        let loose_dbs = if was_uninitialized
            || db_paths.len() != index.loose_dbs.len()
            || db_paths
                .iter()
                .zip(index.loose_dbs.iter().map(|ldb| &ldb.path))
                .any(|(lhs, rhs)| lhs != rhs)
        {
            Arc::new(db_paths.iter().map(crate::loose::Store::at).collect::<Vec<_>>())
        } else {
            Arc::clone(&index.loose_dbs)
        };

        let indices_by_modification_time =
            Self::collect_indices_and_mtime_sorted_by_size(db_paths, index.slot_indices.len().into())?;
        let mut idx_by_index_path: BTreeMap<_, _> = index
            .slot_indices
            .iter()
            .filter_map(|&idx| {
                let f = &self.files[idx];
                Option::as_ref(&f.files.load()).map(|f| (f.index_path().to_owned(), idx))
            })
            .collect();

        let mut new_slot_map_indices = Vec::new(); // these indices into the slot map still exist there/didn't change
        let mut index_paths_to_add = was_uninitialized
            .then(|| VecDeque::with_capacity(indices_by_modification_time.len()))
            .unwrap_or_default();

        let mut num_loaded_indices = 0;
        for (index_path, mtime) in indices_by_modification_time.into_iter().map(|(a, b, _)| (a, b)) {
            match idx_by_index_path.remove(&index_path) {
                Some(slot_idx) => {
                    let slot = &self.files[slot_idx];
                    if is_multipack_index(&index_path)
                        && Option::as_ref(&slot.files.load())
                            .map(|b| b.mtime() != mtime)
                            .expect("slot is set or we wouldn't know it points to this file")
                    {
                        // we have a changed multi-pack index. We can't just change the existing slot as it may alter slot indices
                        // that are currently available. Instead we have to move what's there into a new slot, along with the changes,
                        // and later free the slot or dispose of the index in the slot (like we do for removed/missing files).
                        index_paths_to_add.push_back((index_path, mtime, Some(slot_idx)));
                        // If the current slot is loaded, the soon-to-be copied multi-index path will be loaded as well.
                        if Option::as_ref(&slot.files.load())
                            .map(|f| f.index_is_loaded())
                            .expect("slot is set - see above")
                        {
                            num_loaded_indices += 1;
                        }
                    } else {
                        // packs and indices are immutable, so no need to check modification times. Unchanged multi-pack indices also
                        // are handled like this.
                        if Self::assure_slot_matches_index(
                            &write,
                            slot,
                            index_path,
                            mtime,
                            index.generation,
                            false, /*allow init*/
                        ) {
                            num_loaded_indices += 1;
                        }
                        new_slot_map_indices.push(slot_idx);
                    }
                }
                None => index_paths_to_add.push_back((index_path, mtime, None)),
            }
        }
        let needs_stable_indices = self.maintain_stable_indices(&write);

        let mut next_possibly_free_index = index
            .slot_indices
            .iter()
            .max()
            .map(|idx| (idx + 1) % self.files.len())
            .unwrap_or(0);
        let mut num_indices_checked = 0;
        let mut needs_generation_change = false;
        let mut slot_indices_to_remove: Vec<_> = idx_by_index_path.into_values().collect();
        while let Some((index_path, mtime, move_from_slot_idx)) = index_paths_to_add.pop_front() {
            'increment_slot_index: loop {
                if num_indices_checked == self.files.len() {
                    return Err(Error::InsufficientSlots {
                        current: self.files.len(),
                        needed: index_paths_to_add.len() + 1,
                        /*the one currently popped off*/
                    });
                }
                let slot_index = next_possibly_free_index;
                let slot = &self.files[slot_index];
                next_possibly_free_index = (next_possibly_free_index + 1) % self.files.len();
                num_indices_checked += 1;
                match move_from_slot_idx {
                    Some(move_from_slot_idx) => {
                        debug_assert!(is_multipack_index(&index_path), "only set for multi-pack indices");
                        if let Some(dest_was_empty) = self.try_copy_multi_pack_index(
                            &write,
                            move_from_slot_idx,
                            slot,
                            index_path.clone(), // TODO: once this settles, consider to return this path if it does nothing or refactor the whole thing.
                            mtime,
                            index.generation,
                            needs_stable_indices,
                        ) {
                            slot_indices_to_remove.push(move_from_slot_idx);
                            new_slot_map_indices.push(slot_index);
                            // To avoid handling out the wrong pack (due to reassigned pack ids), declare this a new generation.
                            if !dest_was_empty {
                                needs_generation_change = true;
                            }
                            break 'increment_slot_index;
                        }
                    }
                    None => {
                        if let Some(dest_was_empty) = Self::try_set_single_index_slot(
                            &write,
                            slot,
                            index_path.clone(),
                            mtime,
                            index.generation,
                            needs_stable_indices,
                        ) {
                            new_slot_map_indices.push(slot_index);
                            if !dest_was_empty {
                                needs_generation_change = true;
                            }
                            break 'increment_slot_index;
                        }
                    }
                }
                // This isn't racy as it's only us who can change the Option::Some/None state of a slot.
            }
        }
        assert_eq!(
            index_paths_to_add.len(),
            0,
            "By this time we have assigned all new files to slots"
        );

        let generation = if needs_generation_change {
            index.generation.checked_add(1).ok_or(Error::GenerationOverflow)?
        } else {
            index.generation
        };
        let index_unchanged = index.slot_indices == new_slot_map_indices;
        if generation != index.generation {
            assert!(
                !index_unchanged,
                "if the generation changed, the slot index must have changed for sure"
            );
        }
        if !index_unchanged || loose_dbs != index.loose_dbs {
            let new_index = Arc::new(SlotMapIndex {
                slot_indices: new_slot_map_indices,
                loose_dbs,
                generation,
                // if there was a prior generation, some indices might already be loaded. But we deal with it by trying to load the next index then,
                // until we find one.
                next_index_to_load: index_unchanged
                    .then(|| Arc::clone(&index.next_index_to_load))
                    .unwrap_or_default(),
                loaded_indices: index_unchanged
                    .then(|| Arc::clone(&index.loaded_indices))
                    .unwrap_or_else(|| Arc::new(num_loaded_indices.into())),
                num_indices_currently_being_loaded: Default::default(),
            });
            self.index.store(new_index);
        }

        // deleted items - remove their slots AFTER we have set the new index if we may alter indices, otherwise we only declare them garbage.
        // removing slots may cause pack loading to fail, and they will then reload their indices.
        for slot in slot_indices_to_remove.into_iter().map(|idx| &self.files[idx]) {
            let _lock = slot.write.lock();
            let mut files = slot.files.load_full();
            let files_mut = Arc::make_mut(&mut files);
            if needs_stable_indices {
                if let Some(files) = files_mut.as_mut() {
                    files.trash();
                    // generation stays the same, as it's the same value still but scheduled for eventual removal.
                }
            } else {
                *files_mut = None;
                // Not racy due to lock, generation must be set after unsetting the value.
                slot.generation.store(0, Ordering::SeqCst);
            };
            slot.files.store(files);
        }

        let new_index = self.index.load();
        Ok(if index.state_id() == new_index.state_id() {
            // there was no change, and nothing was loaded in the meantime, reflect that in the return value to not get into loops
            None
        } else {
            if load_new_index {
                self.load_next_index(new_index);
            }
            Some(self.collect_snapshot())
        })
    }

    pub(crate) fn collect_indices_and_mtime_sorted_by_size(
        db_paths: Vec<PathBuf>,
        initial_capacity: Option<usize>,
    ) -> Result<Vec<(PathBuf, SystemTime, u64)>, Error> {
        let mut indices_by_modification_time = Vec::with_capacity(initial_capacity.unwrap_or_default());
        for db_path in db_paths {
            let packs = db_path.join("pack");
            let entries = match std::fs::read_dir(&packs) {
                Ok(e) => e,
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
                Err(err) => return Err(err.into()),
            };
            indices_by_modification_time.extend(
                entries
                    .filter_map(Result::ok)
                    .filter_map(|e| e.metadata().map(|md| (e.path(), md)).ok())
                    .filter(|(_, md)| md.file_type().is_file())
                    .filter(|(p, _)| {
                        let ext = p.extension();
                        ext == Some(OsStr::new("idx")) || (ext.is_none() && is_multipack_index(p))
                    })
                    .map(|(p, md)| md.modified().map_err(Error::from).map(|mtime| (p, mtime, md.len())))
                    .collect::<Result<Vec<_>, _>>()?,
            );
        }
        // Unlike libgit2, do not sort by modification date, but by size and put the biggest indices first. That way
        // the chance to hit an object should be higher. We leave it to the handle to sort by LRU.
        // Git itself doesn't change the order which may safe time, but we want it to be stable which also helps some tests.
        indices_by_modification_time.sort_by(|l, r| l.2.cmp(&r.2).reverse());
        Ok(indices_by_modification_time)
    }

    /// Returns Some(true) if the slot was empty, or Some(false) if it was collected, None if it couldn't claim the slot.
    fn try_set_single_index_slot(
        lock: &parking_lot::MutexGuard<'_, ()>,
        slot: &MutableIndexAndPack,
        index_path: PathBuf,
        mtime: SystemTime,
        current_generation: Generation,
        needs_stable_indices: bool,
    ) -> Option<bool> {
        match &**slot.files.load() {
            Some(bundle) => {
                debug_assert!(
                    !is_multipack_index(&index_path),
                    "multi-indices are not handled here, but use their own 'move' logic"
                );
                if !needs_stable_indices && bundle.is_disposable() {
                    assert_ne!(
                        bundle.index_path(),
                        index_path,
                        "BUG: an index of the same path must have been handled already"
                    );
                    // Need to declare this to be the future to avoid anything in that slot to be returned to people who
                    // last saw the old state. They will then try to get a new index which by that time, might be happening
                    // in time so they get the latest one. If not, they will probably get into the same situation again until
                    // it finally succeeds. Alternatively, the object will be reported unobtainable, but at least it won't return
                    // some other object.
                    let next_generation = current_generation + 1;
                    Self::set_slot_to_index(lock, slot, index_path, mtime, next_generation);
                    Some(false)
                } else {
                    // A valid slot, taken by another file, keep looking
                    None
                }
            }
            None => {
                // an entirely unused (or deleted) slot, free to take.
                Self::assure_slot_matches_index(
                    lock,
                    slot,
                    index_path,
                    mtime,
                    current_generation,
                    true, /*may init*/
                );
                Some(true)
            }
        }
    }

    // returns Some<dest slot was empty> if the copy could happen because dest-slot was actually free or disposable , and Some(true) if it was empty
    #[allow(clippy::too_many_arguments, unused_variables)]
    fn try_copy_multi_pack_index(
        &self,
        lock: &parking_lot::MutexGuard<'_, ()>,
        from_slot_idx: usize,
        dest_slot: &MutableIndexAndPack,
        index_path: PathBuf,
        mtime: SystemTime,
        current_generation: Generation,
        needs_stable_indices: bool,
    ) -> Option<bool> {
        match &**dest_slot.files.load() {
            Some(bundle) => {
                if bundle.index_path() == index_path {
                    // it's possible to see ourselves in case all slots are taken, but there are still a few more to look for.
                    // This can only happen for multi-pack indices which are mutable in place.
                    return None;
                }
                todo!("copy to possibly disposable slot")
            }
            None => {
                // Do NOT copy the packs over, they need to be reopened to get the correct pack id matching the new slot map index.
                // If we try are allowed to delete the original, and nobody has the pack referenced, it is closed which is preferred.
                // Thus we simply always start new with packs in multi-pack indices.
                // In the worst case this could mean duplicate file handle usage though as the old and the new index can't share
                // packs due to the intrinsic id.
                // Note that the ID is used for cache access, too, so it must be unique. It must also be mappable from pack-id to slotmap id.
                todo!("copy/clone resources over, but leave the original alone for now")
            }
        }
    }

    fn set_slot_to_index(
        _lock: &parking_lot::MutexGuard<'_, ()>,
        slot: &MutableIndexAndPack,
        index_path: PathBuf,
        mtime: SystemTime,
        generation: Generation,
    ) {
        let _lock = slot.write.lock();
        let mut files = slot.files.load_full();
        let files_mut = Arc::make_mut(&mut files);
        // set the generation before we actually change the value, otherwise readers of old generations could observe the new one.
        // We rather want them to turn around here and update their index, which, by that time, migth actually already be available.
        // If not, they would fail unable to load a pack or index they need, but that's preferred over returning wrong objects.
        slot.generation.store(generation, Ordering::SeqCst);
        *files_mut = Some(IndexAndPacks::new_by_index_path(index_path, mtime));
        slot.files.store(files);
    }

    /// Returns true if the index was loaded.
    fn assure_slot_matches_index(
        _lock: &parking_lot::MutexGuard<'_, ()>,
        slot: &MutableIndexAndPack,
        index_path: PathBuf,
        mtime: SystemTime,
        current_generation: Generation,
        may_init: bool,
    ) -> bool {
        match Option::as_ref(&slot.files.load()) {
            Some(bundle) => {
                assert_eq!(
                    bundle.index_path(),
                    index_path,
                    "Parallel writers cannot change the file the slot points to."
                );
                if bundle.is_disposable() {
                    // put it into the correct mode, it's now available for sure so should not be missing or garbage.
                    // The latter can happen if files are removed and put back for some reason, but we should definitely
                    // have them in a decent state now that we know/think they are there.
                    let _lock = slot.write.lock();
                    let mut files = slot.files.load_full();
                    let files_mut = Arc::make_mut(&mut files);
                    files_mut
                        .as_mut()
                        .expect("BUG: cannot change from something to nothing, would be race")
                        .put_back();
                    // Safety: can't race as we hold the lock, must be set before replacing the data.
                    // NOTE that we don't change the generation as it's still the very same index we talk about, it doesn't change
                    // identity.
                    slot.generation.store(current_generation, Ordering::SeqCst);
                    slot.files.store(files);
                } else {
                    // it's already in the correct state, either loaded or unloaded.
                }
                bundle.index_is_loaded()
            }
            None => {
                if may_init {
                    let _lock = slot.write.lock();
                    let mut files = slot.files.load_full();
                    let files_mut = Arc::make_mut(&mut files);
                    assert!(
                        files_mut.is_none(),
                        "BUG: There must be no race between us checking and obtaining a lock."
                    );
                    *files_mut = IndexAndPacks::new_by_index_path(index_path, mtime).into();
                    // Safety: can't race as we hold the lock.
                    slot.generation.store(current_generation, Ordering::SeqCst);
                    slot.files.store(files);
                    false
                } else {
                    unreachable!("BUG: a slot can never be deleted if we have it recorded in the index WHILE changing said index. There shouldn't be a race")
                }
            }
        }
    }

    /// Stability means that indices returned by this API will remain valid.
    /// Without that constraint, we may unload unused packs and indices, and may rebuild the slotmap index.
    ///
    /// Note that this must be called with a lock to the relevant state held to assure these values don't change while
    /// we are working on said index.
    fn maintain_stable_indices(&self, _guard: &parking_lot::MutexGuard<'_, ()>) -> bool {
        self.num_handles_stable.load(Ordering::SeqCst) > 0
    }

    pub(crate) fn collect_snapshot(&self) -> Snapshot {
        let index = self.index.load();
        let indices = if index.is_initialized() {
            index
                .slot_indices
                .iter()
                .map(|idx| (*idx, &self.files[*idx]))
                .filter_map(|(id, file)| {
                    let lookup = match (&**file.files.load()).as_ref()? {
                        types::IndexAndPacks::Index(bundle) => handle::SingleOrMultiIndex::Single {
                            index: bundle.index.loaded()?.clone(),
                            data: bundle.data.loaded().cloned(),
                        },
                        types::IndexAndPacks::MultiIndex(multi) => handle::SingleOrMultiIndex::Multi {
                            index: multi.multi_index.loaded()?.clone(),
                            data: multi.data.iter().map(|f| f.loaded().cloned()).collect(),
                        },
                    };
                    handle::IndexLookup { file: lookup, id }.into()
                })
                .collect()
        } else {
            Vec::new()
        };

        Snapshot {
            indices,
            loose_dbs: Arc::clone(&index.loose_dbs),
            marker: index.marker(),
        }
    }
}

// Outside of this method we will never assign new slot indices.
fn is_multipack_index(path: &Path) -> bool {
    path.file_name() == Some(OsStr::new("multi-pack-index"))
}

struct IncOnNewAndDecOnDrop<'a>(&'a AtomicU16);
impl<'a> IncOnNewAndDecOnDrop<'a> {
    pub fn new(v: &'a AtomicU16) -> Self {
        v.fetch_add(1, Ordering::SeqCst);
        Self(v)
    }
}
impl<'a> Drop for IncOnNewAndDecOnDrop<'a> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

struct IncOnDrop<'a>(&'a AtomicUsize);
impl<'a> Drop for IncOnDrop<'a> {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
