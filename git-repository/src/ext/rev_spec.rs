pub trait Sealed {}

impl Sealed for git_ref::Reference {}

/// Extensions for [revision specifications][git_revision::Spec].
pub trait RevSpecExt {
    /// Attach [`Repository`][crate::Repository] to the given rev-spec.
    fn attach(self, repo: &crate::Repository) -> crate::revision::Spec<'_>;
}

impl RevSpecExt for git_revision::Spec {
    fn attach(self, repo: &crate::Repository) -> crate::revision::Spec<'_> {
        crate::revision::Spec {
            inner: self,
            first_ref: None,
            second_ref: None,
            repo,
        }
    }
}
