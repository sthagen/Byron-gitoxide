use crate::easy;

/// The catch-all of extension traits.
impl easy::Handle {
    // TODO: actual implementation
    /// Return the committer as configured by this repository, which is determined by…
    ///
    /// * …the git configuration…
    /// * …the GIT_(AUTHOR|COMMITTER)_(NAME|EMAIL|DATE) environment variables…
    ///
    /// …and in that order.
    pub fn committer(&self) -> git_actor::Signature {
        // TODO: actually do the work, probably that should be cached and be refreshable
        git_actor::Signature::empty()
    }

    /// The kind of object hash the repository is configured to use.
    pub fn object_hash(&self) -> git_hash::Kind {
        self.object_hash
    }
}
