///
pub mod edit {
    /// The error returned by [edit_references(…)][crate::Repository::edit_references()], and others
    /// which ultimately create a reference.
    #[derive(Debug, thiserror::Error)]
    #[allow(missing_docs)]
    pub enum Error {
        #[error(transparent)]
        FileTransactionPrepare(#[from] git_ref::file::transaction::prepare::Error),
        #[error(transparent)]
        FileTransactionCommit(#[from] git_ref::file::transaction::commit::Error),
        #[error(transparent)]
        NameValidation(#[from] git_validate::reference::name::Error),
    }
}

///
pub mod peel {
    /// The error returned by [Reference::peel_to_id_in_place(…)][crate::Reference::peel_to_id_in_place()] and
    /// [Reference::into_fully_peeled_id(…)][crate::Reference::into_fully_peeled_id()].
    #[derive(Debug, thiserror::Error)]
    #[allow(missing_docs)]
    pub enum Error {
        #[error(transparent)]
        ToId(#[from] git_ref::peel::to_id::Error),
        #[error(transparent)]
        PackedRefsOpen(#[from] git_ref::packed::buffer::open::Error),
    }
}

///
pub mod find {
    ///
    pub mod existing {
        /// The error returned by [find_reference(…)][crate::Repository::find_reference()], and others.
        #[derive(Debug, thiserror::Error)]
        #[allow(missing_docs)]
        pub enum Error {
            #[error(transparent)]
            Find(#[from] crate::reference::find::Error),
            #[error("The reference did not exist even though that was expected")]
            NotFound,
        }
    }

    /// The error returned by [try_find_reference(…)][crate::Repository::try_find_reference()].
    #[derive(Debug, thiserror::Error)]
    #[allow(missing_docs)]
    pub enum Error {
        #[error(transparent)]
        Find(#[from] git_ref::file::find::Error),
        #[error(transparent)]
        PackedRefsOpen(#[from] git_ref::packed::buffer::open::Error),
    }
}
