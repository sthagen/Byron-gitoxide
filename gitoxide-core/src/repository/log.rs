use anyhow::bail;
use gix::bstr::{BString, ByteSlice};

pub fn log(mut repo: gix::Repository, out: &mut dyn std::io::Write, path: Option<BString>) -> anyhow::Result<()> {
    repo.object_cache_size_if_unset(repo.compute_object_cache_size_for_tree_diffs(&**repo.index_or_empty()?));

    if let Some(path) = path {
        log_file(repo, out, path)
    } else {
        log_all(repo, out)
    }
}

fn log_all(repo: gix::Repository, out: &mut dyn std::io::Write) -> Result<(), anyhow::Error> {
    let head = repo.head()?.peel_to_commit_in_place()?;
    let topo = gix::traverse::commit::topo::Builder::from_iters(&repo.objects, [head.id], None::<Vec<gix::ObjectId>>)
        .build()?;

    for info in topo {
        let info = info?;

        write_info(&repo, &mut *out, &info)?;
    }

    Ok(())
}

fn log_file(_repo: gix::Repository, _out: &mut dyn std::io::Write, _path: BString) -> anyhow::Result<()> {
    bail!("File-based lookup isn't yet implemented in a way that is competitively fast");
}

fn write_info(
    repo: &gix::Repository,
    mut out: impl std::io::Write,
    info: &gix::traverse::commit::Info,
) -> Result<(), std::io::Error> {
    let commit = repo.find_commit(info.id).unwrap();

    let message = commit.message_raw_sloppy();
    let title = message.lines().next();

    writeln!(
        out,
        "{} {}",
        info.id.to_hex_with_len(8),
        title.map_or_else(|| "<no message>".into(), BString::from)
    )?;

    Ok(())
}
