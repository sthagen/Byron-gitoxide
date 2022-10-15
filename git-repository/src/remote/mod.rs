/// The direction of an operation carried out (or to be carried out) through a remote.
#[derive(Debug, Eq, PartialEq, Copy, Clone, Hash)]
pub enum Direction {
    /// Push local changes to the remote.
    Push,
    /// Fetch changes from the remote to the local repository.
    Fetch,
}

impl Direction {
    /// Return ourselves as string suitable for use as verb in an english sentence.
    pub fn as_str(&self) -> &'static str {
        match self {
            Direction::Push => "push",
            Direction::Fetch => "fetch",
        }
    }
}

mod build;

mod errors;
pub use errors::find;

///
pub mod name {
    /// The error returned by [validated()].
    #[derive(Debug, thiserror::Error)]
    #[error("remote names must be valid within refspecs for fetching: {name:?}")]
    #[allow(missing_docs)]
    pub struct Error {
        source: git_refspec::parse::Error,
        name: String,
    }

    /// Return `name` if it is valid or convert it into an `Error`.
    pub fn validated(name: impl Into<String>) -> Result<String, Error> {
        let name = name.into();
        match git_refspec::parse(
            format!("refs/heads/test:refs/remotes/{name}/test").as_str().into(),
            git_refspec::parse::Operation::Fetch,
        ) {
            Ok(_) => Ok(name),
            Err(err) => Err(Error { source: err, name }),
        }
    }
}

///
pub mod init;

///
#[cfg(any(feature = "async-network-client", feature = "blocking-network-client"))]
pub mod fetch;

///
#[cfg(any(feature = "async-network-client", feature = "blocking-network-client"))]
pub mod connect;

#[cfg(any(feature = "async-network-client", feature = "blocking-network-client"))]
mod connection;
#[cfg(any(feature = "async-network-client", feature = "blocking-network-client"))]
pub use connection::{ref_map, Connection};

///
pub mod save;

mod access;
pub(crate) mod url;
