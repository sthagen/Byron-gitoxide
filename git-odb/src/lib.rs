#![deny(missing_docs, unsafe_code, rust_2018_idioms)]

//! Git stores all of its data as _Objects_, which are data along with a hash over all data. Thus it's an
//! object store indexed by the signature of data itself with inherent deduplication: the same data will have the same hash,
//! and thus occupy the same space within the store.
//!
//! There is only one all-round object store, also known as the [`Store`], as it supports ~~everything~~ most of what git has to offer.
//!
//! * loose object reading and writing
//! * access to packed objects
//! * multiple loose objects and pack locations as gathered from `alternates` files.
// TODO: actually remove the deprecated items and remove thos allow
#![allow(deprecated)]

use arc_swap::ArcSwap;
use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;

use git_features::threading::OwnShared;
use git_features::zlib::stream::deflate;
pub use git_pack as pack;

mod store_impls;
pub use store_impls::{compound, dynamic as store, linked, loose};

pub mod alternate;

/// A way to access objects along with pre-configured thread-local caches for packed base objects as well as objects themselves.
///
/// By default, no cache will be used.
pub struct Cache<S> {
    /// The inner provider of trait implementations we use in conjunction with our caches.
    pub inner: S,
    // TODO: have single-threaded code-paths also for pack-creation (entries from counts) so that we can use OwnShared here
    //       instead of Arc. However, it's probably not that important as these aren't called often.
    new_pack_cache: Option<Arc<cache::NewPackCacheFn>>,
    new_object_cache: Option<Arc<cache::NewObjectCacheFn>>,
    pack_cache: Option<RefCell<Box<cache::PackCache>>>,
    object_cache: Option<RefCell<Box<cache::ObjectCache>>>,
}

///
pub mod cache;

///
/// It can optionally compress the content, similarly to what would happen when using a [`loose::Store`][crate::loose::Store].
///
pub struct Sink {
    compressor: Option<RefCell<deflate::Write<std::io::Sink>>>,
}

/// Create a new [`Sink`] with compression disabled.
pub fn sink() -> Sink {
    Sink { compressor: None }
}

///
pub mod sink;

///
pub mod find;

/// An object database equivalent to `/dev/null`, dropping all objects stored into it.
mod traits;

pub use traits::{Find, FindExt, Write};

/// A thread-local handle to access any object.
pub type Handle = Cache<store::Handle<OwnShared<Store>>>;
/// A thread-local handle to access any object, but thread-safe and independent of the actual type of `OwnShared` or feature toggles in `git-features`.
pub type HandleArc = Cache<store::Handle<Arc<Store>>>;

use store::types;

/// The object store for use in any applications with support for auto-updates in the light of changes to the object database.
///
/// ### Features
///
/// - entirely lazy, creating an instance does no disk IO at all if [`Slots::Given`][store::init::Slots::Given] is used.
/// - multi-threaded lazy-loading of indices and packs
/// - per-thread pack and object caching avoiding cache trashing.
/// - most-recently-used packs are always first for speedups if objects are stored in the same pack, typical for packs organized by
///   commit graph and object age.
/// - lock-free reading for perfect scaling across all cores, and changes to it don't affect readers as long as these don't want to
///   enter the same branch.
/// - sync with the state on disk if objects aren't found to catch up with changes if an object seems to be missing.
///    - turn off the behaviour above for all handles if objects are expected to be missing due to spare checkouts.
pub struct Store {
    /// The central write lock without which the slotmap index can't be changed.
    write: parking_lot::Mutex<()>,

    /// The source directory from which all content is loaded, and the central write lock for use when a directory refresh is needed.
    pub(crate) path: PathBuf,

    /// A list of indices keeping track of which slots are filled with data. These are usually, but not always, consecutive.
    pub(crate) index: ArcSwap<types::SlotMapIndex>,

    /// The below state acts like a slot-map with each slot is mutable when the write lock is held, but readable independently of it.
    /// This allows multiple file to be loaded concurrently if there is multiple handles requesting to load packs or additional indices.
    /// The map is static and cannot typically change.
    /// It's read often and changed rarely.
    pub(crate) files: Vec<types::MutableIndexAndPack>,

    /// The amount of handles that would prevent us from unloading packs or indices
    pub(crate) num_handles_stable: AtomicUsize,
    /// The amount of handles that don't affect our ability to compact our internal data structures or unload packs or indices.
    pub(crate) num_handles_unstable: AtomicUsize,

    /// The amount of times we re-read the disk state to consolidate our in-memory representation.
    pub(crate) num_disk_state_consolidation: AtomicUsize,
}

impl Store {
    /// The root path at which we expect to find all objects and packs.
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
}

/// Create a new cached handle to the object store with support for additional options.
pub fn at_opts(objects_dir: impl Into<PathBuf>, slots: store::init::Slots) -> std::io::Result<Handle> {
    let handle = OwnShared::new(Store::at_opts(objects_dir, slots)?).to_handle();
    Ok(Cache::from(handle))
}

/// Create a new cached handle to the object store.
pub fn at(objects_dir: impl Into<PathBuf>) -> std::io::Result<Handle> {
    at_opts(objects_dir, Default::default())
}
