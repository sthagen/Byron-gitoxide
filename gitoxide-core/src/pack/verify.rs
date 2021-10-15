use std::{
    io,
    path::Path,
    str::FromStr,
    sync::{atomic::AtomicBool, Arc},
};

use anyhow::{anyhow, Context as AnyhowContext, Result};
use bytesize::ByteSize;
use git_repository::{
    easy::object,
    hash::ObjectId,
    odb,
    odb::{pack, pack::index},
    progress, Progress,
};
pub use index::verify::Mode;

use crate::OutputFormat;

#[derive(Debug, Eq, PartialEq, Hash, Clone, Copy)]
pub enum Algorithm {
    LessTime,
    LessMemory,
}

impl Algorithm {
    pub fn variants() -> &'static [&'static str] {
        &["less-time", "less-memory"]
    }
}

impl FromStr for Algorithm {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s_lc = s.to_ascii_lowercase();
        Ok(match s_lc.as_str() {
            "less-memory" => Algorithm::LessMemory,
            "less-time" => Algorithm::LessTime,
            _ => return Err(format!("Invalid verification algorithm: '{}'", s)),
        })
    }
}

impl From<Algorithm> for index::traverse::Algorithm {
    fn from(v: Algorithm) -> Self {
        match v {
            Algorithm::LessMemory => index::traverse::Algorithm::Lookup,
            Algorithm::LessTime => index::traverse::Algorithm::DeltaTreeLookup,
        }
    }
}

/// A general purpose context for many operations provided here
pub struct Context<W1: io::Write, W2: io::Write> {
    /// If set, provide statistics to `out` in the given format
    pub output_statistics: Option<OutputFormat>,
    /// A stream to which to output operation results
    pub out: W1,
    /// A stream to which to errors
    pub err: W2,
    /// If set, don't use more than this amount of threads.
    /// Otherwise, usually use as many threads as there are logical cores.
    /// A value of 0 is interpreted as no-limit
    pub thread_limit: Option<usize>,
    pub mode: index::verify::Mode,
    pub algorithm: Algorithm,
    pub should_interrupt: Arc<AtomicBool>,
}

impl Default for Context<Vec<u8>, Vec<u8>> {
    fn default() -> Self {
        Context {
            output_statistics: None,
            thread_limit: None,
            mode: index::verify::Mode::Sha1Crc32,
            algorithm: Algorithm::LessMemory,
            out: Vec::new(),
            err: Vec::new(),
            should_interrupt: Default::default(),
        }
    }
}

enum EitherCache<const SIZE: usize> {
    Left(pack::cache::Never),
    Right(pack::cache::lru::StaticLinkedList<SIZE>),
}

impl<const SIZE: usize> pack::cache::DecodeEntry for EitherCache<SIZE> {
    fn put(&mut self, pack_id: u32, offset: u64, data: &[u8], kind: object::Kind, compressed_size: usize) {
        match self {
            EitherCache::Left(v) => v.put(pack_id, offset, data, kind, compressed_size),
            EitherCache::Right(v) => v.put(pack_id, offset, data, kind, compressed_size),
        }
    }

    fn get(&mut self, pack_id: u32, offset: u64, out: &mut Vec<u8>) -> Option<(object::Kind, usize)> {
        match self {
            EitherCache::Left(v) => v.get(pack_id, offset, out),
            EitherCache::Right(v) => v.get(pack_id, offset, out),
        }
    }
}

pub fn pack_or_pack_index<W1, W2>(
    path: impl AsRef<Path>,
    progress: Option<impl Progress>,
    Context {
        mut out,
        mut err,
        mode,
        output_statistics,
        thread_limit,
        algorithm,
        should_interrupt,
    }: Context<W1, W2>,
) -> Result<(ObjectId, Option<index::traverse::Outcome>)>
where
    W1: io::Write,
    W2: io::Write,
{
    let path = path.as_ref();
    let ext = path.extension().and_then(|ext| ext.to_str()).ok_or_else(|| {
        anyhow!(
            "Cannot determine data type on path without extension '{}', expecting default extensions 'idx' and 'pack'",
            path.display()
        )
    })?;
    let res = match ext {
        "pack" => {
            let pack = odb::pack::data::File::at(path).with_context(|| "Could not open pack file")?;
            pack.verify_checksum(
                progress::DoOrDiscard::from(progress).add_child("Sha1 of pack"),
                &should_interrupt,
            )
            .map(|id| (id, None))?
        }
        "idx" => {
            let idx = odb::pack::index::File::at(path).with_context(|| "Could not open pack index file")?;
            let packfile_path = path.with_extension("pack");
            let pack = odb::pack::data::File::at(&packfile_path)
                .map_err(|e| {
                    writeln!(
                        err,
                        "Could not find matching pack file at '{}' - only index file will be verified, error was: {}",
                        packfile_path.display(),
                        e
                    )
                    .ok();
                    e
                })
                .ok();
            const CACHE_SIZE: usize = 64;
            let cache = || -> EitherCache<CACHE_SIZE> {
                if matches!(algorithm, Algorithm::LessMemory) {
                    if output_statistics.is_some() {
                        // turn off acceleration as we need to see entire chains all the time
                        EitherCache::Left(pack::cache::Never)
                    } else {
                        EitherCache::Right(pack::cache::lru::StaticLinkedList::<CACHE_SIZE>::default())
                    }
                } else {
                    EitherCache::Left(pack::cache::Never)
                }
            };

            idx.verify_integrity(
                pack.as_ref().map(|p| (p, mode, algorithm.into(), cache)),
                thread_limit,
                progress,
                should_interrupt,
            )
            .map(|(a, b, _)| (a, b))
            .with_context(|| "Verification failure")?
        }
        ext => return Err(anyhow!("Unknown extension {:?}, expecting 'idx' or 'pack'", ext)),
    };
    if let Some(stats) = res.1.as_ref() {
        #[cfg_attr(not(feature = "serde1"), allow(clippy::single_match))]
        match output_statistics {
            Some(OutputFormat::Human) => drop(print_statistics(&mut out, stats)),
            #[cfg(feature = "serde1")]
            Some(OutputFormat::Json) => serde_json::to_writer_pretty(out, stats)?,
            _ => {}
        };
    }
    Ok(res)
}

fn print_statistics(out: &mut impl io::Write, stats: &index::traverse::Outcome) -> io::Result<()> {
    writeln!(out, "objects per delta chain length")?;
    let mut chain_length_to_object: Vec<_> = stats.objects_per_chain_length.iter().map(|(a, b)| (*a, *b)).collect();
    chain_length_to_object.sort_by_key(|e| e.0);
    let mut total_object_count = 0;
    for (chain_length, object_count) in chain_length_to_object.into_iter() {
        total_object_count += object_count;
        writeln!(out, "\t{:>2}: {}", chain_length, object_count)?;
    }
    writeln!(out, "\t->: {}", total_object_count)?;

    let pack::data::decode_entry::Outcome {
        kind: _,
        num_deltas,
        decompressed_size,
        compressed_size,
        object_size,
    } = stats.average;

    let width = 30;
    writeln!(out, "\naverages")?;
    #[rustfmt::skip]
    writeln!(
        out,
        "\t{:<width$} {};\n\t{:<width$} {};\n\t{:<width$} {};\n\t{:<width$} {};",
        "delta chain length:", num_deltas,
        "decompressed entry [B]:", decompressed_size,
        "compressed entry [B]:", compressed_size,
        "decompressed object size [B]:", object_size,
        width = width
    )?;

    writeln!(out, "\ncompression")?;
    #[rustfmt::skip]
    writeln!(
        out, "\t{:<width$}: {}\n\t{:<width$}: {}\n\t{:<width$}: {}\n\t{:<width$}: {}",
        "compressed entries size", ByteSize(stats.total_compressed_entries_size),
        "decompressed entries size", ByteSize(stats.total_decompressed_entries_size),
        "total object size", ByteSize(stats.total_object_size),
        "pack size", ByteSize(stats.pack_size),
        width = width
    )?;
    #[rustfmt::skip]
    writeln!(
        out,
        "\n\t{:<width$}: {}\n\t{:<width$}: {}\n\t{:<width$}: {}\n\t{:<width$}: {}",
        "num trees", stats.num_trees,
        "num blobs", stats.num_blobs,
        "num commits", stats.num_commits,
        "num tags", stats.num_tags,
        width = width
    )?;
    let compression_ratio = stats.total_decompressed_entries_size as f64 / stats.total_compressed_entries_size as f64;
    let delta_compression_ratio = stats.total_object_size as f64 / stats.total_compressed_entries_size as f64;
    #[rustfmt::skip]
    writeln!(
        out,
        "\n\t{:<width$}: {:.2}\n\t{:<width$}: {:.2}\n\t{:<width$}: {:.2}\n\t{:<width$}: {:.3}%",
        "compression ratio", compression_ratio,
        "delta compression ratio", delta_compression_ratio,
        "delta gain", delta_compression_ratio / compression_ratio,
        "pack overhead", (1.0 - (stats.total_compressed_entries_size as f64 / stats.pack_size as f64)) * 100.0,
        width = width
    )?;
    Ok(())
}
