use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use crate::bstr::ByteSlice;

/// The error used in [`into()`].
#[derive(Debug, thiserror::Error)]
#[allow(missing_docs)]
pub enum Error {
    #[error("Could not open data at '{}'", .path.display())]
    IoOpen { source: std::io::Error, path: PathBuf },
    #[error("Could not write data at '{}'", .path.display())]
    IoWrite { source: std::io::Error, path: PathBuf },
    #[error("Refusing to initialize the existing '{}' directory", .path.display())]
    DirectoryExists { path: PathBuf },
    #[error("Refusing to initialize the non-empty directory as '{}'", .path.display())]
    DirectoryNotEmpty { path: PathBuf },
    #[error("Could not create directory at '{}'", .path.display())]
    CreateDirectory { source: std::io::Error, path: PathBuf },
}

const GIT_DIR_NAME: &str = ".git";

const TPL_INFO_EXCLUDE: &[u8] = include_bytes!("../assets/baseline-init/info/exclude");
const TPL_HOOKS_APPLYPATCH_MSG: &[u8] = include_bytes!("../assets/baseline-init/hooks/applypatch-msg.sample");
const TPL_HOOKS_COMMIT_MSG: &[u8] = include_bytes!("../assets/baseline-init/hooks/commit-msg.sample");
const TPL_HOOKS_FSMONITOR_WATCHMAN: &[u8] = include_bytes!("../assets/baseline-init/hooks/fsmonitor-watchman.sample");
const TPL_HOOKS_POST_UPDATE: &[u8] = include_bytes!("../assets/baseline-init/hooks/post-update.sample");
const TPL_HOOKS_PRE_APPLYPATCH: &[u8] = include_bytes!("../assets/baseline-init/hooks/pre-applypatch.sample");
const TPL_HOOKS_PRE_COMMIT: &[u8] = include_bytes!("../assets/baseline-init/hooks/pre-commit.sample");
const TPL_HOOKS_PRE_MERGE_COMMIT: &[u8] = include_bytes!("../assets/baseline-init/hooks/pre-merge-commit.sample");
const TPL_HOOKS_PRE_PUSH: &[u8] = include_bytes!("../assets/baseline-init/hooks/pre-push.sample");
const TPL_HOOKS_PRE_REBASE: &[u8] = include_bytes!("../assets/baseline-init/hooks/pre-rebase.sample");
const TPL_HOOKS_PRE_RECEIVE: &[u8] = include_bytes!("../assets/baseline-init/hooks/pre-receive.sample");
const TPL_HOOKS_PREPARE_COMMIT_MSG: &[u8] = include_bytes!("../assets/baseline-init/hooks/prepare-commit-msg.sample");
const TPL_HOOKS_UPDATE: &[u8] = include_bytes!("../assets/baseline-init/hooks/update.sample");
const TPL_CONFIG: &[u8] = include_bytes!("../assets/baseline-init/config");
const TPL_DESCRIPTION: &[u8] = include_bytes!("../assets/baseline-init/description");
const TPL_HEAD: &[u8] = include_bytes!("../assets/baseline-init/HEAD");

struct PathCursor<'a>(&'a mut PathBuf);

struct NewDir<'a>(&'a mut PathBuf);

impl<'a> PathCursor<'a> {
    fn at(&mut self, component: &str) -> &Path {
        self.0.push(component);
        self.0.as_path()
    }
}

impl<'a> NewDir<'a> {
    fn at(self, component: &str) -> Result<Self, Error> {
        self.0.push(component);
        create_dir(self.0)?;
        Ok(self)
    }
    fn as_mut(&mut self) -> &mut PathBuf {
        self.0
    }
}

impl<'a> Drop for NewDir<'a> {
    fn drop(&mut self) {
        self.0.pop();
    }
}

impl<'a> Drop for PathCursor<'a> {
    fn drop(&mut self) {
        self.0.pop();
    }
}

fn write_file(data: &[u8], path: &Path) -> Result<(), Error> {
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .append(false)
        .open(path)
        .map_err(|e| Error::IoOpen {
            source: e,
            path: path.to_owned(),
        })?;
    file.write_all(data).map_err(|e| Error::IoWrite {
        source: e,
        path: path.to_owned(),
    })
}

fn create_dir(p: &Path) -> Result<(), Error> {
    fs::create_dir_all(p).map_err(|e| Error::CreateDirectory {
        source: e,
        path: p.to_owned(),
    })
}

/// Create a new `.git` repository of `kind` within the possibly non-existing `directory`
/// and return its path.
pub fn into(directory: impl Into<PathBuf>, kind: crate::Kind) -> Result<crate::Path, Error> {
    let mut dot_git = directory.into();

    match kind {
        crate::Kind::Bare => {
            if std::fs::read_dir(&dot_git)
                .map_err(|err| Error::IoOpen {
                    source: err,
                    path: dot_git.clone(),
                })?
                .count()
                != 0
            {
                return Err(Error::DirectoryNotEmpty { path: dot_git });
            }
        }
        crate::Kind::WorkTree => {
            dot_git.push(GIT_DIR_NAME);

            if dot_git.is_dir() {
                return Err(Error::DirectoryExists { path: dot_git });
            }
        }
    }
    create_dir(&dot_git)?;

    {
        let mut cursor = NewDir(&mut dot_git).at("info")?;
        write_file(TPL_INFO_EXCLUDE, PathCursor(cursor.as_mut()).at("exclude"))?;
    }

    {
        let mut cursor = NewDir(&mut dot_git).at("hooks")?;
        for (tpl, filename) in &[
            (TPL_HOOKS_UPDATE, "update.sample"),
            (TPL_HOOKS_PREPARE_COMMIT_MSG, "prepare-commit-msg.sample"),
            (TPL_HOOKS_PRE_RECEIVE, "pre-receive.sample"),
            (TPL_HOOKS_PRE_REBASE, "pre-rebase.sample"),
            (TPL_HOOKS_PRE_PUSH, "pre-push.sample"),
            (TPL_HOOKS_PRE_COMMIT, "pre-commit.sample"),
            (TPL_HOOKS_PRE_MERGE_COMMIT, "pre-merge-commit.sample"),
            (TPL_HOOKS_PRE_APPLYPATCH, "pre-applypatch.sample"),
            (TPL_HOOKS_POST_UPDATE, "post-update.sample"),
            (TPL_HOOKS_FSMONITOR_WATCHMAN, "fsmonitor-watchman.sample"),
            (TPL_HOOKS_COMMIT_MSG, "commit-msg.sample"),
            (TPL_HOOKS_APPLYPATCH_MSG, "applypatch-msg.sample"),
        ] {
            write_file(tpl, PathCursor(cursor.as_mut()).at(filename))?;
        }
    }

    {
        let mut cursor = NewDir(&mut dot_git).at("objects")?;
        create_dir(PathCursor(cursor.as_mut()).at("info"))?;
        create_dir(PathCursor(cursor.as_mut()).at("pack"))?;
    }

    {
        let mut cursor = NewDir(&mut dot_git).at("refs")?;
        create_dir(PathCursor(cursor.as_mut()).at("heads"))?;
        create_dir(PathCursor(cursor.as_mut()).at("tags"))?;
    }

    for (tpl, filename) in &[
        (TPL_HEAD, "HEAD"),
        (TPL_DESCRIPTION, "description"),
        (TPL_CONFIG, "config"),
    ] {
        if *filename == "config" {
            write_file(
                &tpl.replace(
                    "{bare-value}",
                    match kind {
                        crate::Kind::Bare => "true",
                        crate::Kind::WorkTree => "false",
                    },
                ),
                PathCursor(&mut dot_git).at(filename),
            )?;
        } else {
            write_file(tpl, PathCursor(&mut dot_git).at(filename))?;
        }
    }

    Ok(crate::Path::from_dot_git_dir(dot_git, kind))
}
