use git_hash::ObjectId;
use git_object::{bstr::BStr, TreeRefIter};

use crate::{
    easy,
    easy::{ext::ObjectAccessExt, object::find, TreeRef},
};

impl<'repo, A> TreeRef<'repo, A>
where
    A: easy::Access + Sized,
{
    /// Obtain a tree instance by handing in all components that it is made up of.
    pub fn from_id_and_data(id: impl Into<ObjectId>, data: std::cell::Ref<'repo, [u8]>, access: &'repo A) -> Self {
        TreeRef {
            id: id.into(),
            data,
            access,
        }
    }
    // TODO: move implementation to git-object, tests.
    /// Follow a sequence of `path` components starting from this instance, and look them up one by one until the last component
    /// is looked up and its tree entry is returned.
    ///
    /// # Performance Notes
    ///
    /// Searching tree entries is currently done in sequence, which allows to the search to be allocation free. It would be possible
    /// to re-use a vector and use a binary search instead, which might be able to improve performance over all.
    /// However, a benchmark should be created first to have some data and see which trade-off to choose here.
    pub fn lookup_path<I, P>(mut self, path: I) -> Result<Option<git_object::tree::Entry>, find::existing::Error>
    where
        I: IntoIterator<Item = P>,
        P: PartialEq<BStr>,
    {
        // let mut out = None;
        let mut path = path.into_iter().peekable();
        while let Some(component) = path.next() {
            match TreeRefIter::from_bytes(&self.data)
                .filter_map(Result::ok)
                .find(|entry| component.eq(entry.filename))
            {
                Some(entry) => {
                    if path.peek().is_none() {
                        return Ok(Some(entry.into()));
                    } else {
                        let next_id = entry.oid.to_owned();
                        let access = self.access;
                        drop(entry);
                        drop(self);
                        self = match access.find_object(next_id)?.try_into_tree() {
                            Ok(tree) => tree,
                            Err(_) => return Ok(None),
                        };
                    }
                }
                None => return Ok(None),
            }
        }
        Ok(None)
    }
}
