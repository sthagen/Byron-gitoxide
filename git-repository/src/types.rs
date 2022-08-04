use std::{cell::RefCell, path::PathBuf};

use git_hash::ObjectId;

use crate::head;

/// A worktree checkout containing the files of the repository in consumable form.
pub struct Worktree<'repo> {
    pub(crate) parent: &'repo Repository,
    /// The root path of the checkout.
    pub(crate) path: &'repo std::path::Path,
}

/// The head reference, as created from looking at `.git/HEAD`, able to represent all of its possible states.
///
/// Note that like [`Reference`], this type's data is snapshot of persisted state on disk.
pub struct Head<'repo> {
    /// One of various possible states for the HEAD reference
    pub kind: head::Kind,
    pub(crate) repo: &'repo Repository,
}

/// An [ObjectId] with access to a repository.
#[derive(Clone, Copy)]
pub struct Id<'r> {
    /// The actual object id
    pub(crate) inner: ObjectId,
    pub(crate) repo: &'r Repository,
}

/// A decoded object with a reference to its owning repository.
pub struct Object<'repo> {
    /// The id of the object
    pub id: ObjectId,
    /// The kind of the object
    pub kind: git_object::Kind,
    /// The fully decoded object data
    pub data: Vec<u8>,
    pub(crate) repo: &'repo Repository,
}

impl<'a> Drop for Object<'a> {
    fn drop(&mut self) {
        self.repo.reuse_buffer(&mut self.data);
    }
}

/// A decoded tree object with access to its owning repository.
pub struct Tree<'repo> {
    /// The id of the tree
    pub id: ObjectId,
    /// The fully decoded tree data
    pub data: Vec<u8>,
    pub(crate) repo: &'repo Repository,
}

impl<'a> Drop for Tree<'a> {
    fn drop(&mut self) {
        self.repo.reuse_buffer(&mut self.data);
    }
}

/// A decoded tag object with access to its owning repository.
pub struct Tag<'repo> {
    /// The id of the tree
    pub id: ObjectId,
    /// The fully decoded tag data
    pub data: Vec<u8>,
    pub(crate) repo: &'repo Repository,
}

impl<'a> Drop for Tag<'a> {
    fn drop(&mut self) {
        self.repo.reuse_buffer(&mut self.data);
    }
}

/// A decoded commit object with access to its owning repository.
pub struct Commit<'repo> {
    /// The id of the commit
    pub id: ObjectId,
    /// The fully decoded commit data
    pub data: Vec<u8>,
    pub(crate) repo: &'repo Repository,
}

impl<'a> Drop for Commit<'a> {
    fn drop(&mut self) {
        self.repo.reuse_buffer(&mut self.data);
    }
}

/// A detached, self-contained object, without access to its source repository.
///
/// Use it if an `ObjectRef` should be sent over thread boundaries or stored in collections.
#[derive(Clone)]
pub struct ObjectDetached {
    /// The id of the object
    pub id: ObjectId,
    /// The kind of the object
    pub kind: git_object::Kind,
    /// The fully decoded object data
    pub data: Vec<u8>,
}

/// A reference that points to an object or reference, with access to its source repository.
///
/// Note that these are snapshots and won't recognize if they are stale.
pub struct Reference<'r> {
    /// The actual reference data
    pub inner: git_ref::Reference,
    pub(crate) repo: &'r Repository,
}

/// A thread-local handle to interact with a repository from a single thread.
///
/// It is `Send` but **not** `Sync` - for the latter you can convert it `to_sync()`.
/// Note that it clones itself so that it is empty, requiring the user to configure each clone separately, specifically
/// and explicitly. This is to have the fastest-possible default configuration available by default, but allow
/// those who experiment with workloads to get speed boosts of 2x or more.
pub struct Repository {
    /// A ref store with shared ownership (or the equivalent of it).
    pub refs: crate::RefStore,
    /// A way to access objects.
    pub objects: crate::OdbHandle,

    pub(crate) work_tree: Option<PathBuf>,
    /// The path to the resolved common directory if this is a linked worktree repository or it is otherwise set.
    pub(crate) common_dir: Option<PathBuf>,
    /// A free-list of re-usable object backing buffers
    pub(crate) bufs: RefCell<Vec<Vec<u8>>>,
    /// A pre-assembled selection of often-accessed configuration values for quick access.
    pub(crate) config: crate::config::Cache,
    /// the options obtained when instantiating this repository.
    ///
    /// Particularly useful when following linked worktrees and instantiating new equally configured worktree repositories.
    pub(crate) options: crate::open::Options,
    pub(crate) index: crate::worktree::IndexStorage,
}

/// An instance with access to everything a git repository entails, best imagined as container implementing `Sync + Send` for _most_
/// for system resources required to interact with a `git` repository which are loaded in once the instance is created.
///
/// Use this type to reference it in a threaded context for creation the creation of a thread-local [`Repositories`][Repository].
///
/// Note that this type purposefully isn't very useful until it is converted into a thread-local repository with `to_thread_local()`,
/// it's merely meant to be able to exist in a `Sync` context.
pub struct ThreadSafeRepository {
    /// A store for references to point at objects
    pub refs: crate::RefStore,
    /// A store for objects that contain data
    #[cfg(feature = "unstable")]
    pub objects: git_features::threading::OwnShared<git_odb::Store>,
    #[cfg(not(feature = "unstable"))]
    pub(crate) objects: git_features::threading::OwnShared<git_odb::Store>,
    /// The path to the worktree at which to find checked out files
    pub work_tree: Option<PathBuf>,
    /// The path to the common directory if this is a linked worktree repository or it is otherwise set.
    pub common_dir: Option<PathBuf>,
    pub(crate) config: crate::config::Cache,
    /// options obtained when instantiating this repository for use when following linked worktrees.
    pub(crate) linked_worktree_options: crate::open::Options,
    /// The index of this instances worktree.
    pub(crate) index: crate::worktree::IndexStorage,
}
