use std::{convert::TryFrom, path::PathBuf};

use git_config::{Boolean, Integer};

use super::{Cache, Error};
use crate::config::section::is_trusted;
use crate::{bstr::ByteSlice, permission};

impl Cache {
    pub fn new(
        git_dir_trust: git_sec::Trust,
        git_dir: &std::path::Path,
        xdg_config_home_env: permission::env_var::Resource,
        home_env: permission::env_var::Resource,
        git_install_dir: Option<&std::path::Path>,
    ) -> Result<Self, Error> {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .and_then(|home| home_env.check(home).ok().flatten());
        // TODO: don't forget to use the canonicalized home for initializing the stacked config.
        //       like git here: https://github.com/git/git/blob/master/config.c#L208:L208
        let config = {
            let mut buf = Vec::with_capacity(512);
            git_config::File::from_path_with_buf(
                &git_dir.join("config"),
                &mut buf,
                git_config::file::Metadata::from(git_config::Source::Local).with(git_dir_trust),
                git_config::file::init::Options {
                    lossy: !cfg!(debug_assertions),
                    includes: git_config::file::init::includes::Options::follow(
                        interpolate_context(git_install_dir, home.as_deref()),
                        git_config::file::init::includes::conditional::Context {
                            git_dir: git_dir.into(),
                            branch_name: None,
                        },
                    ),
                },
            )?
        };

        let is_bare = config_bool(&config, "core.bare", false)?;
        let use_multi_pack_index = config_bool(&config, "core.multiPackIndex", true)?;
        let ignore_case = config_bool(&config, "core.ignoreCase", false)?;
        let excludes_file = config
            .path_filter("core", None, "excludesFile", &mut is_trusted)
            .map(|p| {
                p.interpolate(interpolate_context(git_install_dir, home.as_deref()))
                    .map(|p| p.into_owned())
            })
            .transpose()?;
        let repo_format_version = config
            .value::<Integer>("core", None, "repositoryFormatVersion")
            .map_or(0, |v| v.to_decimal().unwrap_or_default());
        let object_hash = (repo_format_version != 1)
            .then(|| Ok(git_hash::Kind::Sha1))
            .or_else(|| {
                config.string("extensions", None, "objectFormat").map(|format| {
                    if format.as_ref().eq_ignore_ascii_case(b"sha1") {
                        Ok(git_hash::Kind::Sha1)
                    } else {
                        Err(Error::UnsupportedObjectFormat {
                            name: format.to_vec().into(),
                        })
                    }
                })
            })
            .transpose()?
            .unwrap_or(git_hash::Kind::Sha1);
        let reflog = config.string("core", None, "logallrefupdates").map(|val| {
            (val.eq_ignore_ascii_case(b"always"))
                .then(|| git_ref::store::WriteReflog::Always)
                .or_else(|| {
                    git_config::Boolean::try_from(val)
                        .ok()
                        .and_then(|b| b.is_true().then(|| git_ref::store::WriteReflog::Normal))
                })
                .unwrap_or(git_ref::store::WriteReflog::Disable)
        });

        let mut hex_len = None;
        if let Some(hex_len_str) = config.string("core", None, "abbrev") {
            if hex_len_str.trim().is_empty() {
                return Err(Error::EmptyValue { key: "core.abbrev" });
            }
            if !hex_len_str.eq_ignore_ascii_case(b"auto") {
                let value_bytes = hex_len_str.as_ref();
                if let Ok(false) = Boolean::try_from(value_bytes).map(Into::into) {
                    hex_len = object_hash.len_in_hex().into();
                } else {
                    let value = Integer::try_from(value_bytes)
                        .map_err(|_| Error::CoreAbbrev {
                            value: hex_len_str.clone().into_owned(),
                            max: object_hash.len_in_hex() as u8,
                        })?
                        .to_decimal()
                        .ok_or_else(|| Error::CoreAbbrev {
                            value: hex_len_str.clone().into_owned(),
                            max: object_hash.len_in_hex() as u8,
                        })?;
                    if value < 4 || value as usize > object_hash.len_in_hex() {
                        return Err(Error::CoreAbbrev {
                            value: hex_len_str.clone().into_owned(),
                            max: object_hash.len_in_hex() as u8,
                        });
                    }
                    hex_len = Some(value as usize);
                }
            }
        }

        Ok(Cache {
            resolved: config.into(),
            use_multi_pack_index,
            object_hash,
            reflog,
            is_bare,
            ignore_case,
            hex_len,
            excludes_file,
            xdg_config_home_env,
            home_env,
        })
    }

    /// Return a path by using the `$XDF_CONFIG_HOME` or `$HOME/.config/…` environment variables locations.
    #[cfg_attr(not(feature = "git-index"), allow(dead_code))]
    pub fn xdg_config_path(
        &self,
        resource_file_name: &str,
    ) -> Result<Option<PathBuf>, git_sec::permission::Error<PathBuf, git_sec::Permission>> {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(|path| (path, &self.xdg_config_home_env))
            .or_else(|| std::env::var_os("HOME").map(|path| (path, &self.home_env)))
            .and_then(|(base, permission)| {
                let resource = std::path::PathBuf::from(base).join("git").join(resource_file_name);
                permission.check(resource).transpose()
            })
            .transpose()
    }

    /// Return the home directory if we are allowed to read it and if it is set in the environment.
    ///
    /// We never fail for here even if the permission is set to deny as we `git-config` will fail later
    /// if it actually wants to use the home directory - we don't want to fail prematurely.
    pub fn home_dir(&self) -> Option<PathBuf> {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .and_then(|path| self.home_env.check(path).ok().flatten())
    }
}

pub(crate) fn interpolate_context<'a>(
    git_install_dir: Option<&'a std::path::Path>,
    home_dir: Option<&'a std::path::Path>,
) -> git_config::path::interpolate::Context<'a> {
    git_config::path::interpolate::Context {
        git_install_dir,
        home_dir,
        home_for_user: Some(git_config::path::interpolate::home_for_user), // TODO: figure out how to configure this
    }
}

fn config_bool(config: &git_config::File<'_>, key: &str, default: bool) -> Result<bool, Error> {
    let (section, key) = key.split_once('.').expect("valid section.key format");
    config
        .boolean(section, None, key)
        .unwrap_or(Ok(default))
        .map_err(|err| Error::DecodeBoolean {
            value: err.input,
            key: key.into(),
        })
}
