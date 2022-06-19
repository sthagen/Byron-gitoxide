use bstr::BString;
use std::convert::TryFrom;
use std::fs;
use std::fs::create_dir_all;
use std::path::{Path, PathBuf};

use crate::git_config::cow_str;
use crate::git_config::from_paths::escape_backslashes;
use git_config::file::from_paths;
use git_config::File;
use git_ref::FullName;
use tempfile::{tempdir, tempdir_in};

pub struct CanonicalizedTempDir {
    pub dir: tempfile::TempDir,
}

pub fn create_symlink(from: &Path, to: &Path) {
    create_dir_all(from.parent().unwrap()).unwrap();
    #[cfg(not(target_os = "windows"))]
    std::os::unix::fs::symlink(to, &from).unwrap();
    #[cfg(target_os = "windows")]
    std::os::windows::fs::symlink_file(to, &from).unwrap();
}

impl CanonicalizedTempDir {
    pub fn new() -> Self {
        #[cfg(windows)]
        let canonicalized_tempdir = std::env::temp_dir();
        #[cfg(not(windows))]
        let canonicalized_tempdir = std::env::temp_dir().canonicalize().unwrap();
        let dir = tempdir_in(canonicalized_tempdir).unwrap();
        Self { dir }
    }
}

impl Default for CanonicalizedTempDir {
    fn default() -> Self {
        Self::new()
    }
}

impl AsRef<Path> for CanonicalizedTempDir {
    fn as_ref(&self) -> &Path {
        self
    }
}

impl std::ops::Deref for CanonicalizedTempDir {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        self.dir.path()
    }
}

#[test]
fn girdir_and_onbranch() {
    let dir = tempdir().unwrap();

    let config_path = dir.path().join("a");
    let absolute_path = dir.path().join("b");
    let home_dot_git_path = dir.path().join("c");
    let home_trailing_slash_path = dir.path().join("c_slash");
    let relative_dot_git_path2 = dir.path().join("d");
    let relative_path = dir.path().join("e");
    let casei_path = dir.path().join("i");
    let relative_dot_slash_path = dir.path().join("g");
    let relative_dot_git_path = dir.path().join("w");
    let relative_with_backslash_path = dir.path().join("x");
    let branch_path = dir.path().join("branch");
    let tmp_path = dir.path().join("tmp");
    let tmp_dir_m_n_with_slash = format!(
        "{}/",
        CanonicalizedTempDir::new()
            .join("m")
            .join("n")
            .to_str()
            .unwrap()
            .replace('\\', "/")
    );

    fs::write(
        config_path.as_path(),
        format!(
            r#"
[core]
  x = 1
  a = 1
  b = 1
  c = 1
  i = 1
  t = 1
[includeIf "onbranch:/br/"]
  path = {}
[includeIf "gitdir/i:a/B/c/D/"]
  path = {}
[includeIf "gitdir:c\\d"]
  path = {}
[includeIf "gitdir:./p/"]
  path = {}
[includeIf "gitdir:z/y/"]
  path = {}
[includeIf "gitdir:w/.git"]
  path = {}
[includeIf "gitdir:~/.git"]
  path = {}
[includeIf "gitdir:~/c/"]
  path = {}
[includeIf "gitdir:a/.git"]
  path = {}
[includeIf "gitdir:{}"]
  path = {}
[includeIf "gitdir:/e/x/"]
  path = {}"#,
            escape_backslashes(&branch_path),
            escape_backslashes(&casei_path),
            escape_backslashes(&relative_with_backslash_path),
            escape_backslashes(&relative_dot_slash_path),
            escape_backslashes(&relative_path),
            escape_backslashes(&relative_dot_git_path),
            escape_backslashes(&home_dot_git_path),
            escape_backslashes(&home_trailing_slash_path),
            escape_backslashes(&relative_dot_git_path2),
            &tmp_dir_m_n_with_slash,
            escape_backslashes(&tmp_path),
            escape_backslashes(&absolute_path),
        ),
    )
    .unwrap();

    fs::write(
        branch_path.as_path(),
        "
[core]
  x = branch-override",
    )
    .unwrap();

    fs::write(
        casei_path.as_path(),
        "
[core]
  i = case-i-match",
    )
    .unwrap();

    fs::write(
        relative_with_backslash_path.as_path(),
        "
[core]
  c = relative with backslash do not match",
    )
    .unwrap();

    fs::write(
        absolute_path.as_path(),
        "
[core]
  b = absolute-path",
    )
    .unwrap();

    fs::write(
        home_dot_git_path.as_path(),
        "
[core]
  b = home-dot-git",
    )
    .unwrap();

    fs::write(
        relative_dot_git_path2.as_path(),
        "
[core]
  b = relative-dot-git-2",
    )
    .unwrap();

    fs::write(
        relative_path.as_path(),
        "
[core]
  a = relative-path",
    )
    .unwrap();

    fs::write(
        relative_dot_git_path.as_path(),
        "
[core]
  a = relative-dot-git",
    )
    .unwrap();

    fs::write(
        home_trailing_slash_path.as_path(),
        "
[core]
  b = home-trailing-slash",
    )
    .unwrap();

    fs::write(
        relative_dot_slash_path.as_path(),
        "
[core]
  b = relative-dot-slash-path",
    )
    .unwrap();

    fs::write(
        tmp_path.as_path(),
        "
[core]
  t = absolute-path-with-symlink",
    )
    .unwrap();

    {
        let branch_name = FullName::try_from(BString::from("refs/heads/repo/br/one")).unwrap();
        let branch_name = branch_name.as_ref();
        let options = from_paths::Options {
            branch_name: Some(branch_name),
            ..Default::default()
        };

        let config = File::from_paths(Some(&config_path), options).unwrap();
        assert_eq!(
            config.string("core", None, "x"),
            Some(cow_str("branch-override")),
            "branch name match"
        );
    }

    {
        let dir = Path::new("/a/b/c/d/.git");
        let config = File::from_paths(Some(&config_path), options_with_git_dir(dir)).unwrap();
        assert_eq!(
            config.string("core", None, "i"),
            Some(cow_str("case-i-match")),
            "case insensitive patterns match"
        );
    }

    {
        let dir = Path::new("/a/b/c/d/.git");
        let config = File::from_paths(Some(&config_path), options_with_git_dir(dir)).unwrap();
        assert_eq!(
            config.integer("core", None, "c"),
            Some(Ok(1)),
            "patterns with backslashes do not match"
        );
    }

    {
        let dir = config_path.parent().unwrap().join("p").join("q").join(".git");
        let config = File::from_paths(Some(&config_path), options_with_git_dir(&dir)).unwrap();
        assert_eq!(
            config.string("core", None, "b"),
            Some(cow_str("relative-dot-slash-path")),
            "relative path pattern is matched correctly"
        );
    }

    {
        let dir = config_path.join("z").join("y").join("b").join(".git");
        let config = File::from_paths(Some(&config_path), options_with_git_dir(&dir)).unwrap();
        assert_eq!(
            config.string("core", None, "a"),
            Some(cow_str("relative-path")),
            "the pattern is prefixed and suffixed with ** to match GIT_DIR containing it in the middle"
        );
    }

    {
        let dir = PathBuf::from("C:\\w\\.git".to_string());
        let config = File::from_paths(Some(&config_path), options_with_git_dir(&dir)).unwrap();
        assert_eq!(
            config.string("core", None, "a"),
            Some(cow_str("relative-dot-git")),
            "backslashes in GIT_DIR are converted to forward slashes"
        );
    }

    {
        let dir = dirs::home_dir().unwrap().join(".git");
        let config = File::from_paths(Some(&config_path), options_with_git_dir(&dir)).unwrap();
        assert_eq!(
            config.strings("core", None, "b"),
            Some(vec![cow_str("1"), cow_str("home-dot-git")]),
            "tilde ~ path is resolved to home directory"
        );
    }

    {
        let dir = dirs::home_dir().unwrap().join("c").join("d").join(".git");
        let config = File::from_paths(Some(&config_path), options_with_git_dir(&dir)).unwrap();
        assert_eq!(
            config.string("core", None, "b"),
            Some(cow_str("home-trailing-slash")),
            "path with trailing slash is matched"
        );
    }

    {
        let dir = dir.path().join("x").join("a").join(".git");
        let config = File::from_paths(Some(&config_path), options_with_git_dir(&dir)).unwrap();
        assert_eq!(
            config.string("core", None, "b"),
            Some(cow_str("relative-dot-git-2")), // TODO: figure out what's the difference to the non -2 version
            "** is prepended so paths ending with the pattern are matched"
        );
    }

    {
        let dir = PathBuf::from("/e/x/y/.git");
        let config = File::from_paths(Some(config_path.as_path()), options_with_git_dir(&dir)).unwrap();
        assert_eq!(
            config.string("core", None, "b"),
            Some(cow_str("absolute-path")),
            "absolute path pattern is matched with sub path from GIT_DIR"
        );
    }

    {
        let symlink_outside_tempdir_m_n = CanonicalizedTempDir::new().join("m").join("symlink");
        create_symlink(
            &symlink_outside_tempdir_m_n,
            &PathBuf::from(&format!("{}.git", tmp_dir_m_n_with_slash)),
        );
        let dir = PathBuf::from(&symlink_outside_tempdir_m_n);
        let config = File::from_paths(Some(config_path), options_with_git_dir(&dir)).unwrap();
        assert_eq!(
            config.string("core", None, "t"),
            Some(cow_str("absolute-path-with-symlink")),
            "absolute path pattern is matched with path from GIT_DIR when it contains symlink"
        );
        fs::remove_file(symlink_outside_tempdir_m_n.as_path()).unwrap();
    }
}

fn options_with_git_dir(git_dir: &Path) -> from_paths::Options<'_> {
    from_paths::Options {
        git_dir: Some(git_dir),
        ..Default::default()
    }
}
