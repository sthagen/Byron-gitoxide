use std::{
    convert::TryFrom,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use git_config::parse::section;
use git_discover::DOT_GIT_DIR;

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

const TPL_INFO_EXCLUDE: &[u8] = include_bytes!("assets/baseline-init/info/exclude");
const TPL_HOOKS_APPLYPATCH_MSG: &[u8] = include_bytes!("assets/baseline-init/hooks/applypatch-msg.sample");
const TPL_HOOKS_COMMIT_MSG: &[u8] = include_bytes!("assets/baseline-init/hooks/commit-msg.sample");
const TPL_HOOKS_FSMONITOR_WATCHMAN: &[u8] = include_bytes!("assets/baseline-init/hooks/fsmonitor-watchman.sample");
const TPL_HOOKS_POST_UPDATE: &[u8] = include_bytes!("assets/baseline-init/hooks/post-update.sample");
const TPL_HOOKS_PRE_APPLYPATCH: &[u8] = include_bytes!("assets/baseline-init/hooks/pre-applypatch.sample");
const TPL_HOOKS_PRE_COMMIT: &[u8] = include_bytes!("assets/baseline-init/hooks/pre-commit.sample");
const TPL_HOOKS_PRE_MERGE_COMMIT: &[u8] = include_bytes!("assets/baseline-init/hooks/pre-merge-commit.sample");
const TPL_HOOKS_PRE_PUSH: &[u8] = include_bytes!("assets/baseline-init/hooks/pre-push.sample");
const TPL_HOOKS_PRE_REBASE: &[u8] = include_bytes!("assets/baseline-init/hooks/pre-rebase.sample");
const TPL_HOOKS_PRE_RECEIVE: &[u8] = include_bytes!("assets/baseline-init/hooks/pre-receive.sample");
const TPL_HOOKS_PREPARE_COMMIT_MSG: &[u8] = include_bytes!("assets/baseline-init/hooks/prepare-commit-msg.sample");
const TPL_HOOKS_UPDATE: &[u8] = include_bytes!("assets/baseline-init/hooks/update.sample");
const TPL_DESCRIPTION: &[u8] = include_bytes!("assets/baseline-init/description");
const TPL_HEAD: &[u8] = include_bytes!("assets/baseline-init/HEAD");

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

/// Options for use in [`into()`];
#[derive(Copy, Clone)]
pub struct Options {
    /// If true, the repository will be a bare repository without a worktree.
    pub bare: bool,

    /// If set, use these filesystem capabilities to populate the respective git-config fields.
    /// If `None`, the directory will be probed.
    pub fs_capabilities: Option<git_worktree::fs::Capabilities>,
}

/// Create a new `.git` repository of `kind` within the possibly non-existing `directory`
/// and return its path.
pub fn into(
    directory: impl Into<PathBuf>,
    Options { bare, fs_capabilities }: Options,
) -> Result<git_discover::repository::Path, Error> {
    let mut dot_git = directory.into();

    if bare {
        if fs::read_dir(&dot_git)
            .map_err(|err| Error::IoOpen {
                source: err,
                path: dot_git.clone(),
            })?
            .count()
            != 0
        {
            return Err(Error::DirectoryNotEmpty { path: dot_git });
        }
    } else {
        dot_git.push(DOT_GIT_DIR);

        if dot_git.is_dir() {
            return Err(Error::DirectoryExists { path: dot_git });
        }
    };
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

    for (tpl, filename) in &[(TPL_HEAD, "HEAD"), (TPL_DESCRIPTION, "description")] {
        write_file(tpl, PathCursor(&mut dot_git).at(filename))?;
    }

    {
        let mut config = git_config::File::default();
        {
            let caps = fs_capabilities.unwrap_or_else(|| git_worktree::fs::Capabilities::probe(&dot_git));
            let mut core = config.new_section("core", None).expect("valid section name");

            core.push(key("repositoryformatversion"), "0");
            core.push(key("filemode"), bool(caps.executable_bit));
            core.push(key("bare"), bool(bare));
            core.push(key("logallrefupdates"), bool(!bare));
            core.push(key("symlinks"), bool(caps.symlink));
            core.push(key("ignorecase"), bool(caps.ignore_case));
            core.push(key("precomposeunicode"), bool(caps.precompose_unicode));
        }
        let mut cursor = PathCursor(&mut dot_git);
        let config_path = cursor.at("config");
        std::fs::write(&config_path, &config.to_bstring()).map_err(|err| Error::IoWrite {
            source: err,
            path: config_path.to_owned(),
        })?;
    }

    Ok(git_discover::repository::Path::from_dot_git_dir(
        dot_git,
        bare.then(|| git_discover::repository::Kind::Bare)
            .unwrap_or(git_discover::repository::Kind::WorkTree { linked_git_dir: None }),
    ))
}

fn key(name: &'static str) -> section::Key<'static> {
    section::Key::try_from(name).expect("valid key name")
}

fn bool(v: bool) -> &'static str {
    match v {
        true => "true",
        false => "false",
    }
}
