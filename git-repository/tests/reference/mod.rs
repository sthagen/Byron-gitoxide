mod log {
    use git_repository as git;

    #[test]
    fn message() {
        assert_eq!(
            git::reference::log::message("commit", "the subject\n\nthe body".into(), 0),
            "commit (initial): the subject"
        );
        assert_eq!(
            git::reference::log::message("other", "the subject".into(), 1),
            "other: the subject"
        );

        assert_eq!(
            git::reference::log::message("rebase", "the subject".into(), 2),
            "rebase (merge): the subject"
        );
    }
}
mod find {
    use std::convert::TryInto;

    use git_ref as refs;
    use git_ref::FullNameRef;
    use git_testtools::hex_to_id;

    fn repo() -> crate::Result<git_repository::Repository> {
        crate::repo("make_references_repo.sh").map(Into::into)
    }

    #[test]
    fn and_peel() -> crate::Result {
        let repo = repo()?;
        let mut packed_tag_ref = repo.try_find_reference("dt1")?.expect("tag to exist");
        let expected: &FullNameRef = "refs/tags/dt1".try_into()?;
        assert_eq!(packed_tag_ref.name(), expected);

        assert_eq!(
            packed_tag_ref.inner.target,
            refs::Target::Peeled(hex_to_id("4c3f4cce493d7beb45012e478021b5f65295e5a3")),
            "it points to a tag object"
        );

        let object = packed_tag_ref.peel_to_id_in_place()?;
        let the_commit = hex_to_id("134385f6d781b7e97062102c6a483440bfda2a03");
        assert_eq!(object, the_commit, "it is assumed to be fully peeled");
        assert_eq!(
            object,
            packed_tag_ref.peel_to_id_in_place()?,
            "peeling again yields the same object"
        );

        let mut symbolic_ref = repo.find_reference("multi-link-target1")?;

        let expected: &FullNameRef = "refs/heads/multi-link-target1".try_into()?;
        assert_eq!(symbolic_ref.name(), expected);
        assert_eq!(symbolic_ref.peel_to_id_in_place()?, the_commit);

        let expected: &FullNameRef = "refs/remotes/origin/multi-link-target3".try_into()?;
        assert_eq!(symbolic_ref.name(), expected, "it follows symbolic refs, too");
        assert_eq!(symbolic_ref.into_fully_peeled_id()?, the_commit, "idempotency");
        Ok(())
    }
}

mod remote {
    use crate::remote;
    use git_repository as git;

    #[test]
    fn push_defaults_to_fetch() -> crate::Result {
        let repo = remote::repo("many-fetchspecs");
        let head = repo.head()?;
        let branch = head.clone().try_into_referent().expect("history");
        assert_eq!(
            branch
                .remote_name(git::remote::Direction::Push)
                .expect("fallback to fetch"),
            branch.remote_name(git::remote::Direction::Fetch).expect("configured"),
            "push falls back to fetch"
        );
        assert_eq!(
            branch
                .remote(git::remote::Direction::Push)
                .expect("configured")?
                .name()
                .expect("set"),
            "origin"
        );
        assert_eq!(
            head.into_remote(git::remote::Direction::Push)
                .expect("same with branch")?
                .name()
                .expect("set"),
            "origin"
        );
        Ok(())
    }

    #[test]
    fn separate_push_and_fetch() -> crate::Result {
        for name in ["push-default", "branch-push-remote"] {
            let repo = remote::repo(name);
            let head = repo.head()?;
            let branch = head.clone().try_into_referent().expect("history");

            assert_eq!(branch.remote_name(git::remote::Direction::Push).expect("set"), "myself");
            assert_eq!(
                branch.remote_name(git::remote::Direction::Fetch).expect("set"),
                "new-origin"
            );

            assert_ne!(
                branch.remote(git::remote::Direction::Push).transpose()?,
                branch.remote(git::remote::Direction::Fetch).transpose()?
            );
            assert_ne!(
                head.clone().into_remote(git::remote::Direction::Push).transpose()?,
                head.into_remote(git::remote::Direction::Fetch).transpose()?
            );
        }
        Ok(())
    }

    #[test]
    fn not_configured() -> crate::Result {
        let repo = remote::repo("base");
        let head = repo.head()?;
        let branch = head.clone().try_into_referent().expect("history");

        assert_eq!(branch.remote_name(git::remote::Direction::Push), None);
        assert_eq!(branch.remote_name(git::remote::Direction::Fetch), None);
        assert_eq!(branch.remote(git::remote::Direction::Fetch).transpose()?, None);
        assert_eq!(head.into_remote(git::remote::Direction::Fetch).transpose()?, None);

        Ok(())
    }
}
