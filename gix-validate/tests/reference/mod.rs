macro_rules! mktests {
    ($name:ident, $input:expr, $expected:expr) => {
        #[test]
        fn $name() {
            let actual = gix_validate::reference::name_partial_or_sanitize($input.as_bstr());
            assert_eq!(actual, $expected);
            assert!(gix_validate::reference::name_partial(actual.as_ref()).is_ok());
        }
    };
}

mod name_partial {
    mod valid {
        use bstr::ByteSlice;
        macro_rules! mktest {
            ($name:ident, $input:expr) => {
                #[test]
                fn $name() {
                    assert!(gix_validate::reference::name_partial($input.as_bstr()).is_ok())
                }
            };
        }

        mktest!(refs_path, b"refs/heads/main");
        mktests!(refs_path_san, b"refs/heads/main", "refs/heads/main");
        mktest!(main_worktree_pseudo_ref, b"main-worktree/HEAD");
        mktests!(
            main_worktree_pseudo_ref_san,
            b"main-worktree/HEAD",
            "main-worktree/HEAD"
        );
        mktest!(main_worktree_ref, b"main-worktree/refs/bisect/good");
        mktests!(
            main_worktree_ref_san,
            b"main-worktree/refs/bisect/good",
            "main-worktree/refs/bisect/good"
        );
        mktest!(other_worktree_pseudo_ref, b"worktrees/id/HEAD");
        mktests!(other_worktree_pseudo_ref_san, b"worktrees/id/HEAD", "worktrees/id/HEAD");
        mktest!(other_worktree_ref, b"worktrees/id/refs/bisect/good");
        mktests!(
            other_worktree_ref_san,
            b"worktrees/id/refs/bisect/good",
            "worktrees/id/refs/bisect/good"
        );
        mktest!(worktree_private_ref, b"refs/worktree/private");
        mktests!(
            worktree_private_ref_san,
            b"refs/worktree/private",
            "refs/worktree/private"
        );
        mktest!(refs_path_with_file_extension, b"refs/heads/main.ext");
        mktests!(
            refs_path_with_file_extension_san,
            b"refs/heads/main.ext",
            "refs/heads/main.ext"
        );
        mktest!(refs_path_underscores_and_dashes, b"refs/heads/main-2nd_ext");
        mktests!(
            refs_path_underscores_and_dashes_san,
            b"refs/heads/main-2nd_ext",
            "refs/heads/main-2nd_ext"
        );
        mktest!(relative_path, b"etc/foo");
        mktests!(relative_path_san, b"etc/foo", "etc/foo");
        mktest!(all_uppercase, b"MAIN");
        mktests!(all_uppercase_san, b"MAIN", "MAIN");
        mktest!(all_uppercase_with_underscore, b"NEW_HEAD");
        mktests!(all_uppercase_with_underscore_san, b"NEW_HEAD", "NEW_HEAD");
        mktest!(partial_name_lowercase, b"main");
        mktests!(partial_name_lowercase_san, b"main", "main");
        mktest!(chinese_utf8, "heads/你好吗".as_bytes());
        mktests!(chinese_utf8_san, "heads/你好吗".as_bytes(), "heads/你好吗");
        mktest!(parentheses_special_case_upload_pack, b"(null)");
        mktests!(parentheses_special_case_upload_pack_san, b"(null)", "(null)");
    }

    mod invalid {
        use bstr::ByteSlice;
        use gix_validate::{reference::name::Error as RefError, tag::name::Error as TagError};

        macro_rules! mktest {
            ($name:ident, $input:literal, $expected:pat) => {
                #[test]
                fn $name() {
                    match gix_validate::reference::name_partial($input.as_bstr()) {
                        Err($expected) => {}
                        got => panic!("Wanted {}, got {:?}", stringify!($expected), got),
                    }
                }
            };
        }

        mktest!(
            refs_path_double_dot,
            b"refs/../somewhere",
            RefError::Tag(TagError::StartsWithDot)
        );
        mktests!(refs_path_double_dot_san, b"refs/../somewhere", "refs/-/somewhere");
        mktest!(
            refs_path_name_starts_with_dot,
            b".refs/somewhere",
            RefError::Tag(TagError::StartsWithDot)
        );
        mktest!(
            refs_path_name_starts_with_multi_dot,
            b"..refs/somewhere",
            RefError::Tag(TagError::RepeatedDot)
        );
        mktests!(
            refs_path_name_starts_with_multi_dot_san,
            b"..refs/somewhere",
            "-refs/somewhere"
        );
        mktests!(
            refs_path_name_starts_with_dot_san,
            b".refs/somewhere",
            "-refs/somewhere"
        );
        mktest!(
            refs_path_component_is_singular_dot,
            b"refs/./still-inside-but-not-cool",
            RefError::Tag(TagError::StartsWithDot)
        );
        mktests!(
            refs_path_component_is_singular_dot_san,
            b"refs/./still-inside-but-not-cool",
            "refs/-/still-inside-but-not-cool"
        );
        mktest!(
            any_path_starts_with_slash,
            b"/etc/foo",
            RefError::Tag(TagError::StartsWithSlash)
        );
        mktests!(any_path_starts_with_slash_san, b"/etc/foo", "etc/foo");
        mktest!(empty_path, b"", RefError::Tag(TagError::Empty));
        mktests!(empty_path_san, b"", "-");
        mktest!(
            refs_starts_with_slash,
            b"/refs/heads/main",
            RefError::Tag(TagError::StartsWithSlash)
        );
        mktests!(refs_starts_with_slash_san, b"/refs/heads/main", "refs/heads/main");
        mktest!(
            ends_with_slash,
            b"refs/heads/main/",
            RefError::Tag(TagError::EndsWithSlash)
        );
        mktests!(ends_with_slash_san, b"refs/heads/main/", "refs/heads/main");
        mktest!(
            path_with_duplicate_slashes,
            b"refs//heads/main",
            RefError::Tag(TagError::RepeatedSlash)
        );
        mktests!(path_with_duplicate_slashes_san, b"refs//heads/main", "refs/heads/main");
        mktest!(
            path_with_spaces,
            b"refs/heads/name with spaces",
            RefError::Tag(TagError::InvalidByte { .. })
        );
        mktests!(
            path_with_spaces_san,
            b"refs//heads////name with spaces",
            "refs/heads/name-with-spaces"
        );
        mktest!(
            path_with_backslashes,
            b"refs\\heads/name with spaces",
            RefError::Tag(TagError::InvalidByte { .. })
        );
        mktests!(
            path_with_backslashes_san,
            b"refs\\heads/name with spaces",
            "refs-heads/name-with-spaces"
        );
    }
}

mod name {
    mod valid {
        use bstr::ByteSlice;
        macro_rules! mktest {
            ($name:ident, $input:expr) => {
                #[test]
                fn $name() {
                    assert!(gix_validate::reference::name($input.as_bstr()).is_ok())
                }
            };
        }

        mktest!(main_worktree_pseudo_ref, b"main-worktree/HEAD");
        mktests!(
            main_worktree_pseudo_ref_san,
            b"main-worktree/HEAD",
            "main-worktree/HEAD"
        );
        mktest!(main_worktree_ref, b"main-worktree/refs/bisect/good");
        mktests!(
            main_worktree_ref_san,
            b"main-worktree/refs/bisect/good",
            "main-worktree/refs/bisect/good"
        );
        mktest!(other_worktree_pseudo_ref, b"worktrees/id/HEAD");
        mktests!(other_worktree_pseudo_ref_san, b"worktrees/id/HEAD", "worktrees/id/HEAD");
        mktest!(other_worktree_ref, b"worktrees/id/refs/bisect/good");
        mktests!(
            other_worktree_ref_san,
            b"worktrees/id/refs/bisect/good",
            "worktrees/id/refs/bisect/good"
        );
        mktest!(worktree_private_ref, b"refs/worktree/private");
        mktests!(
            worktree_private_ref_san,
            b"refs/worktree/private",
            "refs/worktree/private"
        );
        mktest!(refs_path, b"refs/heads/main");
        mktests!(refs_path_san, b"refs/heads/main", "refs/heads/main");
        mktest!(refs_path_with_file_extension, b"refs/heads/main.ext");
        mktests!(
            refs_path_with_file_extension_san,
            b"refs/heads/main.ext",
            "refs/heads/main.ext"
        );
        mktest!(refs_path_underscores_and_dashes, b"refs/heads/main-2nd_ext");
        mktests!(
            refs_path_underscores_and_dashes_san,
            b"refs/heads/main-2nd_ext",
            "refs/heads/main-2nd_ext"
        );
        mktest!(relative_path, b"etc/foo");
        mktests!(relative_path_san, b"etc/foo", "etc/foo");
        mktest!(all_uppercase, b"MAIN");
        mktests!(all_uppercase_san, b"MAIN", "MAIN");
        mktest!(all_uppercase_with_underscore, b"NEW_HEAD");
        mktests!(all_uppercase_with_underscore_san, b"NEW_HEAD", "NEW_HEAD");
        mktest!(chinese_utf8, "refs/heads/你好吗".as_bytes());
        mktests!(chinese_utf8_san, "refs/heads/你好吗".as_bytes(), "refs/heads/你好吗");
        mktest!(dot_in_directory_component, b"this./totally./works");
        mktests!(
            dot_in_directory_component_san,
            b"this./totally./works",
            "this./totally./works"
        );
    }

    mod invalid {
        use bstr::ByteSlice;
        use gix_validate::{reference::name::Error as RefError, tag::name::Error as TagError};

        macro_rules! mktest {
            ($name:ident, $input:literal, $expected:pat) => {
                #[test]
                fn $name() {
                    match gix_validate::reference::name($input.as_bstr()) {
                        Err($expected) => {}
                        got => panic!("Wanted {}, got {:?}", stringify!($expected), got),
                    }
                }
            };
        }

        mktest!(
            refs_path_double_dot,
            b"refs/../somewhere",
            RefError::Tag(TagError::StartsWithDot)
        );
        mktests!(refs_path_double_dot_san, b"refs/../somewhere", "refs/-/somewhere");
        mktest!(refs_name_special_case_upload_pack, b"(null)", RefError::SomeLowercase);
        mktests!(refs_name_special_case_upload_pack_san, b"(null)", "(null)");
        mktest!(
            refs_path_name_starts_with_dot,
            b".refs/somewhere",
            RefError::Tag(TagError::StartsWithDot)
        );
        mktests!(
            refs_path_name_starts_with_dot_san,
            b".refs/somewhere",
            "-refs/somewhere"
        );

        mktest!(
            refs_path_name_starts_with_dot_in_name,
            b"refs/.somewhere",
            RefError::Tag(TagError::StartsWithDot)
        );
        mktests!(
            refs_path_name_starts_with_dot_in_name_san,
            b"refs/.somewhere",
            "refs/-somewhere"
        );
        mktest!(
            refs_path_name_ends_with_dot_in_name,
            b"refs/somewhere.",
            RefError::Tag(TagError::EndsWithDot)
        );
        mktests!(
            refs_path_name_ends_with_dot_in_name_san,
            b"refs/somewhere.",
            "refs/somewhere-"
        );
        mktest!(
            refs_path_component_is_singular_dot,
            b"refs/./still-inside-but-not-cool",
            RefError::Tag(TagError::StartsWithDot)
        );
        mktests!(
            refs_path_component_is_singular_dot_an,
            b"refs/./still-inside-but-not-cool",
            "refs/-/still-inside-but-not-cool"
        );
        mktest!(capitalized_name_without_path, b"Main", RefError::SomeLowercase);
        mktests!(capitalized_name_without_path_san, b"Main", "Main");
        mktest!(lowercase_name_without_path, b"main", RefError::SomeLowercase);
        mktests!(lowercase_name_without_path_san, b"main", "main");
        mktest!(
            any_path_starts_with_slash,
            b"/etc/foo",
            RefError::Tag(TagError::StartsWithSlash)
        );
        mktests!(any_path_starts_with_slash_san, b"/etc/foo", "etc/foo");
        mktest!(empty_path, b"", RefError::Tag(TagError::Empty));
        mktests!(empty_path_san, b"", "-");
        mktest!(
            refs_starts_with_slash,
            b"/refs/heads/main",
            RefError::Tag(TagError::StartsWithSlash)
        );
        mktests!(refs_starts_with_slash_san, b"/refs/heads/main", "refs/heads/main");
        mktest!(
            ends_with_slash,
            b"refs/heads/main/",
            RefError::Tag(TagError::EndsWithSlash)
        );
        mktests!(ends_with_slash_san, b"refs/heads/main/", "refs/heads/main");
        mktest!(
            ends_with_slash_multiple,
            b"refs/heads/main///",
            RefError::Tag(TagError::EndsWithSlash)
        );
        mktests!(ends_with_slash_multiple_san, b"refs/heads/main///", "refs/heads/main");
        mktest!(
            a_path_with_duplicate_slashes,
            b"refs//heads/main",
            RefError::Tag(TagError::RepeatedSlash)
        );
        mktests!(
            a_path_with_duplicate_slashes_san,
            b"refs//heads/main",
            "refs/heads/main"
        );
    }
}
