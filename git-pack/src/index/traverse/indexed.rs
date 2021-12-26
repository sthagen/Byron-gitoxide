use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use git_features::{parallel, progress::Progress};

use super::{Error, SafetyCheck};
use crate::{
    cache::delta::traverse::Context,
    index::{self, util::index_entries_sorted_by_offset_ascending},
};

/// Traversal with index
impl index::File {
    /// Iterate through all _decoded objects_ in the given `pack` and handle them with a `Processor`, using an index to reduce waste
    /// at the cost of memory.
    ///
    /// For more details, see the documentation on the [`traverse()`][index::File::traverse()] method.
    pub fn traverse_with_index<P, Processor, E>(
        &self,
        check: SafetyCheck,
        thread_limit: Option<usize>,
        new_processor: impl Fn() -> Processor + Send + Clone,
        mut progress: P,
        pack: &crate::data::File,
        should_interrupt: Arc<AtomicBool>,
    ) -> Result<(git_hash::ObjectId, index::traverse::Outcome, P), Error<E>>
    where
        P: Progress,
        Processor: FnMut(
            git_object::Kind,
            &[u8],
            &index::Entry,
            &mut <<P as Progress>::SubProgress as Progress>::SubProgress,
        ) -> Result<(), E>,
        E: std::error::Error + Send + Sync + 'static,
    {
        let (verify_result, traversal_result) = parallel::join(
            {
                let pack_progress = progress.add_child("SHA1 of pack");
                let index_progress = progress.add_child("SHA1 of index");
                let should_interrupt = Arc::clone(&should_interrupt);
                move || {
                    let res = self.possibly_verify(
                        pack,
                        check,
                        pack_progress,
                        index_progress,
                        Arc::clone(&should_interrupt),
                    );
                    if res.is_err() {
                        should_interrupt.store(true, Ordering::SeqCst);
                    }
                    res
                }
            },
            || -> Result<_, Error<_>> {
                let sorted_entries =
                    index_entries_sorted_by_offset_ascending(self, progress.add_child("collecting sorted index"));
                let tree = crate::cache::delta::Tree::from_offsets_in_pack(
                    sorted_entries.into_iter().map(Entry::from),
                    |e| e.index_entry.pack_offset,
                    pack.path(),
                    progress.add_child("indexing"),
                    &should_interrupt,
                    |id| self.lookup(id).map(|idx| self.pack_offset_at_index(idx)),
                    self.object_hash,
                )?;
                let there_are_enough_objects = || self.num_objects > 10_000;
                let mut outcome = digest_statistics(tree.traverse(
                    there_are_enough_objects,
                    |slice, out| pack.entry_slice(slice).map(|entry| out.copy_from_slice(entry)),
                    progress.add_child("Resolving"),
                    progress.add_child("Decoding"),
                    thread_limit,
                    &should_interrupt,
                    pack.pack_end() as u64,
                    new_processor,
                    |data,
                     progress,
                     Context {
                         entry: pack_entry,
                         entry_end,
                         decompressed: bytes,
                         state: ref mut processor,
                         level,
                     }| {
                        let object_kind = pack_entry.header.as_kind().expect("non-delta object");
                        data.level = level;
                        data.decompressed_size = pack_entry.decompressed_size;
                        data.object_kind = object_kind;
                        data.compressed_size = entry_end - pack_entry.data_offset;
                        data.object_size = bytes.len() as u64;
                        let result = crate::index::traverse::process_entry(
                            check,
                            object_kind,
                            bytes,
                            progress,
                            &data.index_entry,
                            || {
                                // TODO: Fix this - we overwrite the header of 'data' which also changes the computed entry size,
                                // causing index and pack to seemingly mismatch. This is surprising, and should be done differently.
                                // debug_assert_eq!(&data.index_entry.pack_offset, &pack_entry.pack_offset());
                                git_features::hash::crc32(
                                    pack.entry_slice(data.index_entry.pack_offset..entry_end)
                                        .expect("slice pointing into the pack (by now data is verified)"),
                                )
                            },
                            processor,
                        );
                        match result {
                            Err(err @ Error::PackDecode { .. }) if !check.fatal_decode_error() => {
                                progress.info(format!("Ignoring decode error: {}", err));
                                Ok(())
                            }
                            res => res,
                        }
                    },
                    self.object_hash,
                )?);
                outcome.pack_size = pack.data_len() as u64;
                Ok(outcome)
            },
        );
        let id = verify_result?;
        let outcome = traversal_result?;
        Ok((id, outcome, progress))
    }
}

struct Entry {
    index_entry: crate::index::Entry,
    object_kind: git_object::Kind,
    object_size: u64,
    decompressed_size: u64,
    compressed_size: u64,
    level: u16,
}

impl From<crate::index::Entry> for Entry {
    fn from(index_entry: crate::index::Entry) -> Self {
        Entry {
            index_entry,
            level: 0,
            object_kind: git_object::Kind::Tree,
            object_size: 0,
            decompressed_size: 0,
            compressed_size: 0,
        }
    }
}

fn digest_statistics(items: VecDeque<crate::cache::delta::Item<Entry>>) -> index::traverse::Outcome {
    let mut res = index::traverse::Outcome::default();
    let average = &mut res.average;
    for item in &items {
        res.total_compressed_entries_size += item.data.compressed_size;
        res.total_decompressed_entries_size += item.data.decompressed_size;
        res.total_object_size += item.data.object_size;
        *res.objects_per_chain_length.entry(item.data.level as u32).or_insert(0) += 1;

        average.decompressed_size += item.data.decompressed_size;
        average.compressed_size += item.data.compressed_size as usize;
        average.object_size += item.data.object_size;
        average.num_deltas += item.data.level as u32;
        use git_object::Kind::*;
        match item.data.object_kind {
            Blob => res.num_blobs += 1,
            Tree => res.num_trees += 1,
            Tag => res.num_tags += 1,
            Commit => res.num_commits += 1,
        };
    }

    average.decompressed_size /= items.len() as u64;
    average.compressed_size /= items.len();
    average.object_size /= items.len() as u64;
    average.num_deltas /= items.len() as u32;

    res
}
