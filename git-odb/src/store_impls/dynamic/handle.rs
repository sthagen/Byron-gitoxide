use std::{
    cell::RefCell,
    ops::Deref,
    sync::{atomic::Ordering, Arc},
};

use git_features::threading::OwnShared;
use git_hash::oid;

use crate::store::{handle, types, RefreshMode};

pub(crate) mod multi_index {
    // TODO: remove this declaration and replace it with the actual type where it's used
    pub type File = git_pack::multi_index::File;
}

pub enum SingleOrMultiIndex {
    Single {
        index: Arc<git_pack::index::File>,
        data: Option<Arc<git_pack::data::File>>,
    },
    Multi {
        index: Arc<multi_index::File>,
        data: Vec<Option<Arc<git_pack::data::File>>>,
    },
}

/// A utility to allow looking up pack offsets for a particular pack
pub enum IntraPackLookup<'a> {
    Single(&'a git_pack::index::File),
    /// the internal pack-id inside of a multi-index for which the lookup is supposed to be.
    /// Used to prevent ref-delta OIDs to, for some reason, point to a different pack.
    Multi {
        index: &'a git_pack::multi_index::File,
        required_pack_index: git_pack::multi_index::PackIndex,
    },
}

impl<'a> IntraPackLookup<'a> {
    pub(crate) fn pack_offset_by_id(&self, id: &oid) -> Option<git_pack::data::Offset> {
        match self {
            IntraPackLookup::Single(index) => index
                .lookup(id)
                .map(|entry_index| index.pack_offset_at_index(entry_index)),
            IntraPackLookup::Multi {
                index,
                required_pack_index,
            } => index.lookup(id).and_then(|entry_index| {
                let (pack_index, pack_offset) = index.pack_id_and_pack_offset_at_index(entry_index);
                (pack_index == *required_pack_index).then(|| pack_offset)
            }),
        }
    }
}

pub struct IndexLookup {
    pub(crate) file: SingleOrMultiIndex,
    /// The index we were found at in the slot map
    pub(crate) id: types::IndexId,
}

pub struct IndexForObjectInPack {
    /// The internal identifier of the pack itself, which either is referred to by an index or a multi-pack index.
    pub(crate) pack_id: types::PackId,
    /// The offset at which the object's entry can be found
    pub(crate) pack_offset: u64,
}

pub(crate) mod index_lookup {
    use std::sync::Arc;

    use git_hash::oid;

    use crate::store::{handle, handle::IntraPackLookup, types};

    pub(crate) struct Outcome<'a> {
        pub object_index: handle::IndexForObjectInPack,
        pub index_file: IntraPackLookup<'a>,
        pub pack: &'a mut Option<Arc<git_pack::data::File>>,
    }

    impl handle::IndexLookup {
        /// Return an iterator over the entries of the given pack. The `pack_id` is only required to
        pub(crate) fn iter(
            &self,
            pack_id: types::PackId,
        ) -> Option<Box<dyn Iterator<Item = git_pack::index::Entry> + '_>> {
            (self.id == pack_id.index).then(|| match &self.file {
                handle::SingleOrMultiIndex::Single { index, .. } => index.iter(),
                handle::SingleOrMultiIndex::Multi { index, .. } => {
                    let pack_index = pack_id.multipack_index.expect(
                        "BUG: multi-pack index must be set if this is a multi-pack, pack-indices seem unstable",
                    );
                    Box::new(index.iter().filter_map(move |e| {
                        (e.pack_index == pack_index as u32).then(|| git_pack::index::Entry {
                            oid: e.oid,
                            pack_offset: e.pack_offset,
                            crc32: None,
                        })
                    }))
                }
            })
        }

        pub(crate) fn pack(&mut self, pack_id: types::PackId) -> Option<&'_ mut Option<Arc<git_pack::data::File>>> {
            (self.id == pack_id.index).then(move || match &mut self.file {
                handle::SingleOrMultiIndex::Single { data, .. } => data,
                handle::SingleOrMultiIndex::Multi { data, .. } => {
                    let pack_index = pack_id.multipack_index.expect(
                        "BUG: multi-pack index must be set if this is a multi-pack, pack-indices seem unstable",
                    );
                    &mut data[pack_index as usize]
                }
            })
        }

        /// Return true if the given object id exists in this index
        pub(crate) fn contains(&self, object_id: &oid) -> bool {
            match &self.file {
                handle::SingleOrMultiIndex::Single { index, .. } => index.lookup(object_id).is_some(),
                handle::SingleOrMultiIndex::Multi { index, .. } => index.lookup(object_id).is_some(),
            }
        }

        /// Return true if the given object id exists in this index
        pub(crate) fn oid_at_index(&self, entry_index: u32) -> &git_hash::oid {
            match &self.file {
                handle::SingleOrMultiIndex::Single { index, .. } => index.oid_at_index(entry_index),
                handle::SingleOrMultiIndex::Multi { index, .. } => index.oid_at_index(entry_index),
            }
        }

        /// Return the amount of objects contained in the index, essentially the number of object ids.
        pub(crate) fn num_objects(&self) -> u32 {
            match &self.file {
                handle::SingleOrMultiIndex::Single { index, .. } => index.num_objects(),
                handle::SingleOrMultiIndex::Multi { index, .. } => index.num_objects(),
            }
        }

        /// See if the oid is contained in this index, and return its full id for lookup possibly alongside its data file if already
        /// loaded.
        /// Also return the index itself as it's needed to resolve intra-pack ref-delta objects. They are a possibility even though
        /// they won't be used in practice as it's more efficient to store their offsets.
        /// If it is not loaded, ask it to be loaded and put it into the returned mutable option for safe-keeping.
        pub(crate) fn lookup(&mut self, object_id: &oid) -> Option<Outcome<'_>> {
            let id = self.id;
            match &mut self.file {
                handle::SingleOrMultiIndex::Single { index, data } => index.lookup(object_id).map(move |idx| Outcome {
                    object_index: handle::IndexForObjectInPack {
                        pack_id: types::PackId {
                            index: id,
                            multipack_index: None,
                        },
                        pack_offset: index.pack_offset_at_index(idx),
                    },
                    index_file: IntraPackLookup::Single(&**index),
                    pack: data,
                }),
                handle::SingleOrMultiIndex::Multi { index, data } => index.lookup(object_id).map(move |idx| {
                    let (pack_index, pack_offset) = index.pack_id_and_pack_offset_at_index(idx);
                    Outcome {
                        object_index: handle::IndexForObjectInPack {
                            pack_id: types::PackId {
                                index: id,
                                multipack_index: Some(pack_index),
                            },
                            pack_offset,
                        },
                        index_file: IntraPackLookup::Multi {
                            index: &**index,
                            required_pack_index: pack_index,
                        },
                        pack: &mut data[pack_index as usize],
                    }
                }),
            }
        }
    }
}

pub(crate) enum Mode {
    DeletedPacksAreInaccessible,
    /// This mode signals that we should not unload packs even after they went missing.
    KeepDeletedPacksAvailable,
}

/// Handle registration
impl super::Store {
    pub(crate) fn register_handle(&self) -> Mode {
        self.num_handles_unstable.fetch_add(1, Ordering::Relaxed);
        Mode::DeletedPacksAreInaccessible
    }
    pub(crate) fn remove_handle(&self, mode: Mode) {
        match mode {
            Mode::KeepDeletedPacksAvailable => {
                let _lock = self.write.lock();
                self.num_handles_stable.fetch_sub(1, Ordering::SeqCst)
            }
            Mode::DeletedPacksAreInaccessible => self.num_handles_unstable.fetch_sub(1, Ordering::Relaxed),
        };
    }
    pub(crate) fn upgrade_handle(&self, mode: Mode) -> Mode {
        if let Mode::DeletedPacksAreInaccessible = mode {
            let _lock = self.write.lock();
            self.num_handles_stable.fetch_add(1, Ordering::SeqCst);
            self.num_handles_unstable.fetch_sub(1, Ordering::SeqCst);
        }
        Mode::KeepDeletedPacksAvailable
    }
}

/// Handle creation
impl super::Store {
    /// Create a new cache filled with a handle to this store, if this store is supporting shared ownership.
    ///
    /// Note that the actual type of `OwnShared` depends on the `parallel` feature toggle of the `git-features` crate.
    pub fn to_cache(self: &OwnShared<Self>) -> crate::Cache<super::Handle<OwnShared<super::Store>>> {
        self.to_handle().into()
    }

    /// Create a new cache filled with a handle to this store if this store is held in an `Arc`.
    pub fn to_cache_arc(self: &Arc<Self>) -> crate::Cache<super::Handle<Arc<super::Store>>> {
        self.to_handle_arc().into()
    }

    /// Create a new database handle to this store if this store is supporting shared ownership.
    ///
    /// See also, [`to_cache()`][super::Store::to_cache()] which is probably more useful.
    pub fn to_handle(self: &OwnShared<Self>) -> super::Handle<OwnShared<super::Store>> {
        let token = self.register_handle();
        super::Handle {
            store: self.clone(),
            refresh_mode: RefreshMode::default(),
            token: Some(token),
            snapshot: RefCell::new(self.collect_snapshot()),
        }
    }

    /// Create a new database handle to this store if this store is held in an `Arc`.
    ///
    /// This method is useful in applications that know they will use threads.
    pub fn to_handle_arc(self: &Arc<Self>) -> super::Handle<Arc<super::Store>> {
        let token = self.register_handle();
        super::Handle {
            store: self.clone(),
            refresh_mode: Default::default(),
            token: Some(token),
            snapshot: RefCell::new(self.collect_snapshot()),
        }
    }

    /// Transform the only instance into an `Arc<Self>` or panic if this is not the only Rc handle
    /// to the contained store.
    ///
    /// This is meant to be used when the `git_features::threading::OwnShared` refers to an `Rc` as it was compiled without the
    /// `parallel` feature toggle.
    pub fn into_shared_arc(self: OwnShared<Self>) -> Arc<Self> {
        match OwnShared::try_unwrap(self) {
            Ok(this) => Arc::new(this),
            Err(_) => panic!("BUG: Must be called when there is only one owner for this RC"),
        }
    }
}

impl<S> super::Handle<S>
where
    S: Deref<Target = super::Store> + Clone,
{
    /// Call once if pack ids are stored and later used for lookup, meaning they should always remain mapped and not be unloaded
    /// even if they disappear from disk.
    /// This must be called if there is a chance that git maintenance is happening while a pack is created.
    pub fn prevent_pack_unload(&mut self) {
        self.token = self.token.take().map(|token| self.store.upgrade_handle(token));
    }

    /// Return a shared reference to the contained store.
    pub fn store_ref(&self) -> &S::Target {
        &*self.store
    }

    /// Return an owned store with shared ownership.
    pub fn store(&self) -> S {
        self.store.clone()
    }

    /// Set the handle to never cause ODB refreshes if an object could not be found.
    ///
    /// The latter is the default, as typically all objects referenced in a git repository are contained in the local clone.
    /// More recently, however, this doesn't always have to be the case due to sparse checkouts and other ways to only have a
    /// limited amount of objects available locally.
    pub fn refresh_never(&mut self) {
        self.refresh_mode = RefreshMode::Never;
    }

    /// Return the current refresh mode.
    pub fn refresh_mode(&mut self) -> RefreshMode {
        self.refresh_mode
    }
}

impl<S> Drop for super::Handle<S>
where
    S: Deref<Target = super::Store> + Clone,
{
    fn drop(&mut self) {
        if let Some(token) = self.token.take() {
            self.store.remove_handle(token)
        }
    }
}

impl<S> Clone for super::Handle<S>
where
    S: Deref<Target = super::Store> + Clone,
{
    fn clone(&self) -> Self {
        super::Handle {
            store: self.store.clone(),
            refresh_mode: self.refresh_mode,
            token: {
                let token = self.store.register_handle();
                match self.token.as_ref().expect("token is always set here ") {
                    handle::Mode::DeletedPacksAreInaccessible => token,
                    handle::Mode::KeepDeletedPacksAvailable => self.store.upgrade_handle(token),
                }
                .into()
            },
            snapshot: RefCell::new(self.store.collect_snapshot()),
        }
    }
}
