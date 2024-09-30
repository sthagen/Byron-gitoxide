use gix_merge::blob::builtin_driver::binary::{Pick, ResolveWith};
use gix_merge::blob::{builtin_driver, Resolution};

#[test]
fn binary() {
    assert_eq!(
        builtin_driver::binary(None),
        (Pick::Ours, Resolution::Conflict),
        "by default it picks ours and marks it as conflict"
    );
    assert_eq!(
        builtin_driver::binary(Some(ResolveWith::Ancestor)),
        (Pick::Ancestor, Resolution::Complete),
        "Otherwise we can pick anything and it will mark it as complete"
    );
    assert_eq!(
        builtin_driver::binary(Some(ResolveWith::Ours)),
        (Pick::Ours, Resolution::Complete)
    );
    assert_eq!(
        builtin_driver::binary(Some(ResolveWith::Theirs)),
        (Pick::Theirs, Resolution::Complete)
    );
}

mod text {
    use bstr::ByteSlice;
    use gix_merge::blob::Resolution;
    use pretty_assertions::assert_str_eq;

    const DIVERGING: &[&str] = &[
        // Somehow, on in zdiff mode, it's different, and I wasn't able to figure out the rule properly.
        // Now we prefer ancestor/before newlines and somewhat ignore our hunks. It's probably a minor issue in practice.
        // gix: "1\r\n2\n<<<<<<< complex/marker-newline-handling-lf2/ours.blob\n4\r\n||||||| complex/marker-newline-handling-lf2/base.blob\r\n2\r\n3\n=======\n5\n>>>>>>> complex/marker-newline-handling-lf2/theirs.blob\n"
        // git: "1\r\n2\n<<<<<<< complex/marker-newline-handling-lf2/ours.blob\n4  \n||||||| complex/marker-newline-handling-lf2/base.blob  \n2\r\n3\n=======\n5\n>>>>>>> complex/marker-newline-handling-lf2/theirs.blob\n"
        "complex/marker-newline-handling-lf2/zdiff3.merged",
        "complex/marker-newline-handling-lf2/zdiff3-histogram.merged",
        // This is related to Git seemingly extending a hunk to increase overlap (see diff3)
        "zdiff3-interesting/merge.merged",
        "zdiff3-interesting/merge-ours.merged",
        "zdiff3-interesting/diff3.merged",
        "zdiff3-interesting/diff3-histogram.merged",
        "zdiff3-interesting/zdiff3.merged",
        "zdiff3-interesting/zdiff3-histogram.merged",
        "zdiff3-interesting/merge-union.merged",
        // Git can extend hunks, similar to above, but the effect is not as noticeable.
        // Implementing this would be interesting, to figure out when the hunk processing should apply.
        "zdiff3-evil/merge.merged",
        "zdiff3-evil/merge-union.merged",
        // Git seems to merge to hunks if they are close together to get a less noisy diff.
        "zdiff3-middlecommon/merge.merged",
        "zdiff3-middlecommon/merge-union.merged",
        // Git has special character handling, which does magic to prevent conflicts
        "complex/auto-simplification/merge.merged",
        "complex/auto-simplification/merge-union.merged",
        // Git has special newline handling when diffing,
        // which auto-inserts a newline when it was removed, kind of.
        "complex/missing-LF-at-EOF/merge.merged",
        "complex/missing-LF-at-EOF/diff3.merged",
        "complex/missing-LF-at-EOF/diff3-histogram.merged",
        "complex/missing-LF-at-EOF/zdiff3.merged",
        "complex/missing-LF-at-EOF/zdiff3-histogram.merged",
        "complex/missing-LF-at-EOF/merge-ours.merged",
        "complex/missing-LF-at-EOF/merge-theirs.merged",
        "complex/missing-LF-at-EOF/merge-union.merged",
        // Git has different diff-slider-heuristics so diffs can be different.
        // See https://github.com/mhagger/diff-slider-tools.
        "complex/spurious-c-conflicts/merge.merged",
        "complex/spurious-c-conflicts/merge-union.merged",
        "complex/spurious-c-conflicts/diff3-histogram.merged",
        "complex/spurious-c-conflicts/zdiff3-histogram.merged",
    ];

    // TODO: fix all of these eventually
    fn is_case_diverging(case: &baseline::Expectation) -> bool {
        DIVERGING.iter().any(|name| case.name == *name)
    }

    #[test]
    fn run_baseline() -> crate::Result {
        let root = gix_testtools::scripted_fixture_read_only("text-baseline.sh")?;
        let cases = std::fs::read_to_string(root.join("baseline.cases"))?;
        let mut out = Vec::new();
        let mut num_diverging = 0;
        let mut num_cases = 0;
        for case in baseline::Expectations::new(&root, &cases) {
            num_cases += 1;
            let mut input = imara_diff::intern::InternedInput::default();
            let actual = gix_merge::blob::builtin_driver::text(
                &mut out,
                &mut input,
                case.labels(),
                &case.ours,
                &case.base,
                &case.theirs,
                case.options,
            );
            if is_case_diverging(&case) {
                num_diverging += 1;
            } else {
                let expected_resolution = if case.expected.contains_str("<<<<<<<") {
                    Resolution::Conflict
                } else {
                    Resolution::Complete
                };
                assert_eq!(out.as_bstr(), case.expected);
                assert_str_eq!(
                    out.as_bstr().to_str_lossy(),
                    case.expected.to_str_lossy(),
                    "{}: output mismatch\n{}",
                    case.name,
                    out.as_bstr()
                );
                assert_eq!(actual, expected_resolution, "{}: resolution mismatch", case.name,);
            }
        }

        assert_eq!(
            num_diverging,
            DIVERGING.len(),
            "Number of expected diverging cases must match the actual one - probably the implementation improved"
        );
        assert_eq!(
            ((num_diverging as f32 / num_cases as f32) * 100.0) as usize,
            11,
            "Just to show the percentage of skipped tests - this should get better"
        );
        Ok(())
    }

    mod baseline {
        use bstr::BString;
        use gix_merge::blob::builtin_driver::text::{Conflict, ConflictStyle};
        use std::path::Path;

        #[derive(Debug)]
        pub struct Expectation {
            pub ours: BString,
            pub ours_marker: String,
            pub theirs: BString,
            pub theirs_marker: String,
            pub base: BString,
            pub base_marker: String,
            pub name: BString,
            pub expected: BString,
            pub options: gix_merge::blob::builtin_driver::text::Options,
        }

        impl Expectation {
            pub fn labels(&self) -> gix_merge::blob::builtin_driver::text::Labels<'_> {
                gix_merge::blob::builtin_driver::text::Labels {
                    ancestor: Some(self.base_marker.as_str().as_ref()),
                    current: Some(self.ours_marker.as_str().as_ref()),
                    other: Some(self.theirs_marker.as_str().as_ref()),
                }
            }
        }

        pub struct Expectations<'a> {
            root: &'a Path,
            lines: std::str::Lines<'a>,
        }

        impl<'a> Expectations<'a> {
            pub fn new(root: &'a Path, cases: &'a str) -> Self {
                Expectations {
                    root,
                    lines: cases.lines(),
                }
            }
        }

        impl Iterator for Expectations<'_> {
            type Item = Expectation;

            fn next(&mut self) -> Option<Self::Item> {
                let line = self.lines.next()?;
                let mut words = line.split(' ');
                let (Some(ours), Some(base), Some(theirs), Some(output)) =
                    (words.next(), words.next(), words.next(), words.next())
                else {
                    panic!("need at least the input and output")
                };

                let read = |rela_path: &str| read_blob(self.root, rela_path);

                let mut options = gix_merge::blob::builtin_driver::text::Options::default();
                for arg in words {
                    options.conflict = match arg {
                        "--diff3" => Conflict::Keep {
                            style: ConflictStyle::Diff3,
                            marker_size: 7,
                        },
                        "--zdiff3" => Conflict::Keep {
                            style: ConflictStyle::ZealousDiff3,
                            marker_size: 7,
                        },
                        "--ours" => Conflict::ResolveWithOurs,
                        "--theirs" => Conflict::ResolveWithTheirs,
                        "--union" => Conflict::ResolveWithUnion,
                        _ => panic!("Unknown argument to parse into options: '{arg}'"),
                    }
                }
                if output.contains("histogram") {
                    options.diff_algorithm = imara_diff::Algorithm::Histogram;
                }

                Some(Expectation {
                    ours: read(ours),
                    ours_marker: ours.into(),
                    theirs: read(theirs),
                    theirs_marker: theirs.into(),
                    base: read(base),
                    base_marker: base.into(),
                    expected: read(output),
                    name: output.into(),
                    options,
                })
            }
        }

        fn read_blob(root: &Path, rela_path: &str) -> BString {
            std::fs::read(root.join(rela_path))
                .unwrap_or_else(|_| panic!("Failed to read '{rela_path}' in '{}'", root.display()))
                .into()
        }
    }
}
