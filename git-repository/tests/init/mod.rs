mod bare {
    #[test]
    fn init_into_empty_directory_creates_a_dot_git_dir() -> crate::Result {
        let tmp = tempfile::tempdir()?;
        let repo = git_repository::init_bare(tmp.path())?;
        assert_eq!(repo.kind(), git_repository::Kind::Bare);
        assert!(
            repo.work_dir().is_none(),
            "a worktree isn't present in bare repositories"
        );
        assert_eq!(
            repo.git_dir(),
            tmp.path(),
            "the repository is placed into the directory itself"
        );
        assert_eq!(git_repository::open(repo.git_dir())?, repo);
        Ok(())
    }

    #[test]
    fn init_into_non_empty_directory_is_not_allowed() -> crate::Result {
        let tmp = tempfile::tempdir()?;
        std::fs::write(tmp.path().join("existing.txt"), b"I was here before you")?;

        assert!(git_repository::init_bare(tmp.path())
            .unwrap_err()
            .to_string()
            .starts_with("Refusing to initialize the non-empty directory as"));
        Ok(())
    }
}

mod non_bare {
    #[test]
    fn init_into_empty_directory_creates_a_dot_git_dir() -> crate::Result {
        let tmp = tempfile::tempdir()?;
        let repo = git_repository::init(tmp.path())?;
        assert_eq!(repo.kind(), git_repository::Kind::WorkTree { is_linked: false });
        assert_eq!(repo.work_dir(), Some(tmp.path()), "there is a work tree by default");
        assert_eq!(
            repo.git_dir(),
            tmp.path().join(".git"),
            "there is a work tree by default"
        );
        assert_eq!(git_repository::open(repo.git_dir())?, repo);
        assert_eq!(
            git_repository::open(repo.work_dir().as_ref().expect("non-bare repo"))?,
            repo
        );
        Ok(())
    }

    #[test]
    fn init_into_non_empty_directory_is_allowed() -> crate::Result {
        let tmp = tempfile::tempdir()?;
        std::fs::write(tmp.path().join("existing.txt"), b"I was here before you")?;

        git_repository::init(tmp.path())?;
        Ok(())
    }
}
