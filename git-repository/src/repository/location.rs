impl crate::Repository {
    /// The path to the `.git` directory itself, or equivalent if this is a bare repository.
    pub fn path(&self) -> &std::path::Path {
        self.git_dir()
    }

    /// Return the work tree containing all checked out files, if there is one.
    pub fn work_dir(&self) -> Option<&std::path::Path> {
        self.work_tree.as_deref()
    }

    // TODO: tests, respect precomposeUnicode
    /// The directory of the binary path of the current process.
    pub fn install_dir(&self) -> std::io::Result<std::path::PathBuf> {
        crate::path::install_dir()
    }

    /// Returns the relative path which is the components between the working tree and the current working dir (CWD).
    /// Note that there may be `None` if there is no work tree, even though the `PathBuf` will be empty
    /// if the CWD is at the root of the work tree.
    // TODO: tests, details - there is a lot about environment variables to change things around.
    pub fn prefix(&self) -> Option<std::io::Result<std::path::PathBuf>> {
        self.work_tree.as_ref().map(|root| {
            root.canonicalize().and_then(|root| {
                std::env::current_dir().and_then(|cwd| {
                    cwd.strip_prefix(&root)
                        .map_err(|_| {
                            std::io::Error::new(
                                std::io::ErrorKind::Other,
                                format!(
                                    "CWD '{}' isn't within the work tree '{}'",
                                    cwd.display(),
                                    root.display()
                                ),
                            )
                        })
                        .map(ToOwned::to_owned)
                })
            })
        })
    }

    /// Return the kind of repository, either bare or one with a work tree.
    pub fn kind(&self) -> crate::Kind {
        match self.work_tree {
            Some(_) => crate::Kind::WorkTree,
            None => crate::Kind::Bare,
        }
    }

    /// Return the path to the repository itself, containing objects, references, configuration, and more.
    ///
    /// Synonymous to [`path()`][crate::Repository::path()].
    pub fn git_dir(&self) -> &std::path::Path {
        self.refs.base()
    }
}
