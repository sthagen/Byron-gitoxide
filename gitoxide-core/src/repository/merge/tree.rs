use crate::OutputFormat;

pub struct Options {
    pub format: OutputFormat,
    pub file_favor: Option<gix::merge::tree::FileFavor>,
    pub in_memory: bool,
}

pub(super) mod function {

    use crate::OutputFormat;
    use anyhow::{anyhow, bail, Context};
    use gix::bstr::BString;
    use gix::bstr::ByteSlice;
    use gix::merge::tree::UnresolvedConflict;
    use gix::prelude::Write;

    use super::Options;

    #[allow(clippy::too_many_arguments)]
    pub fn tree(
        mut repo: gix::Repository,
        out: &mut dyn std::io::Write,
        err: &mut dyn std::io::Write,
        base: BString,
        ours: BString,
        theirs: BString,
        Options {
            format,
            file_favor,
            in_memory,
        }: Options,
    ) -> anyhow::Result<()> {
        if format != OutputFormat::Human {
            bail!("JSON output isn't implemented yet");
        }
        repo.object_cache_size_if_unset(repo.compute_object_cache_size_for_tree_diffs(&**repo.index_or_empty()?));
        if in_memory {
            repo.objects.enable_object_memory();
        }
        let (base_ref, base_id) = refname_and_tree(&repo, base)?;
        let (ours_ref, ours_id) = refname_and_tree(&repo, ours)?;
        let (theirs_ref, theirs_id) = refname_and_tree(&repo, theirs)?;

        let options = repo.tree_merge_options()?.with_file_favor(file_favor);
        let base_id_str = base_id.to_string();
        let ours_id_str = ours_id.to_string();
        let theirs_id_str = theirs_id.to_string();
        let labels = gix::merge::blob::builtin_driver::text::Labels {
            ancestor: base_ref
                .as_ref()
                .map_or(base_id_str.as_str().into(), |n| n.as_bstr())
                .into(),
            current: ours_ref
                .as_ref()
                .map_or(ours_id_str.as_str().into(), |n| n.as_bstr())
                .into(),
            other: theirs_ref
                .as_ref()
                .map_or(theirs_id_str.as_str().into(), |n| n.as_bstr())
                .into(),
        };
        let res = repo.merge_trees(base_id, ours_id, theirs_id, labels, options)?;
        let has_conflicts = res.conflicts.is_empty();
        let has_unresolved_conflicts = res.has_unresolved_conflicts(UnresolvedConflict::Renames);
        {
            let _span = gix::trace::detail!("Writing merged tree");
            let mut written = 0;
            let tree_id = res
                .tree
                .detach()
                .write(|tree| {
                    written += 1;
                    repo.write(tree)
                })
                .map_err(|err| anyhow!("{err}"))?;
            writeln!(out, "{tree_id} (wrote {written} trees)")?;
        }

        if !has_conflicts {
            writeln!(err, "{} possibly resolved conflicts", res.conflicts.len())?;
        }
        if has_unresolved_conflicts {
            bail!("Tree conflicted")
        }
        Ok(())
    }

    fn refname_and_tree(
        repo: &gix::Repository,
        revspec: BString,
    ) -> anyhow::Result<(Option<BString>, gix::hash::ObjectId)> {
        let spec = repo.rev_parse(revspec.as_bstr())?;
        let tree_id = spec
            .single()
            .context("Expected revspec to expand to a single rev only")?
            .object()?
            .peel_to_tree()?
            .id;
        let refname = spec.first_reference().map(|r| r.name.shorten().as_bstr().to_owned());
        Ok((refname, tree_id))
    }
}
