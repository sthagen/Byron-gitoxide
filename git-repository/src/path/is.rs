use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

/// The error returned by [`git()`].
#[derive(Debug, thiserror::Error)]
#[allow(missing_docs)]
pub enum Error {
    #[error("Could not find a valid HEAD reference")]
    FindHeadRef(#[from] git_ref::file::find::existing::Error),
    #[error("Expected HEAD at '.git/HEAD', got '.git/{}'", .name)]
    MisplacedHead { name: crate::bstr::BString },
    #[error("Expected an objects directory at '{}'", .missing.display())]
    MissingObjectsDirectory { missing: PathBuf },
    #[error("Expected a refs directory at '{}'", .missing.display())]
    MissingRefsDirectory { missing: PathBuf },
}

/// Returns true if the given `git_dir` seems to be a bare repository.
///
/// Please note that repositories without any file in their work tree will also appear bare.
pub fn bare(git_dir_candidate: impl AsRef<Path>) -> bool {
    let git_dir = git_dir_candidate.as_ref();
    !(git_dir.join("index").exists() || (git_dir.file_name() == Some(OsStr::new(".git")) && git_dir.is_file()))
}

/// What constitutes a valid git repository, and what's yet to be implemented, returning the guessed repository kind
/// purely based on the presence of files. Note that the git-config ultimately decides what's bare.
///
/// * [ ] git files
/// * [x] a valid head
/// * [ ] git common directory
///   * [ ] respect GIT_COMMON_DIR
/// * [x] an objects directory
///   * [x] respect GIT_OBJECT_DIRECTORY
/// * [x] a refs directory
pub fn git(git_dir: impl AsRef<Path>) -> Result<crate::Kind, Error> {
    let dot_git = git_dir.as_ref();

    {
        // We expect to be able to parse any ref-hash, so we shouldn't have to know the repos hash here.
        // With ref-table, the has is probably stored as part of the ref-db itself, so we can handle it from there.
        // In other words, it's important not to fail on detached heads here because we guessed the hash kind wrongly.
        let object_hash_should_not_matter_here = git_hash::Kind::Sha1;
        let refs = crate::RefStore::at(
            &dot_git,
            git_ref::store::WriteReflog::Normal,
            object_hash_should_not_matter_here,
        );
        let head = refs.find_loose("HEAD")?;
        if head.name.as_bstr() != "HEAD" {
            return Err(Error::MisplacedHead {
                name: head.name.into_inner(),
            });
        }
    }

    {
        let objects_path = std::env::var("GIT_OBJECT_DIRECTORY")
            .map(PathBuf::from)
            .unwrap_or_else(|_| dot_git.join("objects"));
        if !objects_path.is_dir() {
            return Err(Error::MissingObjectsDirectory { missing: objects_path });
        }
    }
    {
        let refs_path = dot_git.join("refs");
        if !refs_path.is_dir() {
            return Err(Error::MissingRefsDirectory { missing: refs_path });
        }
    }

    Ok(if bare(git_dir) {
        crate::Kind::Bare
    } else {
        crate::Kind::WorkTree
    })
}
