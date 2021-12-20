#![allow(missing_docs)]
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU16, AtomicU32, AtomicUsize, Ordering},
        Arc,
    },
    time::SystemTime,
};

use arc_swap::ArcSwap;
use git_features::hash;

/// An id to refer to an index file or a multipack index file
pub type IndexId = usize;
pub(crate) type StateId = u32;
pub(crate) type Generation = u32;
pub(crate) type AtomicGeneration = AtomicU32;

/// A way to indicate which pack indices we have seen already and which of them are loaded, along with an idea
/// of whether stored `PackId`s are still usable.
#[derive(Default, Copy, Clone)]
pub struct SlotIndexMarker {
    /// The generation the `loaded_until_index` belongs to. Indices of different generations are completely incompatible.
    /// This value changes once the internal representation is compacted, something that may happen only if there is no handle
    /// requiring stable pack indices.
    pub(crate) generation: Generation,
    /// A unique id identifying the index state as well as all loose databases we have last observed.
    /// If it changes in any way, the value is different.
    pub(crate) state_id: StateId,
}

/// A way to load and refer to a pack uniquely, namespaced by their indexing mechanism, aka multi-pack or not.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct PackId {
    /// This is the index in the slot map at which the packs index is located.
    pub(crate) index: IndexId,
    /// If the pack is in a multi-pack index, this additional index is the pack-index within the multi-pack index identified by `index`.
    pub(crate) multipack_index: Option<IndexId>,
}

impl PackId {
    /// Packs have a built-in identifier to make data structures simpler, and this method represents ourselves as such id
    /// to be convertible back and forth. We essentially compress ourselves into a u32.
    pub(crate) fn to_intrinsic_pack_id(self) -> u32 {
        assert!(self.index < 1 << 15, "There shouldn't be more than 2^15 indices");
        match self.multipack_index {
            None => self.index as u32,
            Some(midx) => {
                assert!(
                    midx < 1 << 16,
                    "There shouldn't be more than 2^16 packs per multi-index"
                );
                ((self.index | 1 << 15) | midx << 16) as u32
            }
        }
    }

    pub(crate) fn from_intrinsic_pack_id(pack_id: u32) -> Self {
        if pack_id & (1 << 15) == 0 {
            PackId {
                index: pack_id as usize,
                multipack_index: None,
            }
        } else {
            PackId {
                index: (pack_id & 0x7fff) as IndexId,
                multipack_index: Some((pack_id >> 16) as IndexId),
            }
        }
    }
}

/// An index that changes only if the packs directory changes and its contents is re-read.
#[derive(Default)]
pub struct SlotMapIndex {
    /// The index into the slot map at which we expect an index or pack file. Neither of these might be loaded yet.
    pub(crate) slot_indices: Vec<usize>,
    /// A list of loose object databases as resolved by their alternates file in the `object_directory`. The first entry is this objects
    /// directory loose file database. All other entries are the loose stores of alternates.
    /// It's in an Arc to be shared to Handles, but not to be shared across SlotMapIndices.
    pub(crate) loose_dbs: Arc<Vec<crate::loose::Store>>,

    /// A static value that doesn't ever change for a particular clone of this index.
    pub(crate) generation: Generation,
    /// The number of indices loaded thus far when the index of the slot map was last examined, which can change as new indices are loaded
    /// in parallel.
    /// Shared across SlotMapIndex instances of the same generation.
    pub(crate) next_index_to_load: Arc<AtomicUsize>,
    /// Incremented by one up to `slot_indices.len()` once an attempt to load an index completed.
    /// If a load failed, there will also be an increment.
    /// Shared across SlotMapIndex instances of the same generation.
    pub(crate) loaded_indices: Arc<AtomicUsize>,
    /// The amount of indices that are currently being loaded.
    /// Zero if no loading operation is currently happening, or more otherwise.
    pub(crate) num_indices_currently_being_loaded: Arc<AtomicU16>,
}

impl SlotMapIndex {
    pub(crate) fn state_id(self: &Arc<SlotMapIndex>) -> StateId {
        // We let the loaded indices take part despite not being part of our own snapshot.
        // This is to account for indices being loaded in parallel without actually changing the snapshot itself.
        let hash = hash::crc32(&(Arc::as_ptr(self) as usize).to_be_bytes());
        hash::crc32_update(hash, &self.loaded_indices.load(Ordering::SeqCst).to_be_bytes())
    }

    pub(crate) fn marker(self: &Arc<SlotMapIndex>) -> SlotIndexMarker {
        SlotIndexMarker {
            generation: self.generation,
            state_id: self.state_id(),
        }
    }

    /// Returns true if we already know at least one loose object db, a sign of being initialized
    pub(crate) fn is_initialized(&self) -> bool {
        !self.loose_dbs.is_empty()
    }
}

#[derive(Clone)]
pub(crate) struct OnDiskFile<T: Clone> {
    /// The last known path of the file
    path: Arc<PathBuf>,
    /// the time the file was last modified
    mtime: SystemTime,
    state: OnDiskFileState<T>,
}

#[derive(Clone)]
pub(crate) enum OnDiskFileState<T: Clone> {
    /// The file is on disk and can be loaded from there.
    Unloaded,
    Loaded(T),
    /// The file was loaded, but appeared to be missing on disk after reconciling our state with what's on disk.
    /// As there were handles that required pack-id stability we had to keep the item to allow finding it on later
    /// lookups.
    Garbage(T),
    /// File is missing on disk and could not be loaded when we tried or turned missing after reconciling our state.
    Missing,
}

impl<T: Clone> OnDiskFile<T> {
    /// Return true if we hold a memory map of the file already.
    pub fn is_loaded(&self) -> bool {
        matches!(self.state, OnDiskFileState::Loaded(_) | OnDiskFileState::Garbage(_))
    }

    /// Return true if we are to be collected as garbage
    pub fn is_disposable(&self) -> bool {
        matches!(self.state, OnDiskFileState::Garbage(_) | OnDiskFileState::Missing)
    }

    // On error, always declare the file missing and return an error.
    pub(crate) fn load_strict(&mut self, load: impl FnOnce(&Path) -> std::io::Result<T>) -> std::io::Result<()> {
        use OnDiskFileState::*;
        match self.state {
            Unloaded | Missing => match load(&self.path) {
                Ok(v) => {
                    self.state = Loaded(v);
                    Ok(())
                }
                Err(err) => {
                    // TODO: Should be provide more information? We don't even know what exactly failed right now, degenerating information.
                    self.state = Missing;
                    Err(err)
                }
            },
            Loaded(_) | Garbage(_) => Ok(()),
        }
    }
    /// If the file is missing, we don't consider this failure but instead return Ok(None) to allow recovery.
    /// when we know that loading is necessary. This also works around borrow check, which is a nice coincidence.
    pub fn load_with_recovery(&mut self, load: impl FnOnce(&Path) -> std::io::Result<T>) -> std::io::Result<Option<T>> {
        use OnDiskFileState::*;
        match &mut self.state {
            Loaded(v) | Garbage(v) => Ok(Some(v.clone())),
            Missing => Ok(None),
            Unloaded => match load(&self.path) {
                Ok(v) => {
                    self.state = OnDiskFileState::Loaded(v.clone());
                    Ok(Some(v))
                }
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                    self.state = OnDiskFileState::Missing;
                    Ok(None)
                }
                Err(err) => Err(err),
            },
        }
    }

    pub fn loaded(&self) -> Option<&T> {
        use OnDiskFileState::*;
        match &self.state {
            Loaded(v) | Garbage(v) => Some(v),
            Unloaded | Missing => None,
        }
    }

    pub fn put_back(&mut self) {
        match std::mem::replace(&mut self.state, OnDiskFileState::Missing) {
            OnDiskFileState::Garbage(v) => self.state = OnDiskFileState::Loaded(v),
            OnDiskFileState::Missing => self.state = OnDiskFileState::Unloaded,
            other @ (OnDiskFileState::Loaded(_) | OnDiskFileState::Unloaded) => self.state = other,
        }
    }

    pub fn trash(&mut self) {
        match std::mem::replace(&mut self.state, OnDiskFileState::Missing) {
            OnDiskFileState::Loaded(v) => self.state = OnDiskFileState::Garbage(v),
            other @ (OnDiskFileState::Garbage(_) | OnDiskFileState::Unloaded | OnDiskFileState::Missing) => {
                self.state = other
            }
        }
    }
}

#[derive(Clone)]
pub(crate) struct IndexFileBundle {
    pub index: OnDiskFile<Arc<git_pack::index::File>>,
    pub data: OnDiskFile<Arc<git_pack::data::File>>,
}

#[derive(Clone)]
pub(crate) struct MultiIndexFileBundle {
    pub multi_index: OnDiskFile<Arc<super::handle::multi_index::File>>,
    pub data: Vec<OnDiskFile<Arc<git_pack::data::File>>>,
}

#[derive(Clone)]
pub(crate) enum IndexAndPacks {
    Index(IndexFileBundle),
    /// Note that there can only be one multi-pack file per repository, but thanks to git alternates, there can be multiple overall.
    MultiIndex(MultiIndexFileBundle),
}

impl IndexAndPacks {
    pub(crate) fn index_path(&self) -> &Path {
        match self {
            IndexAndPacks::Index(index) => &index.index.path,
            IndexAndPacks::MultiIndex(index) => &index.multi_index.path,
        }
    }

    pub(crate) fn mtime(&self) -> SystemTime {
        match self {
            IndexAndPacks::Index(index) => index.index.mtime,
            IndexAndPacks::MultiIndex(index) => index.multi_index.mtime,
        }
    }

    /// If we are garbaged, put ourselve into the loaded state. Otherwise put ourselves back to unloaded.
    pub(crate) fn put_back(&mut self) {
        match self {
            IndexAndPacks::Index(bundle) => {
                bundle.index.put_back();
                bundle.data.put_back();
            }
            IndexAndPacks::MultiIndex(bundle) => {
                bundle.multi_index.put_back();
                for data in &mut bundle.data {
                    data.put_back();
                }
            }
        }
    }

    // The inverse of `put_back()`, by trashing the content.
    pub(crate) fn trash(&mut self) {
        match self {
            IndexAndPacks::Index(bundle) => {
                bundle.index.trash();
                bundle.data.trash();
            }
            IndexAndPacks::MultiIndex(bundle) => {
                bundle.multi_index.trash();
                for data in &mut bundle.data {
                    data.trash();
                }
            }
        }
    }

    pub(crate) fn index_is_loaded(&self) -> bool {
        match self {
            Self::Index(bundle) => bundle.index.is_loaded(),
            Self::MultiIndex(bundle) => bundle.multi_index.is_loaded(),
        }
    }

    pub(crate) fn is_disposable(&self) -> bool {
        match self {
            Self::Index(bundle) => bundle.index.is_disposable() || bundle.data.is_disposable(),
            Self::MultiIndex(bundle) => {
                bundle.multi_index.is_disposable() || bundle.data.iter().any(|odf| odf.is_disposable())
            }
        }
    }

    pub(crate) fn load_index(&mut self) -> std::io::Result<()> {
        match self {
            IndexAndPacks::Index(bundle) => bundle.index.load_strict(|path| {
                git_pack::index::File::at(path).map(Arc::new).map_err(|err| match err {
                    git_pack::index::init::Error::Io { source, .. } => source,
                    err => std::io::Error::new(std::io::ErrorKind::Other, err),
                })
            }),
            IndexAndPacks::MultiIndex(_bundle) => todo!("load multi-index"),
        }
    }

    pub(crate) fn new_by_index_path(index_path: PathBuf, mtime: SystemTime) -> Self {
        if index_path.extension() == Some(OsStr::new("idx")) {
            IndexAndPacks::new_single(index_path, mtime)
        } else {
            IndexAndPacks::new_multi(index_path, mtime)
        }
    }

    fn new_single(index_path: PathBuf, mtime: SystemTime) -> Self {
        let data_path = index_path.with_extension("pack");
        Self::Index(IndexFileBundle {
            index: OnDiskFile {
                path: Arc::new(index_path),
                state: OnDiskFileState::Unloaded,
                mtime,
            },
            data: OnDiskFile {
                path: Arc::new(data_path),
                state: OnDiskFileState::Unloaded,
                mtime,
            },
        })
    }

    #[allow(unused, unreachable_code)]
    fn new_multi(index_path: PathBuf, mtime: SystemTime) -> Self {
        Self::MultiIndex(MultiIndexFileBundle {
            multi_index: OnDiskFile {
                path: Arc::new(index_path),
                state: OnDiskFileState::Unloaded,
                mtime,
            },
            data: todo!(
                "figure we actually have to map it here or find a way to learn about the data files in advance."
            ),
        })
    }
}

#[derive(Default)]
pub(crate) struct MutableIndexAndPack {
    pub(crate) files: ArcSwap<Option<IndexAndPacks>>,
    pub(crate) write: parking_lot::Mutex<()>,
    /// The generation required at least to read this slot. If these mismatch, the caller is likely referring to a now changed slot
    /// that has different content under the same id.
    /// Must only be changed when the write lock is held.
    pub(crate) generation: AtomicGeneration,
}

/// A snapshot about resource usage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Metrics {
    pub num_handles: usize,
    pub num_refreshes: usize,
    pub open_reachable_indices: usize,
    pub known_indices: usize,
    pub open_reachable_packs: usize,
    pub known_packs: usize,
    pub unused_slots: usize,
    pub unreachable_indices: usize,
    pub loose_dbs: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    mod pack_id {
        use super::PackId;

        #[test]
        fn to_intrinsic_roundtrip() {
            let single = PackId {
                index: (1 << 15) - 1,
                multipack_index: None,
            };
            let multi = PackId {
                index: (1 << 15) - 1,
                multipack_index: Some((1 << 16) - 1),
            };
            assert_eq!(PackId::from_intrinsic_pack_id(single.to_intrinsic_pack_id()), single);
            assert_eq!(PackId::from_intrinsic_pack_id(multi.to_intrinsic_pack_id()), multi);
        }

        #[test]
        #[should_panic]
        fn max_supported_index_count() {
            PackId {
                index: 1 << 15,
                multipack_index: None,
            }
            .to_intrinsic_pack_id();
        }
    }
}
