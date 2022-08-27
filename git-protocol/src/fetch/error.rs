use std::io;

use git_transport::client;

use crate::fetch::handshake;
use crate::fetch::{refs, response};

/// The error used in [`fetch()`][crate::fetch()].
#[derive(Debug, thiserror::Error)]
#[allow(missing_docs)]
pub enum Error {
    #[error(transparent)]
    Handshake(#[from] handshake::Error),
    #[error("Could not access repository or failed to read streaming pack file")]
    Io(#[from] io::Error),
    #[error(transparent)]
    Transport(#[from] client::Error),
    #[error(transparent)]
    Refs(#[from] refs::Error),
    #[error(transparent)]
    Response(#[from] response::Error),
}
