use std::path::PathBuf;

/// A repository path which either points to a work tree or the `.git` repository itself.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum Path {
    /// The currently checked out linked worktree along with its connected and existing git directory.
    LinkedWorkTree {
        /// The base of the work tree.
        work_dir: PathBuf,
        /// The worktree-private git dir, located within the main git directory which holds most of the information.
        git_dir: PathBuf,
    },
    /// The currently checked out or nascent work tree of a git repository
    WorkTree(PathBuf),
    /// The git repository itself, typically bare and without known worktree.
    ///
    /// Note that it might still have linked work-trees which can be accessed later, weather bare or not.
    Repository(PathBuf),
}

mod path {
    use std::path::PathBuf;

    use crate::{
        path::without_dot_git_dir,
        repository::{Kind, Path},
        DOT_GIT_DIR,
    };

    impl AsRef<std::path::Path> for Path {
        fn as_ref(&self) -> &std::path::Path {
            match self {
                Path::WorkTree(path)
                | Path::Repository(path)
                | Path::LinkedWorkTree {
                    work_dir: _,
                    git_dir: path,
                } => path,
            }
        }
    }

    impl Path {
        /// Instantiate a new path from `dir` which is expected to be the `.git` directory, with `kind` indicating
        /// whether it's a bare repository or not.
        pub fn from_dot_git_dir(dir: impl Into<PathBuf>, kind: Kind) -> Self {
            fn absolutize_on_trailing_parent(dir: PathBuf) -> PathBuf {
                if !matches!(dir.components().rev().next(), Some(std::path::Component::ParentDir)) {
                    dir
                } else {
                    git_path::absolutize(&dir, std::env::current_dir().ok()).into_owned()
                }
            }

            let dir = dir.into();
            match kind {
                Kind::WorkTreeGitDir { work_dir } => Path::LinkedWorkTree { git_dir: dir, work_dir },
                Kind::WorkTree { linked_git_dir } => match linked_git_dir {
                    Some(git_dir) => Path::LinkedWorkTree {
                        git_dir,
                        work_dir: without_dot_git_dir(absolutize_on_trailing_parent(dir)),
                    },
                    None => {
                        let mut dir = absolutize_on_trailing_parent(dir);
                        dir.pop(); // ".git" suffix
                        let work_dir = dir.as_os_str().is_empty().then(|| PathBuf::from(".")).unwrap_or(dir);
                        Path::WorkTree(work_dir)
                    }
                },
                Kind::Bare => Path::Repository(dir),
            }
        }
        /// Returns the [kind][Kind] of this repository path.
        pub fn kind(&self) -> Kind {
            match self {
                Path::LinkedWorkTree { work_dir: _, git_dir } => Kind::WorkTree {
                    linked_git_dir: Some(git_dir.to_owned()),
                },
                Path::WorkTree(_) => Kind::WorkTree { linked_git_dir: None },
                Path::Repository(_) => Kind::Bare,
            }
        }

        /// Consume and split this path into the location of the `.git` directory as well as an optional path to the work tree.
        pub fn into_repository_and_work_tree_directories(self) -> (PathBuf, Option<PathBuf>) {
            match self {
                Path::LinkedWorkTree { work_dir, git_dir } => (git_dir, Some(work_dir)),
                Path::WorkTree(working_tree) => (working_tree.join(DOT_GIT_DIR), Some(working_tree)),
                Path::Repository(repository) => (repository, None),
            }
        }
    }
}

/// The kind of repository path.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum Kind {
    /// A bare repository does not have a work tree, that is files on disk beyond the `git` repository itself.
    ///
    /// Note that this is merely a guess at this point as we didn't read the configuration yet.
    Bare,
    /// A `git` repository along with a checked out files in a work tree.
    WorkTree {
        /// If set, this is the git dir associated with this _linked_ worktree.
        /// If `None`, the git_dir is the `.git` directory inside the _main_ worktree we represent.
        linked_git_dir: Option<PathBuf>,
    },
    /// A worktree's git directory in the common`.git` directory in `worktrees/<name>`.
    WorkTreeGitDir {
        /// Path to the worktree directory.
        work_dir: PathBuf,
    },
}

impl Kind {
    /// Returns true if this is a bare repository, one without a work tree.
    pub fn is_bare(&self) -> bool {
        matches!(self, Kind::Bare)
    }
}
