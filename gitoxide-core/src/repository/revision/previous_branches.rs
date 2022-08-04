use anyhow::Context;
use git_repository as git;

use crate::OutputFormat;

pub fn function(repo: git::Repository, mut out: impl std::io::Write, format: OutputFormat) -> anyhow::Result<()> {
    let branches = repo
        .head()?
        .prior_checked_out_branches()?
        .context("The reflog for HEAD is required")?;
    match format {
        OutputFormat::Human => {
            for (name, id) in branches {
                writeln!(out, "{} {}", id, name)?;
            }
        }
        #[cfg(feature = "serde1")]
        OutputFormat::Json => {
            serde_json::to_writer_pretty(&mut out, &branches)?;
        }
    }
    Ok(())
}
