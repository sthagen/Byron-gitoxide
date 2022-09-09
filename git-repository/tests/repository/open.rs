mod submodules {
    use std::path::Path;

    use git_repository as git;

    #[test]
    fn by_their_worktree_checkout_and_git_modules_dir() {
        let dir = git_testtools::scripted_fixture_repo_read_only("make_submodules.sh").unwrap();
        let parent_repo = Path::new("with-submodules");
        let modules = parent_repo.join(".git").join("modules");
        for module in ["m1", "dir/m1"] {
            let submodule_m1_workdir = parent_repo.join(module);
            let submodule_m1_gitdir = modules.join(module);

            for discover_dir in [
                submodule_m1_workdir.clone(),
                submodule_m1_workdir.join("subdir"),
                submodule_m1_gitdir.clone(),
            ] {
                let repo = discover_repo(discover_dir).unwrap();
                // assert_eq!(repo.kind(), git::Kind::Submodule);
                assert_eq!(repo.work_dir().expect("non-bare"), dir.join(&submodule_m1_workdir));
                assert_eq!(repo.git_dir(), dir.join(&submodule_m1_gitdir));

                let repo = git::open_opts(repo.work_dir().expect("non-bare"), git::open::Options::isolated()).unwrap();
                assert_eq!(repo.kind(), git::Kind::Submodule);
                assert_eq!(repo.work_dir().expect("non-bare"), dir.join(&submodule_m1_workdir));
                assert_eq!(repo.git_dir(), dir.join(&submodule_m1_gitdir));
            }
        }
    }

    fn discover_repo(name: impl AsRef<Path>) -> crate::Result<git::Repository> {
        let dir = git_testtools::scripted_fixture_repo_read_only("make_submodules.sh")?;
        let repo_dir = dir.join(name);
        Ok(git::ThreadSafeRepository::discover_opts(
            repo_dir,
            Default::default(),
            git_sec::trust::Mapping {
                full: crate::restricted(),
                reduced: crate::restricted(),
            },
        )?
        .to_thread_local())
    }
}
