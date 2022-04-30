use std::path::PathBuf;

use anyhow::{Context as AnyhowContext, Result};

pub fn init(directory: Option<PathBuf>) -> Result<git_repository::Path> {
    git_repository::path::create::into(directory.unwrap_or_default(), git_repository::Kind::WorkTree)
        .with_context(|| "Repository initialization failed")
}

pub mod tree;

pub mod commit;

pub mod verify;

pub mod odb;

pub mod mailmap;

pub mod exclude;
