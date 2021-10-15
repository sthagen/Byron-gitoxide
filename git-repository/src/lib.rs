//! This crate provides the [`Repository`] abstraction which serves as a hub into all the functionality of git.
//!
//! It's powerful and won't sacrifice performance while still increasing convenience compared to using the sub-crates
//! individually. Sometimes it may hide complexity under the assumption that the performance difference doesn't matter
//! for all but the fewest tools out there, which would be using the underlying crates directly or file an issue.
//!
//! # The prelude and extensions
//!
//! With `use git_repository::prelude::*` you should be ready to go as it pulls in various extension traits to make functionality
//! available on objects that may use it.
//!
//! The method signatures are still complex and may require various arguments for configuration and cache control.
//!
//! ## Easy-Mode
//!
//! Most extensions to existing objects provide an `obj_with_extension.easy(&repo).an_easier_version_of_a_method()` or `easy(&repo)`
//! method to hide all complex arguments and sacrifice some performance for a lot of convenience.
//!
//! When starting out, use `easy(…)` and migrate to the more detailed method signatures to squeeze out the last inkling of performance
//! if it really does make a difference.
//!
//! ## Object-Access Performance
//!
//! Accessing objects quickly is the bread-and-butter of working with git, right after accessing references. Hence it's vital
//! to understand which cache levels exist and how to leverage them.
//!
//! When accessing an object, the first cache that's queried is a  memory-capped LRU object cache, mapping their id to data and kind.
//! On miss, the object is looked up and if ia pack is hit, there is a small fixed-size cache for delta-base objects.
//!
//! In scenarios where the same objects are accessed multiple times, an object cache can be useful and is to be configured specifically
//! using the [`object_cache_size(…)`][prelude::CacheAccessExt::object_cache_size()] method.
//!
//! Use the `cache-efficiency-debug` cargo feature to learn how efficient the cache actually is - it's easy to end up with lowered
//! performance if the cache is not hit in 50% of the time.
//!
//! Environment variables can also be used for configuration if the application is calling
//! [`apply_environment()`][prelude::CacheAccessExt::apply_environment()] on their `Easy*` accordingly.
//!
//! ### Shortcomings & Limitations
//!
//! - Only one `easy::Object` or derivatives can be held in memory at a time, _per `Easy*`_.
//! - Changes made to the configuration, packs, and alternates aren't picked up automatically if they aren't
//!   made through the underlying `Repository` instance. Run one of the [`refresh*()`][prelude::RepositoryAccessExt] to trigger
//!   an update. Also note that this is only a consideration for long-running processes.
//!
//! ### Design Sketch
//!
//! Goal is to make the lower-level plumbing available without having to deal with any caches or buffers, and avoid any allocation
//! beyond sizing the buffer to fit the biggest object seen so far.
//!
//! * no implicit object lookups, thus `Oid` needs to get an `Object` first to start out with data via `object()`
//! * Objects with `Ref` suffix can only exist one at a time unless they are transformed into an owned version of it OR
//!   multiple `Easy` handles are present, each providing another 'slot' for an object as long as its retrieved through
//!   the respective `Easy` object.
//! * `ObjectRef` blocks the current buffer, hence many of its operations that use the buffer are consuming
//! * All methods that access a any field from `Easy`'s mutable `State` are fallible, and return `easy::Result<_>` at least, to avoid
//!   panics if the field can't be referenced due to borrow rules of `RefCell`.
//! * Anything attached to `Access` can be detached to lift the object limit or make them `Send`-able. They can be `attached` to another
//!   `Access` if needed.
//! * `git-repository` functions related to `Access` extensions will always return attached versions of return values, like `Oid` instead
//!   of `git_hash::ObjectId`, `ObjectRef` instead of `git_odb::data::Object`, or `Reference` instead of `git_ref::Reference`.
//! * Obtaining mutable is currently a weak spot as these only work with Arc<RwLock> right now and can't work with `Rc<RefCell>` due
//!   to missing GATs, presumably. All `Easy*!Exclusive` types are unable to provide a mutable reference to the underlying repository.
//!   However, other ways to adjust the `Repository` of long-running applications are possible. For instance, there could be a flag that
//!   indicates a new `Repository` should be created (for instance, after it was changed) which causes the next server connection to
//!   create a new one. This instance is the one to use when spawning new `EasyArc` instances.
//! * `Platform` types are used to hold mutable or shared versions of required state for use in dependent objects they create, like iterators.
//!   These come with the benefit of allowing for nicely readable call chains. Sometimes these are called `Platform` for a lack of a more specific
//!   term, some are called more specifically like `Ancestors`.
//!
//! ### Terminology
//!
//! #### WorkingTree and WorkTree
//!
//! When reading the documentation of the canonical git-worktree program one gets the impression work tree and working tree are used
//! interchangeably. We use the term _work tree_ only and try to do so consistently as its shorter and assumed to be the same.
//!
//! # Cargo-features
//!
//! ## With the optional "unstable" cargo feature
//!
//! To make using  _sub-crates_ easier these are re-exported into the root of this crate. Note that these may change their major version
//! even if this crate doesn't, hence breaking downstream.
//!
//! `git_repository::`
//! * [`hash`]
//! * [`url`]
//! * [`actor`]
//! * [`bstr`][bstr]
//! * [`objs`]
//! * [`odb`]
//!   * [`pack`][odb::pack]
//! * [`refs`]
//! * [`interrupt`]
//! * [`tempfile`]
//! * [`lock`]
//! * [`traverse`]
//! * [`diff`]
//! * [`parallel`]
//! * [`Progress`]
//! * [`progress`]
//! * [`interrupt`]
//! * [`protocol`]
//!   * [`transport`][protocol::transport]
//!     * [`packetline`][protocol::transport::packetline]
//!
#![deny(missing_docs, unsafe_code, rust_2018_idioms)]

use std::{path::PathBuf, rc::Rc, sync::Arc};

// Re-exports to make this a potential one-stop shop crate avoiding people from having to reference various crates themselves.
// This also means that their major version changes affect our major version, but that's alright as we directly expose their
// APIs/instances anyway.
pub use git_actor as actor;
#[cfg(all(feature = "unstable", feature = "git-diff"))]
pub use git_diff as diff;
#[cfg(feature = "unstable")]
pub use git_features::{parallel, progress, progress::Progress};
pub use git_hash as hash;
#[doc(inline)]
pub use git_hash::{oid, ObjectId};
pub use git_lock as lock;
pub use git_object as objs;
pub use git_object::bstr;
#[cfg(feature = "unstable")]
pub use git_odb as odb;
#[cfg(all(feature = "unstable", feature = "git-protocol"))]
pub use git_protocol as protocol;
pub use git_ref as refs;
#[cfg(feature = "unstable")]
pub use git_tempfile as tempfile;
#[cfg(feature = "unstable")]
pub use git_traverse as traverse;
#[cfg(all(feature = "unstable", feature = "git-url"))]
pub use git_url as url;
#[doc(inline)]
#[cfg(all(feature = "unstable", feature = "git-url"))]
pub use git_url::Url;

pub mod interrupt;

mod ext;
///
pub mod prelude {
    pub use git_features::parallel::reduce::Finalize;
    pub use git_odb::{Find, FindExt, Write};

    pub use crate::{easy::ext::*, ext::*};
}

///
pub mod path;

mod repository;
pub use repository::{discover, init, open};

/// A repository path which either points to a work tree or the `.git` repository itself.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum Path {
    /// The currently checked out or nascent work tree of a git repository.
    WorkTree(PathBuf),
    /// The git repository itself
    Repository(PathBuf),
}

/// A instance with access to everything a git repository entails, best imagined as container for _most_ for system resources required
/// to interact with a `git` repository which are loaded in once the instance is created.
///
/// These resources are meant to be shareable across threads and used by most using an `Easy*` type has a handle carrying additional
/// in-memory data to accelerate data access or hold volatile data. Depending on context, `EasyShared` gets the fastest read-only
/// access to the repository, whereas `Easy` has to go through an `Rc` and `EasyArcExclusive` through an `Arc<RwLock>`.
///
/// Namely, this is an object database, a reference database to point to objects.
pub struct Repository {
    /// A store for references to point at objects
    pub refs: git_ref::file::Store,
    /// A store for objects that contain data
    #[cfg(feature = "unstable")]
    pub odb: git_odb::linked::Store,
    #[cfg(not(feature = "unstable"))]
    pub(crate) odb: git_odb::linked::Store,
    /// The path to the worktree at which to find checked out files
    pub work_tree: Option<PathBuf>,
    pub(crate) hash_kind: git_hash::Kind,
    // TODO: git-config should be here - it's read a lot but not written much in must applications, so shouldn't be in `State`.
    //       Probably it's best reload it on signal (in servers) or refresh it when it's known to have been changed similar to how
    //       packs are refreshed. This would be `git_config::fs::Config` when ready.
    // pub(crate) config: git_config::file::GitConfig<'static>,
}

/// A handle to a `Repository` for use when the repository needs to be shared, providing state for one `ObjectRef` at a time, , created with [`Repository::into_easy()`].
///
/// For use in one-off single-threaded commands that don't have to deal with the changes they potentially incur.
/// TODO: There should be an `EasyExclusive` using `Rc<RefCell<…>>` but that needs GATs.
#[derive(Clone)]
pub struct Easy {
    /// The repository
    pub repo: Rc<Repository>,
    /// The state with interior mutability
    pub state: easy::State,
}

/// A handle to a repository for use when the repository needs to be shared using an actual reference, providing state for one `ObjectRef` at a time, created with [`Repository::to_easy()`]
///
/// For use in one-off commands that don't have to deal with the changes they potentially incur.
#[derive(Clone)]
pub struct EasyShared<'a> {
    /// The repository
    pub repo: &'a Repository,
    /// The state with interior mutability
    pub state: easy::State,
}

/// A handle to a `Repository` for sharing across threads, with each thread having one or more caches,
/// created with [`Repository::into_easy_arc()`]
///
/// For use in one-off commands in threaded applications that don't have to deal with the changes they potentially incur.
#[derive(Clone)]
pub struct EasyArc {
    /// The repository
    pub repo: Arc<Repository>,
    /// The state with interior mutability
    pub state: easy::State,
}

/// A handle to a optionally mutable `Repository` for use in long-running applications that eventually need to update the `Repository`
/// to adapt to changes they triggered or that were caused by other processes.
///
/// Using it incurs costs as each `Repository` access has to go through an indirection and involve an _eventually fair_ `RwLock`.
/// However, it's vital to precisely updating the `Repository` instance as opposed to creating a new one while serving other requests
/// on an old instance, which potentially duplicates the resource costs.
///
/// ### Limitation
///
/// * **It can take a long time to get a mutable `repo`….**
///    - Imagine a typical server operation where a pack is sent to a client. As it's a fresh clone it takes 5 minutes.
///      Right after the clone began somebody initiates a push. Everything goes well and as a new pack was created the server
///      wants to tell the object database to update its packs, making the new one available. To do that, it needs mutable access to the
///      repository instance, and obtaining it will take until the end of the ongoing clone as the latter has acquired read-access to
///      the same repository instance. This is most certainly undesirable.
///   - Workarounds would be to
///       - acquire the read-lock each time the clone operation wants to access an object
///       - set a flag that triggers the server to create a new `Repository` instance next time a connection comes in which is subsequently
///         shared across additional connections.
///       - Create a new `Repository` per connection.
#[derive(Clone)]
pub struct EasyArcExclusive {
    /// The repository
    pub repo: Arc<parking_lot::RwLock<Repository>>,
    /// The state with interior mutability
    pub state: easy::State,
}

pub mod easy;

///
pub mod commit;
///
pub mod reference;

/// The kind of `Repository`
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum Kind {
    /// A bare repository does not have a work tree, that is files on disk beyond the `git` repository itself.
    Bare,
    /// A `git` repository along with a checked out files in a work tree.
    WorkTree,
}

impl Kind {
    /// Returns true if this is a bare repository, one without a work tree.
    pub fn is_bare(&self) -> bool {
        matches!(self, Kind::Bare)
    }
}

/// See [Repository::discover()].
pub fn discover(directory: impl AsRef<std::path::Path>) -> Result<Repository, repository::discover::Error> {
    Repository::discover(directory)
}

/// See [Repository::init()].
pub fn init(directory: impl AsRef<std::path::Path>) -> Result<Repository, repository::init::Error> {
    Repository::init(directory, Kind::WorkTree)
}

/// See [Repository::init()].
pub fn init_bare(directory: impl AsRef<std::path::Path>) -> Result<Repository, repository::init::Error> {
    Repository::init(directory, Kind::Bare)
}

/// See [Repository::open()].
pub fn open(directory: impl Into<std::path::PathBuf>) -> Result<Repository, repository::open::Error> {
    Repository::open(directory)
}
