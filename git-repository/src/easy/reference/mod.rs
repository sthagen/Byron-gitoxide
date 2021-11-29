//!
use std::ops::DerefMut;

use git_odb::Find;
use git_ref::file::ReferenceExt;

use crate::{
    easy,
    easy::{Oid, Reference},
};

pub mod iter;

mod errors;

pub use errors::{edit, find, peel};

use crate::ext::ObjectIdExt;

pub mod logs;

/// Access
impl<'repo, A> Reference<'repo, A> {
    /// Return the target to which this reference points to.
    pub fn target(&self) -> git_ref::TargetRef<'_> {
        self.inner.target.to_ref()
    }

    /// Return the reference's full name.
    pub fn name(&self) -> git_ref::FullNameRef<'_> {
        self.inner.name.to_ref()
    }

    /// Turn this instances into a stand-alone reference.
    pub fn detach(self) -> git_ref::Reference {
        self.inner
    }
}

impl<'repo, A> Reference<'repo, A>
where
    A: easy::Access + Sized,
{
    pub(crate) fn from_ref(reference: git_ref::Reference, access: &'repo A) -> Self {
        Reference {
            inner: reference,
            access,
        }
    }

    /// Returns the attached id we point to, or panic if this is a symbolic ref.
    pub fn id(&self) -> easy::Oid<'repo, A> {
        match self.inner.target {
            git_ref::Target::Symbolic(_) => panic!("BUG: tries to obtain object id from symbolic target"),
            git_ref::Target::Peeled(oid) => oid.to_owned().attach(self.access),
        }
    }

    /// Follow all symbolic targets this reference might point to and peel the underlying object
    /// to the end of the chain, and return it.
    ///
    /// This is useful to learn where this reference is ulitmately pointing to.
    pub fn peel_to_id_in_place(&mut self) -> Result<Oid<'repo, A>, peel::Error> {
        let repo = self.access.repo()?;
        let state = self.access.state();
        let mut pack_cache = state.try_borrow_mut_pack_cache()?;
        let oid = self.inner.peel_to_id_in_place(&state.refs, |oid, buf| {
            repo.odb
                .try_find(oid, buf, pack_cache.deref_mut())
                .map(|po| po.map(|o| (o.kind, o.data)))
        })?;
        Ok(Oid::from_id(oid, self.access))
    }

    /// Similar to [`peel_to_id_in_place()`][Reference::peel_to_id_in_place()], but consumes this instance.
    pub fn into_fully_peeled_id(mut self) -> Result<Oid<'repo, A>, peel::Error> {
        self.peel_to_id_in_place()
    }
}
