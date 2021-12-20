//!
use std::ops::Deref;

use git_hash::{oid, ObjectId};

use crate::{
    easy,
    easy::{object::find, Object, Oid},
};

/// An [object id][ObjectId] infused with `Easy`.
impl<'repo> Oid<'repo> {
    /// Find the [`Object`] associated with this object id, and consider it an error if it doesn't exist.
    ///
    /// # Note
    ///
    /// There can only be one `ObjectRef` per `Easy`. To increase that limit, clone the `Easy`.
    pub fn object(&self) -> Result<Object<'repo>, find::existing::OdbError> {
        self.handle.find_object(self.inner)
    }

    /// Try to find the [`Object`] associated with this object id, and return `None` if it's not available locally.
    ///
    /// # Note
    ///
    /// There can only be one `ObjectRef` per `Easy`. To increase that limit, clone the `Easy`.
    pub fn try_object(&self) -> Result<Option<Object<'repo>>, find::OdbError> {
        self.handle.try_find_object(self.inner)
    }
}

impl<'repo> Deref for Oid<'repo> {
    type Target = oid;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<'repo> Oid<'repo> {
    pub(crate) fn from_id(id: impl Into<ObjectId>, handle: &'repo easy::Handle) -> Self {
        Oid {
            inner: id.into(),
            handle,
        }
    }

    /// Turn this instance into its bare [ObjectId].
    pub fn detach(self) -> ObjectId {
        self.inner
    }
}

/// A platform to traverse commit ancestors, also referred to as commit history.
pub struct Ancestors<'repo> {
    handle: &'repo easy::Handle,
    tips: Box<dyn Iterator<Item = ObjectId>>,
    sorting: git_traverse::commit::Sorting,
    parents: git_traverse::commit::Parents,
}

///
pub mod ancestors {
    use git_odb::Find;

    use crate::{
        easy,
        easy::{oid::Ancestors, Oid},
        ext::ObjectIdExt,
    };

    impl<'repo> Oid<'repo> {
        /// Obtain a platform for traversing ancestors of this commit.
        pub fn ancestors(&self) -> Ancestors<'repo> {
            Ancestors {
                handle: self.handle,
                tips: Box::new(Some(self.inner).into_iter()),
                sorting: Default::default(),
                parents: Default::default(),
            }
        }
    }

    impl<'repo> Ancestors<'repo> {
        /// Set the sort mode for commits to the given value. The default is to order by topology.
        pub fn sorting(mut self, sorting: git_traverse::commit::Sorting) -> Self {
            self.sorting = sorting;
            self
        }

        /// Only traverse the first parent of the commit graph.
        pub fn first_parent_only(mut self) -> Self {
            self.parents = git_traverse::commit::Parents::First;
            self
        }

        /// Return an iterator to traverse all commits in the history of the commit the parent [Oid] is pointing to.
        pub fn all(&mut self) -> Iter<'_, 'repo> {
            let tips = std::mem::replace(&mut self.tips, Box::new(None.into_iter()));
            let parents = self.parents;
            let sorting = self.sorting;
            Iter {
                handle: self.handle,
                inner: Box::new(
                    git_traverse::commit::Ancestors::new(
                        tips,
                        git_traverse::commit::ancestors::State::default(),
                        move |oid, buf| {
                            self.handle
                                .objects
                                .try_find(oid, buf)
                                .ok()
                                .flatten()
                                .and_then(|obj| obj.try_into_commit_iter())
                        },
                    )
                    .sorting(sorting)
                    .parents(parents),
                ),
            }
        }
    }

    /// The iterator returned by [`Ancestors::all()`].
    pub struct Iter<'a, 'repo> {
        handle: &'repo easy::Handle,
        inner: Box<dyn Iterator<Item = Result<git_hash::ObjectId, git_traverse::commit::ancestors::Error>> + 'a>,
    }

    impl<'a, 'repo> Iterator for Iter<'a, 'repo> {
        type Item = Result<Oid<'repo>, git_traverse::commit::ancestors::Error>;

        fn next(&mut self) -> Option<Self::Item> {
            self.inner.next().map(|res| res.map(|oid| oid.attach(self.handle)))
        }
    }
}

mod impls {
    use std::{cmp::Ordering, hash::Hasher};

    use git_hash::{oid, ObjectId};

    use crate::easy::{DetachedObject, Object, Oid};
    // Eq, Hash, Ord, PartialOrd,

    impl<'a> std::hash::Hash for Oid<'a> {
        fn hash<H: Hasher>(&self, state: &mut H) {
            self.inner.hash(state)
        }
    }

    impl<'a> PartialOrd<Oid<'a>> for Oid<'a> {
        fn partial_cmp(&self, other: &Oid<'a>) -> Option<Ordering> {
            self.inner.partial_cmp(&other.inner)
        }
    }

    impl<'repo> PartialEq<Oid<'repo>> for Oid<'repo> {
        fn eq(&self, other: &Oid<'repo>) -> bool {
            self.inner == other.inner
        }
    }

    impl<'repo> PartialEq<ObjectId> for Oid<'repo> {
        fn eq(&self, other: &ObjectId) -> bool {
            &self.inner == other
        }
    }

    impl<'repo> PartialEq<oid> for Oid<'repo> {
        fn eq(&self, other: &oid) -> bool {
            self.inner == other
        }
    }

    impl<'repo> PartialEq<Object<'repo>> for Oid<'repo> {
        fn eq(&self, other: &Object<'repo>) -> bool {
            self.inner == other.id
        }
    }

    impl<'repo> PartialEq<DetachedObject> for Oid<'repo> {
        fn eq(&self, other: &DetachedObject) -> bool {
            self.inner == other.id
        }
    }

    impl<'repo> std::fmt::Debug for Oid<'repo> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            self.inner.fmt(f)
        }
    }

    impl<'repo> AsRef<oid> for Oid<'repo> {
        fn as_ref(&self) -> &oid {
            &self.inner
        }
    }

    impl<'repo> From<Oid<'repo>> for ObjectId {
        fn from(v: Oid<'repo>) -> Self {
            v.inner
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_of_oid() {
        assert_eq!(
            std::mem::size_of::<Oid<'_>>(),
            32,
            "size of oid shouldn't change without notice"
        )
    }
}
