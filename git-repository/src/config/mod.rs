use crate::{bstr::BString, permission, Repository};

mod cache;
mod snapshot;

/// A platform to access configuration values as read from disk.
///
/// Note that these values won't update even if the underlying file(s) change.
pub struct Snapshot<'repo> {
    pub(crate) repo: &'repo Repository,
}

pub(crate) mod section {
    pub fn is_trusted(meta: &git_config::file::Metadata) -> bool {
        meta.trust == git_sec::Trust::Full || !meta.source.is_in_repository()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Could not open repository conifguration file")]
    Open(#[from] git_config::file::init::from_paths::Error),
    #[error("Cannot handle objects formatted as {:?}", .name)]
    UnsupportedObjectFormat { name: BString },
    #[error("The value for '{}' cannot be empty", .key)]
    EmptyValue { key: &'static str },
    #[error("Invalid value for 'core.abbrev' = '{}'. It must be between 4 and {}", .value, .max)]
    CoreAbbrev { value: BString, max: u8 },
    #[error("Value '{}' at key '{}' could not be decoded as boolean", .value, .key)]
    DecodeBoolean { key: String, value: BString },
    #[error(transparent)]
    PathInterpolation(#[from] git_config::path::interpolate::Error),
}

/// Utility type to keep pre-obtained configuration values.
#[derive(Debug, Clone)]
pub(crate) struct Cache {
    pub resolved: crate::Config,
    /// The hex-length to assume when shortening object ids. If `None`, it should be computed based on the approximate object count.
    pub hex_len: Option<usize>,
    /// true if the repository is designated as 'bare', without work tree.
    pub is_bare: bool,
    /// The type of hash to use.
    pub object_hash: git_hash::Kind,
    /// If true, multi-pack indices, whether present or not, may be used by the object database.
    pub use_multi_pack_index: bool,
    /// The representation of `core.logallrefupdates`, or `None` if the variable wasn't set.
    pub reflog: Option<git_ref::store::WriteReflog>,
    /// If true, we are on a case-insensitive file system.
    #[cfg_attr(not(feature = "git-index"), allow(dead_code))]
    pub ignore_case: bool,
    /// The path to the user-level excludes file to ignore certain files in the worktree.
    #[cfg_attr(not(feature = "git-index"), allow(dead_code))]
    pub excludes_file: Option<std::path::PathBuf>,
    /// Define how we can use values obtained with `xdg_config(…)` and its `XDG_CONFIG_HOME` variable.
    #[cfg_attr(not(feature = "git-index"), allow(dead_code))]
    xdg_config_home_env: permission::env_var::Resource,
    /// Define how we can use values obtained with `xdg_config(…)`. and its `HOME` variable.
    home_env: permission::env_var::Resource,
    // TODO: make core.precomposeUnicode available as well.
}
