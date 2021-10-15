//! git-style registered tempfiles that are removed upon typical termination signals.
//!
//! This crate installs signal handlers the first time its facilities are used.
//! These are powered by [`signal-hook`] to get notified when the application is told to shut down
//! using signals to assure these are deleted. The deletion is filtered by process id to allow forks to have their own
//! set of tempfiles that won't get deleted when the parent process exits.
//!
//! As typical handlers for `TERMination` are installed on first use and effectively overriding the defaults, we install
//! default handlers to restore this behaviour. Whether or not to do that can be controlled using [`force_setup()`].
//!
//! # Note
//!
//! Applications setting their own signal handlers on termination to abort the process probably want to be called after the ones of this crate
//! can call [`force_setup()`] before installing their own handlers.
//! By default, our signal handlers will emulate the default behaviour and abort the process after cleaning temporary files.
//! For full control the application can also prevent our handler to be installed and call it themselves from their own signal handlers.
//!
//! # Limitations
//!
//! ## Tempfiles might remain on disk
//!
//! * Uninterruptible signals are received like `SIGKILL`
//! * The application is performing a write operation on the tempfile when a signal arrives, preventing this tempfile to be removed,
//!   but not others. Any other operation dealing with the tempfile suffers from the same issue.
//!
//! [signal-hook]: https://docs.rs/signal-hook
#![deny(missing_docs, unsafe_code, rust_2018_idioms)]

use std::{
    io,
    marker::PhantomData,
    path::{Path, PathBuf},
    sync::atomic::AtomicUsize,
};

use dashmap::DashMap;
use once_cell::sync::Lazy;

mod fs;
pub use fs::{create_dir, remove_dir};

pub mod handler;

mod forksafe;
use forksafe::ForksafeTempfile;

pub mod handle;
use crate::handle::{Closed, Writable};

static SIGNAL_HANDLER_MODE: AtomicUsize = AtomicUsize::new(SignalHandlerMode::default() as usize);
static NEXT_MAP_INDEX: AtomicUsize = AtomicUsize::new(0);
static REGISTER: Lazy<DashMap<usize, Option<ForksafeTempfile>>> = Lazy::new(|| {
    let mode = SIGNAL_HANDLER_MODE.load(std::sync::atomic::Ordering::SeqCst);
    if mode != SignalHandlerMode::None as usize {
        for sig in signal_hook::consts::TERM_SIGNALS {
            // SAFETY: handlers are considered unsafe because a lot can go wrong. See `cleanup_tempfiles()` for details on safety.
            #[allow(unsafe_code)]
            unsafe {
                #[cfg(not(windows))]
                {
                    signal_hook_registry::register_sigaction(*sig, handler::cleanup_tempfiles_nix)
                }
                #[cfg(windows)]
                {
                    signal_hook::low_level::register(*sig, handler::cleanup_tempfiles_windows)
                }
            }
            .expect("signals can always be installed");
        }
    }
    DashMap::new()
});

/// Define how our signal handlers act
#[derive(Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq)]
pub enum SignalHandlerMode {
    /// Do not install a signal handler at all, but have somebody else call our handler directly.
    None = 0,
    /// Delete all remaining registered tempfiles on termination.
    DeleteTempfilesOnTermination = 1,
    /// Delete all remaining registered tempfiles on termination and emulate the default handler behaviour.
    ///
    /// This is the default, which leads to the process to be aborted.
    DeleteTempfilesOnTerminationAndRestoreDefaultBehaviour = 2,
}

impl SignalHandlerMode {
    /// By default we will emulate the default behaviour and abort the process.
    ///
    /// While testing, we will not abort the process.
    const fn default() -> Self {
        #[cfg(not(test))]
        return SignalHandlerMode::DeleteTempfilesOnTerminationAndRestoreDefaultBehaviour;
        #[cfg(test)]
        return SignalHandlerMode::DeleteTempfilesOnTermination;
    }
}

/// A type expressing the ways we can deal with directories containing a tempfile.
#[derive(Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq)]
pub enum ContainingDirectory {
    /// Assume the directory for the tempfile exists and cause failure if it doesn't
    Exists,
    /// Create the directory recursively with the given amount of retries in a way that is somewhat race resistant
    /// depending on the amount of retries.
    CreateAllRaceProof(create_dir::Retries),
}

/// A type expressing the ways we cleanup after ourselves to remove resources we created.
/// Note that cleanup has no effect if the tempfile is persisted.
#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub enum AutoRemove {
    /// Remove the temporary file after usage if it wasn't persisted.
    Tempfile,
    /// Remove the temporary file as well the containing directories if they are empty until the given `directory`.
    TempfileAndEmptyParentDirectoriesUntil {
        /// The directory which shall not be removed even if it is empty.
        boundary_directory: PathBuf,
    },
}

impl AutoRemove {
    fn execute_best_effort(self, directory_to_potentially_delete: &Path) -> Option<PathBuf> {
        match self {
            AutoRemove::Tempfile => None,
            AutoRemove::TempfileAndEmptyParentDirectoriesUntil { boundary_directory } => {
                crate::remove_dir::empty_upward_until_boundary(directory_to_potentially_delete, &boundary_directory)
                    .ok();
                Some(boundary_directory)
            }
        }
    }
}

/// A registered temporary file which will delete itself on drop or if the program is receiving signals that
/// should cause it to terminate.
///
/// # Note
///
/// Signals interrupting the calling thread right after taking ownership of the registered tempfile
/// will cause all but this tempfile to be removed automatically. In the common case it will persist on disk as destructors
/// were not called or didn't get to remove the file.
///
/// In the best case the file is a true temporary with a non-clashing name that 'only' fills up the disk,
/// in the worst case the temporary file is used as a lock file which may leave the repository in a locked
/// state forever.
///
/// This kind of raciness exists whenever [`take()`][Handle::take()] is used and can't be circumvented.
#[derive(Debug)]
#[must_use = "A handle that is immediately dropped doesn't lock a resource meaningfully"]
pub struct Handle<Marker: std::fmt::Debug> {
    id: usize,
    _marker: PhantomData<Marker>,
}

/// A shortcut to [`Handle::<Writable>::new()`], creating a writable temporary file with non-clashing name in a directory.
pub fn new(
    containing_directory: impl AsRef<Path>,
    directory: ContainingDirectory,
    cleanup: AutoRemove,
) -> io::Result<Handle<Writable>> {
    Handle::<Writable>::new(containing_directory, directory, cleanup)
}

/// A shortcut to [`Handle::<Writable>::at()`] providing a writable temporary file at the given path.
pub fn writable_at(
    path: impl AsRef<Path>,
    directory: ContainingDirectory,
    cleanup: AutoRemove,
) -> io::Result<Handle<Writable>> {
    Handle::<Writable>::at(path, directory, cleanup)
}

/// A shortcut to [`Handle::<Closed>::at()`] providing a closed temporary file to mark the presence of something.
pub fn mark_at(
    path: impl AsRef<Path>,
    directory: ContainingDirectory,
    cleanup: AutoRemove,
) -> io::Result<Handle<Closed>> {
    Handle::<Closed>::at(path, directory, cleanup)
}

/// Explicitly (instead of lazily) initialize signal handlers and other state to keep track of tempfiles.
/// Only has an effect the first time it is called and furthermore allows to set the `mode` in which signal handlers
/// are installed.
///
/// This is required if the application wants to install their own signal handlers _after_ the ones defined here.
pub fn force_setup(mode: SignalHandlerMode) {
    SIGNAL_HANDLER_MODE.store(mode as usize, std::sync::atomic::Ordering::SeqCst);
    Lazy::force(&REGISTER);
}
