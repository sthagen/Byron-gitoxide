mod diff {
    use std::convert::Infallible;

    use git_object::{bstr::ByteSlice, tree::EntryMode};
    use git_repository as git;
    use git_repository::object::{blob::diff::line::Change, tree::diff::change::Event};

    use crate::named_repo;

    #[test]
    fn changes_against_tree_modified() {
        let repo = named_repo("make_diff_repo.sh").unwrap();
        let from = tree_named(&repo, "@^{/c3}~1");
        let to = tree_named(&repo, ":/c3");
        from.changes()
            .for_each_to_obtain_tree(&to, |change| -> Result<_, Infallible> {
                assert_eq!(change.location, "", "without configuration the location field is empty");
                match change.event {
                    Event::Modification {
                        previous_entry_mode,
                        previous_id,
                        entry_mode,
                        id,
                    } => {
                        assert_eq!(previous_entry_mode, EntryMode::Blob);
                        assert_eq!(entry_mode, EntryMode::Blob);
                        assert_eq!(previous_id.object().unwrap().data.as_bstr(), "a\n");
                        assert_eq!(id.object().unwrap().data.as_bstr(), "a\na1\n");
                    }
                    Event::Deletion { .. } | Event::Addition { .. } => unreachable!("only modification is expected"),
                };

                let diff = change.event.diff().expect("changed file").expect("objects available");
                let count = diff.line_counts();
                assert_eq!(count.insertions, 1);
                assert_eq!(count.removals, 0);
                diff.lines(|hunk| {
                    match hunk {
                        Change::Deletion { .. } => unreachable!("there was no deletion"),
                        Change::Addition { lines } => assert_eq!(lines, vec!["a1".as_bytes().as_bstr()]),
                        Change::Modification { .. } => unreachable!("there was no modification"),
                    };
                    Ok::<_, Infallible>(())
                })
                .expect("infallible");
                Ok(Default::default())
            })
            .unwrap();
    }

    #[test]
    fn changes_against_tree_with_filename_tracking() {
        let repo = named_repo("make_diff_repo.sh").unwrap();
        let from = repo.empty_tree();
        let to = tree_named(&repo, ":/c1");

        let mut expected = vec!["a", "b", "c", "d"];
        from.changes()
            .track_filename()
            .for_each_to_obtain_tree(&to, |change| -> Result<_, Infallible> {
                expected.retain(|name| name != change.location);
                Ok(Default::default())
            })
            .unwrap();
        assert_eq!(expected, Vec::<&str>::new(), "all paths should have been seen");

        let mut expected = vec!["a", "b", "dir/c", "d"];
        from.changes()
            .track_path()
            .for_each_to_obtain_tree(&to, |change| -> Result<_, Infallible> {
                expected.retain(|name| name != change.location);
                Ok(Default::default())
            })
            .unwrap();
        assert_eq!(expected, Vec::<&str>::new(), "all paths should have been seen");

        let err = from
            .changes()
            .track_path()
            .for_each_to_obtain_tree(&to, |_change| {
                Err(std::io::Error::new(std::io::ErrorKind::Other, "custom error"))
            })
            .unwrap_err();
        assert_eq!(
            err.to_string(),
            "The user-provided callback failed",
            "custom errors made visible and not squelched"
        );
    }

    fn tree_named<'repo>(repo: &'repo git::Repository, rev_spec: &str) -> git::Tree<'repo> {
        repo.rev_parse_single(rev_spec)
            .unwrap()
            .object()
            .unwrap()
            .peel_to_kind(git::object::Kind::Tree)
            .unwrap()
            .into_tree()
    }
}
