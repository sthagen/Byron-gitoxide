use std::path::PathBuf;

/// the error returned by [`realpath()`][super::realpath()].
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("The maximum allowed number {} of symlinks in path is exceeded", .max_symlinks)]
    MaxSymlinksExceeded { max_symlinks: u8 },
    #[error(transparent)]
    ReadLink(#[from] std::io::Error),
    #[error("Empty is not a valid path")]
    EmptyPath,
    #[error("Parent component of {} does not exist, {}", .path.display(), .msg)]
    MissingParent { path: PathBuf, msg: &'static str },
}

pub(crate) mod function {
    use super::Error;
    use std::path::Component::{CurDir, Normal, ParentDir, Prefix, RootDir};
    use std::path::{Path, PathBuf};

    /// TODO
    pub fn realpath(path: impl AsRef<Path>, cwd: impl AsRef<Path>, max_symlinks: u8) -> Result<PathBuf, Error> {
        let path = path.as_ref();
        if path.as_os_str().is_empty() {
            return Err(Error::EmptyPath);
        }

        let mut real_path = PathBuf::new();
        if path.is_relative() {
            real_path.push(cwd);
        }

        let mut num_symlinks = 0;
        let mut path_backing: PathBuf;
        let mut components = path.components();
        while let Some(component) = components.next() {
            match component {
                part @ RootDir | part @ Prefix(_) => real_path.push(part),
                CurDir => {}
                ParentDir => {
                    if !real_path.pop() {
                        return Err(Error::MissingParent {
                            path: real_path,
                            msg: "parent path must exist",
                        });
                    }
                }
                Normal(part) => {
                    real_path.push(part);
                    if real_path.is_symlink() {
                        num_symlinks += 1;
                        if num_symlinks > max_symlinks {
                            return Err(Error::MaxSymlinksExceeded { max_symlinks });
                        }
                        let mut link_destination = std::fs::read_link(real_path.as_path())?;
                        if link_destination.is_absolute() {
                            // pushing absolute path to real_path resets it to the pushed absolute path
                            // real_path.clear();
                        } else if !real_path.pop() {
                            return Err(Error::MissingParent {
                                path: real_path,
                                msg: "we just pushed a component",
                            });
                        }
                        link_destination.extend(components);
                        path_backing = link_destination;
                        components = path_backing.components();
                    }
                }
            }
        }
        Ok(real_path)
    }
}
