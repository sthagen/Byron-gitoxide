use std::path::PathBuf;

use git_pack::multi_index::File;
use git_testtools::hex_to_id;

fn multi_index() -> (File, PathBuf) {
    let path = git_testtools::scripted_fixture_repo_read_only("make_pack_gen_repo_multi_index.sh")
        .expect("test fixture exists")
        .join(".git/objects/pack/multi-pack-index");
    let file = git_pack::multi_index::File::at(&path).unwrap();
    (file, path)
}

#[test]
fn access() {
    let (file, path) = multi_index();

    assert_eq!(file.version(), git_pack::multi_index::Version::V1);
    assert_eq!(file.path(), path);
    assert_eq!(file.num_indices(), 1);
    assert_eq!(file.object_hash(), git_hash::Kind::Sha1);
    assert_eq!(file.num_objects(), 868);
    assert_eq!(file.checksum(), hex_to_id("39a3804d0a84de609e4fcb49e66dc1297c75ca11"));
    // assert_eq!()
    assert_eq!(
        file.index_names(),
        vec![PathBuf::from("pack-542ad1d1c7c762ea4e36907570ff9e4b5b7dde1b.idx")]
    );

    for (idx, expected_pack_offset, expected_oid) in &[
        (0u32, 25267u64, hex_to_id("000f574443efab4ddbeee3621e49124eb3f8b6d0")),
        (140, 30421, hex_to_id("2935a65b1d69fb33c93dabc4cdf65a6f4d30ce4c")),
        (867, 24540, hex_to_id("ffea360a6a54c1185eeae4f3cfefc927cf7a35a9")),
    ] {
        let actual_oid = file.oid_at_index(*idx);
        assert_eq!(actual_oid, *expected_oid);
        assert_eq!(file.lookup(actual_oid), Some(*idx));
        let (pack_id, pack_offset) = file.pack_id_and_pack_offset_at_index(*idx);
        assert_eq!(pack_id, 0, "we only have one pack here");
        assert_eq!(pack_offset, *expected_pack_offset);
    }

    let mut count = 0;
    for (idx, entry) in file.iter().enumerate() {
        assert_eq!(entry.oid, file.oid_at_index(idx as u32));
        let (pack_index, pack_offset) = file.pack_id_and_pack_offset_at_index(idx as u32);
        assert_eq!(pack_index, entry.pack_index);
        assert_eq!(pack_offset, entry.pack_offset);
        count += 1;
    }
    assert_eq!(count, file.num_objects());
}

mod verify {
    use std::sync::atomic::AtomicBool;

    use common_macros::b_tree_map;
    use git_features::progress;

    use crate::pack::multi_index::multi_index;

    #[test]
    fn checksum() -> crate::Result {
        let (file, _) = multi_index();
        assert_eq!(
            file.verify_checksum(progress::Discard, &AtomicBool::new(false))?,
            file.checksum()
        );
        Ok(())
    }

    #[test]
    fn integrity() {
        let (file, _) = multi_index();
        let outcome = file
            .verify_integrity(progress::Discard, &AtomicBool::new(false), Default::default())
            .unwrap();
        assert_eq!(outcome.actual_index_checksum, file.checksum());
        assert_eq!(
            outcome.pack_traverse_statistics,
            vec![git_pack::index::traverse::Statistics {
                average: git_pack::data::decode_entry::Outcome {
                    kind: git_object::Kind::Tree,
                    num_deltas: 1,
                    decompressed_size: 47,
                    compressed_size: 46,
                    object_size: 152
                },
                objects_per_chain_length: b_tree_map! {
                    0 => 326,
                    1 => 106,
                    2 => 326,
                    3 => 108,
                    4 => 2,
                },
                total_compressed_entries_size: 40628,
                total_decompressed_entries_size: 40919,
                total_object_size: 131993,
                pack_size: 42856,
                num_commits: 16,
                num_trees: 40,
                num_tags: 1,
                num_blobs: 811
            }]
        );
    }
}

mod write {
    use std::{path::PathBuf, sync::atomic::AtomicBool};

    use git_features::progress;
    use git_testtools::{fixture_path, hex_to_id};

    #[test]
    fn from_paths() -> crate::Result {
        let dir = tempfile::TempDir::new()?;
        let input_indices = std::fs::read_dir(fixture_path("objects/pack"))?
            .filter_map(|r| {
                r.ok()
                    .map(|e| e.path())
                    .filter(|p| p.extension().and_then(|e| e.to_str()).unwrap_or("") == "idx")
            })
            .collect::<Vec<_>>();
        assert_eq!(input_indices.len(), 3);
        let output_path = dir.path().join("multi-pack-index");
        let mut out = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&output_path)?;
        let outcome = git_pack::multi_index::File::write_from_index_paths(
            input_indices.clone(),
            &mut out,
            progress::Discard,
            &AtomicBool::new(false),
            git_pack::multi_index::write::Options {
                object_hash: git_hash::Kind::Sha1,
            },
        )?;

        assert_eq!(
            outcome.multi_index_checksum,
            hex_to_id("d34d327039a3554f8a644b29e07b903fa71ef269")
        );

        let file = git_pack::multi_index::File::at(output_path)?;
        assert_eq!(file.num_indices(), 3);
        assert_eq!(
            file.index_names(),
            vec![
                PathBuf::from("pack-11fdfa9e156ab73caae3b6da867192221f2089c2.idx"),
                PathBuf::from("pack-a2bf8e71d8c18879e499335762dd95119d93d9f1.idx"),
                PathBuf::from("pack-c0438c19fb16422b6bbcce24387b3264416d485b.idx"),
            ]
        );
        assert_eq!(file.num_objects(), 139);
        assert_eq!(file.checksum(), outcome.multi_index_checksum);

        for index in &input_indices {
            std::fs::copy(index, dir.path().join(index.file_name().expect("present")))?;
            let pack = index.with_extension("pack");
            std::fs::copy(&pack, dir.path().join(pack.file_name().expect("present")))?;
        }

        assert_eq!(
            file.verify_integrity(progress::Discard, &AtomicBool::new(false), Default::default())?
                .actual_index_checksum,
            outcome.multi_index_checksum
        );

        let outcome = file.verify_integrity_fast(progress::Discard, &AtomicBool::new(false))?;

        assert_eq!(outcome.0, file.checksum());
        Ok(())
    }
}
