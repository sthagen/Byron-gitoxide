//!

use git_odb::pack::Find;
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
impl<'repo> Reference<'repo> {
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

impl<'repo> Reference<'repo> {
    pub(crate) fn from_ref(reference: git_ref::Reference, handle: &'repo easy::Handle) -> Self {
        Reference {
            inner: reference,
            handle,
        }
    }

    /// Returns the attached id we point to, or panic if this is a symbolic ref.
    pub fn id(&self) -> easy::Oid<'repo> {
        match self.inner.target {
            git_ref::Target::Symbolic(_) => panic!("BUG: tries to obtain object id from symbolic target"),
            git_ref::Target::Peeled(oid) => oid.to_owned().attach(self.handle),
        }
    }

    /// Follow all symbolic targets this reference might point to and peel the underlying object
    /// to the end of the chain, and return it.
    ///
    /// This is useful to learn where this reference is ulitmately pointing to.
    pub fn peel_to_id_in_place(&mut self) -> Result<Oid<'repo>, peel::Error> {
        let handle = &self.handle;
        let oid = self.inner.peel_to_id_in_place(&handle.refs, |oid, buf| {
            handle
                .objects
                .try_find(oid, buf)
                .map(|po| po.map(|(o, _l)| (o.kind, o.data)))
        })?;
        Ok(Oid::from_id(oid, handle))
    }

    /// Similar to [`peel_to_id_in_place()`][Reference::peel_to_id_in_place()], but consumes this instance.
    pub fn into_fully_peeled_id(mut self) -> Result<Oid<'repo>, peel::Error> {
        self.peel_to_id_in_place()
    }
}
