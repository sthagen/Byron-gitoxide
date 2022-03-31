//!
use std::{convert::TryInto, ops::Deref};

use git_hash::{oid, ObjectId};

use crate::{object::find, Id, Object};

/// An [object id][ObjectId] infused with `Easy`.
impl<'repo> Id<'repo> {
    /// Find the [`Object`] associated with this object id, and consider it an error if it doesn't exist.
    ///
    /// # Note
    ///
    /// There can only be one `ObjectRef` per `Easy`. To increase that limit, clone the `Easy`.
    pub fn object(&self) -> Result<Object<'repo>, find::existing::OdbError> {
        self.repo.find_object(self.inner)
    }

    /// Try to find the [`Object`] associated with this object id, and return `None` if it's not available locally.
    ///
    /// # Note
    ///
    /// There can only be one `ObjectRef` per `Easy`. To increase that limit, clone the `Easy`.
    pub fn try_object(&self) -> Result<Option<Object<'repo>>, find::OdbError> {
        self.repo.try_find_object(self.inner)
    }

    /// Turn this object id into a shortened id with a length in hex as configured by `core.abbrev`.
    pub fn shorten(&self) -> Result<git_hash::Prefix, shorten::Error> {
        let hex_len = self.repo.config_int("core.abbrev", 7);
        let hex_len = hex_len.try_into().map_err(|_| shorten::Error::ConfigValue {
            actual: hex_len,
            max_range: self.inner.kind().len_in_hex(),
            err: None,
        })?;
        let prefix =
            git_odb::find::PotentialPrefix::new(self.inner, hex_len).map_err(|err| shorten::Error::ConfigValue {
                actual: hex_len as i64,
                max_range: self.inner.kind().len_in_hex(),
                err: Some(err),
            })?;
        Ok(self
            .repo
            .objects
            .disambiguate_prefix(prefix)
            .map_err(crate::object::find::existing::OdbError::Find)?
            .ok_or(crate::object::find::existing::OdbError::NotFound { oid: self.inner })?)
    }
}

///
pub mod shorten {
    /// Returned by [`Id::prefix()`][super::Id::shorten()].
    #[derive(thiserror::Error, Debug)]
    #[allow(missing_docs)]
    pub enum Error {
        #[error(transparent)]
        FindExisting(#[from] crate::object::find::existing::OdbError),
        #[error("core.abbrev length was {}, but needs to be between 4 and {}", .actual, .max_range)]
        ConfigValue {
            #[source]
            err: Option<git_hash::prefix::Error>,
            actual: i64,
            max_range: usize,
        },
    }
}

impl<'repo> Deref for Id<'repo> {
    type Target = oid;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<'repo> Id<'repo> {
    pub(crate) fn from_id(id: impl Into<ObjectId>, repo: &'repo crate::Repository) -> Self {
        Id { inner: id.into(), repo }
    }

    /// Turn this instance into its bare [ObjectId].
    pub fn detach(self) -> ObjectId {
        self.inner
    }
}

/// A platform to traverse commit ancestors, also referred to as commit history.
pub struct Ancestors<'repo> {
    repo: &'repo crate::Repository,
    tips: Box<dyn Iterator<Item = ObjectId>>,
    sorting: git_traverse::commit::Sorting,
    parents: git_traverse::commit::Parents,
}

///
pub mod ancestors {
    use git_odb::FindExt;

    use crate::{ext::ObjectIdExt, id::Ancestors, Id};

    impl<'repo> Id<'repo> {
        /// Obtain a platform for traversing ancestors of this commit.
        pub fn ancestors(&self) -> Ancestors<'repo> {
            Ancestors {
                repo: self.repo,
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

        /// Return an iterator to traverse all commits in the history of the commit the parent [Id] is pointing to.
        pub fn all(&mut self) -> Iter<'repo> {
            let tips = std::mem::replace(&mut self.tips, Box::new(None.into_iter()));
            let parents = self.parents;
            let sorting = self.sorting;
            let repo = self.repo;
            Iter {
                repo,
                inner: Box::new(
                    git_traverse::commit::Ancestors::new(
                        tips,
                        git_traverse::commit::ancestors::State::default(),
                        move |oid, buf| repo.objects.find_commit_iter(oid, buf),
                    )
                    .sorting(sorting)
                    .parents(parents),
                ),
            }
        }
    }

    /// The iterator returned by [`Ancestors::all()`].
    pub struct Iter<'repo> {
        repo: &'repo crate::Repository,
        inner: Box<dyn Iterator<Item = Result<git_hash::ObjectId, git_traverse::commit::ancestors::Error>> + 'repo>,
    }

    impl<'repo> Iterator for Iter<'repo> {
        type Item = Result<Id<'repo>, git_traverse::commit::ancestors::Error>;

        fn next(&mut self) -> Option<Self::Item> {
            self.inner.next().map(|res| res.map(|oid| oid.attach(self.repo)))
        }
    }
}

mod impls {
    use std::{cmp::Ordering, hash::Hasher};

    use git_hash::{oid, ObjectId};

    use crate::{DetachedObject, Id, Object};

    // Eq, Hash, Ord, PartialOrd,

    impl<'a> std::hash::Hash for Id<'a> {
        fn hash<H: Hasher>(&self, state: &mut H) {
            self.inner.hash(state)
        }
    }

    impl<'a> PartialOrd<Id<'a>> for Id<'a> {
        fn partial_cmp(&self, other: &Id<'a>) -> Option<Ordering> {
            self.inner.partial_cmp(&other.inner)
        }
    }

    impl<'repo> PartialEq<Id<'repo>> for Id<'repo> {
        fn eq(&self, other: &Id<'repo>) -> bool {
            self.inner == other.inner
        }
    }

    impl<'repo> PartialEq<ObjectId> for Id<'repo> {
        fn eq(&self, other: &ObjectId) -> bool {
            &self.inner == other
        }
    }

    impl<'repo> PartialEq<oid> for Id<'repo> {
        fn eq(&self, other: &oid) -> bool {
            self.inner == other
        }
    }

    impl<'repo> PartialEq<Object<'repo>> for Id<'repo> {
        fn eq(&self, other: &Object<'repo>) -> bool {
            self.inner == other.id
        }
    }

    impl<'repo> PartialEq<DetachedObject> for Id<'repo> {
        fn eq(&self, other: &DetachedObject) -> bool {
            self.inner == other.id
        }
    }

    impl<'repo> std::fmt::Debug for Id<'repo> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            self.inner.fmt(f)
        }
    }

    impl<'repo> AsRef<oid> for Id<'repo> {
        fn as_ref(&self) -> &oid {
            &self.inner
        }
    }

    impl<'repo> From<Id<'repo>> for ObjectId {
        fn from(v: Id<'repo>) -> Self {
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
            std::mem::size_of::<Id<'_>>(),
            32,
            "size of oid shouldn't change without notice"
        )
    }
}
