use git_features::fs;

use crate::store::loose::Store;

/// Returned by [`Store::iter()`]
#[derive(thiserror::Error, Debug)]
#[allow(missing_docs)]
pub enum Error {
    #[error(transparent)]
    WalkDir(#[from] fs::walkdir::Error),
}

/// The type for an iterator over `Result<git_hash::ObjectId, Error>)`
pub type Iter = std::iter::FilterMap<
    fs::walkdir::DirEntryIter,
    fn(Result<fs::walkdir::DirEntry, fs::walkdir::Error>) -> Option<Result<git_hash::ObjectId, Error>>,
>;

/// Iteration and traversal
impl Store {
    fn iter_filter_map(
        res: Result<fs::walkdir::DirEntry, fs::walkdir::Error>,
    ) -> Option<Result<git_hash::ObjectId, Error>> {
        use std::path::Component::Normal;

        let mut is_valid_path = false;
        let e = res.map_err(Error::WalkDir).map(|e| {
            let p = e.path();
            let mut ci = p.components();
            let (c2, c1) = (ci.next_back(), ci.next_back());
            if let (Some(Normal(c1)), Some(Normal(c2))) = (c1, c2) {
                if c1.len() == 2 && c2.len() == 38 {
                    if let (Some(c1), Some(c2)) = (c1.to_str(), c2.to_str()) {
                        let mut buf = [0u8; 40];
                        {
                            let (first_byte, rest) = buf.split_at_mut(2);
                            first_byte.copy_from_slice(c1.as_bytes());
                            rest.copy_from_slice(c2.as_bytes());
                        }
                        if let Ok(b) = git_hash::ObjectId::from_hex(&buf[..]) {
                            is_valid_path = true;
                            return b;
                        }
                    }
                }
            }
            git_hash::ObjectId::null_sha1()
        });
        is_valid_path.then(|| e)
    }

    /// Return an iterator over all objects contained in the database.
    ///
    /// The [`Id`][git_hash::ObjectId]s returned by the iterator can typically be used in the [`locate(…)`][Store::try_find()] method.
    /// _Note_ that the result is not sorted or stable, thus ordering can change between runs.
    ///
    /// # Notes
    ///
    /// [`Iter`] is used instead of `impl Iterator<…>` to allow using this iterator in struct fields, as is currently
    /// needed if iterators need to be implemented by hand in the absence of generators.
    pub fn iter(&self) -> Iter {
        fs::walkdir_new(&self.path)
            .min_depth(2)
            .max_depth(3)
            .follow_links(false)
            .into_iter()
            .filter_map(Store::iter_filter_map)
    }
}
