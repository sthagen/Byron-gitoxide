use git_repository as git;

use crate::remote;

#[test]
#[cfg(feature = "blocking-network-client")]
fn fetch_only_with_configuration() -> crate::Result {
    let tmp = git_testtools::tempfile::TempDir::new()?;
    let called_configure_remote = std::sync::Arc::new(std::sync::atomic::AtomicBool::default());
    let remote_name = "special";
    let mut prepare = git::prepare_clone_bare(remote::repo("base").path(), tmp.path())?
        .with_remote_name(remote_name)?
        .configure_remote({
            let called_configure_remote = called_configure_remote.clone();
            move |r| {
                called_configure_remote.store(true, std::sync::atomic::Ordering::Relaxed);
                Ok(
                    r.with_refspec("+refs/tags/*:refs/tags/*", git::remote::Direction::Fetch)
                        .expect("valid static spec"),
                )
            }
        });
    let (repo, out) = prepare.fetch_only(git::progress::Discard, &std::sync::atomic::AtomicBool::default())?;
    drop(prepare);

    assert!(
        called_configure_remote.load(std::sync::atomic::Ordering::Relaxed),
        "custom remote configuration is called"
    );
    assert_eq!(repo.remote_names().len(), 1, "only ever one remote");
    let remote = repo.find_remote(remote_name)?;
    assert_eq!(
        remote.refspecs(git::remote::Direction::Fetch).len(),
        2,
        "our added spec was stored as well"
    );

    assert_eq!(out.ref_map.mappings.len(), 13);
    match out.status {
        git_repository::remote::fetch::Status::Change { update_refs, .. } => {
            for edit in &update_refs.edits {
                use git_odb::Find;
                assert!(
                    repo.objects.contains(edit.change.new_value().expect("always set").id()),
                    "part of the fetched pack"
                );
            }
        }
        _ => unreachable!("clones are always causing changes and dry-runs aren't possible"),
    }
    Ok(())
}

#[test]
#[cfg(feature = "blocking-network-client")]
fn fetch_only_without_configuration() -> crate::Result {
    let tmp = git_testtools::tempfile::TempDir::new()?;
    let (repo, out) = git::prepare_clone_bare(remote::repo("base").path(), tmp.path())?
        .fetch_only(git::progress::Discard, &std::sync::atomic::AtomicBool::default())?;
    assert!(repo.find_remote("origin").is_ok(), "default remote name is 'origin'");
    match out.status {
        git_repository::remote::fetch::Status::Change { write_pack_bundle, .. } => {
            assert!(
                write_pack_bundle.keep_path.is_none(),
                "keep files aren't kept if refs are written"
            );
        }
        _ => unreachable!("a clone always carries a change"),
    }
    Ok(())
}

#[test]
fn clone_and_early_persist_without_receive() -> crate::Result {
    let tmp = git_testtools::tempfile::TempDir::new()?;
    let repo = git::prepare_clone_bare(remote::repo("base").path(), tmp.path())?.persist();
    assert!(repo.is_bare(), "repo is now ours and remains");
    Ok(())
}

#[test]
fn clone_bare_into_empty_directory_and_early_drop() -> crate::Result {
    let tmp = git_testtools::tempfile::TempDir::new()?;
    // this breaks isolation, but shouldn't be affecting the test. If so, use isolation options for opening the repo.
    let prep = git::prepare_clone_bare(remote::repo("base").path(), tmp.path())?;
    let head = tmp.path().join("HEAD");
    assert!(head.is_file(), "now a bare basic repo is present");
    drop(prep);

    assert!(!head.is_file(), "we cleanup if the clone isn't followed through");
    Ok(())
}

#[test]
fn clone_into_empty_directory_and_early_drop() -> crate::Result {
    let tmp = git_testtools::tempfile::TempDir::new()?;
    let prep = git::prepare_clone(remote::repo("base").path(), tmp.path())?;
    let head = tmp.path().join(".git").join("HEAD");
    assert!(head.is_file(), "now a basic repo is present");
    drop(prep);

    assert!(!head.is_file(), "we cleanup if the clone isn't followed through");
    Ok(())
}
