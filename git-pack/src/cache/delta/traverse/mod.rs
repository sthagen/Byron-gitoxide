use std::{
    collections::VecDeque,
    sync::atomic::{AtomicBool, Ordering},
};

use git_features::{
    parallel,
    parallel::in_parallel_if,
    progress::{self, Progress},
    threading::{lock, Mutable, OwnShared},
};

use crate::{
    cache::delta::{Item, Tree},
    data::EntryRange,
};

mod resolve;

/// Returned by [`Tree::traverse()`]
#[derive(thiserror::Error, Debug)]
#[allow(missing_docs)]
pub enum Error {
    #[error("{message}")]
    ZlibInflate {
        source: git_features::zlib::inflate::Error,
        message: &'static str,
    },
    #[error("The resolver failed to obtain the pack entry bytes for the entry at {pack_offset}")]
    ResolveFailed { pack_offset: u64 },
    #[error("One of the object inspectors failed")]
    Inspect(#[from] Box<dyn std::error::Error + Send + Sync>),
    #[error("Interrupted")]
    Interrupted,
}

/// Additional context passed to the `inspect_object(…)` function of the [`Tree::traverse()`] method.
pub struct Context<'a, S> {
    /// The pack entry describing the object
    pub entry: &'a crate::data::Entry,
    /// The offset at which `entry` ends in the pack, useful to learn about the exact range of `entry` within the pack.
    pub entry_end: u64,
    /// The decompressed object itself, ready to be decoded.
    pub decompressed: &'a [u8],
    /// Custom state known to the function
    pub state: &'a mut S,
    /// The depth at which this object resides in the delta-tree. It represents the amount of base objects, with 0 indicating
    /// an 'undeltified' object, and higher values indicating delta objects with the given amount of bases.
    pub level: u16,
}

impl<T> Tree<T>
where
    T: Send,
{
    /// Traverse this tree of delta objects with a function `inspect_object` to process each object at will.
    ///
    /// * `should_run_in_parallel() -> bool` returns true if the underlying pack is big enough to warrant parallel traversal at all.
    /// * `resolve(EntrySlice, &mut Vec<u8>) -> Option<()>` resolves the bytes in the pack for the given `EntrySlice` and stores them in the
    ///   output vector. It returns `Some(())` if the object existed in the pack, or `None` to indicate a resolution error, which would abort the
    ///   operation as well.
    /// * `object_progress` is a progress instance to track progress for each object in the traversal.
    /// * `size_progress` is a progress instance to track the overall progress.
    /// * `tread_limit` is limits the amount of threads used if `Some` or otherwise defaults to all available logical cores.
    /// * `pack_entries_end` marks one-past-the-last byte of the last entry in the pack, as the last entries size would otherwise
    ///   be unknown as it's not part of the index file.
    /// * `new_thread_state() -> State` is a function to create state to be used in each thread, invoked once per thread.
    /// * `inspect_object(node_data: &mut T, progress: Progress, context: Context<ThreadLocal State>) -> Result<(), CustomError>` is a function
    ///   running for each thread receiving fully decoded objects along with contextual information, which either succceeds with `Ok(())`
    ///   or returns a `CustomError`.
    ///   Note that `node_data` can be modified to allow storing maintaining computation results on a per-object basis.
    /// * `object_hash` specifies what kind of hashes we expect to be stored in oid-delta entries, which is viable to decoding them
    ///   with the correct size.
    ///
    /// This method returns a vector of all tree items, along with their potentially modified custom node data.
    ///
    /// _Note_ that this method consumed the Tree to assure safe parallel traversal with mutation support.
    #[allow(clippy::too_many_arguments)]
    pub fn traverse<F, P1, P2, MBFN, S, E>(
        mut self,
        should_run_in_parallel: impl FnOnce() -> bool,
        resolve: F,
        object_progress: P1,
        size_progress: P2,
        thread_limit: Option<usize>,
        should_interrupt: &AtomicBool,
        pack_entries_end: u64,
        new_thread_state: impl Fn() -> S + Send + Clone,
        inspect_object: MBFN,
        object_hash: git_hash::Kind,
    ) -> Result<VecDeque<Item<T>>, Error>
    where
        F: for<'r> Fn(EntryRange, &'r mut Vec<u8>) -> Option<()> + Send + Clone,
        P1: Progress,
        P2: Progress,
        MBFN: Fn(&mut T, &mut <P1 as Progress>::SubProgress, Context<'_, S>) -> Result<(), E> + Send + Clone,
        E: std::error::Error + Send + Sync + 'static,
    {
        self.set_pack_entries_end(pack_entries_end);
        let (chunk_size, thread_limit, _) = parallel::optimize_chunk_size_and_thread_limit(1, None, thread_limit, None);
        let object_progress = OwnShared::new(Mutable::new(object_progress));

        let num_objects = self.items.len();
        in_parallel_if(
            should_run_in_parallel,
            self.iter_root_chunks(chunk_size),
            thread_limit,
            {
                let object_progress = object_progress.clone();
                move |thread_index| {
                    (
                        Vec::<u8>::with_capacity(4096),
                        lock(&object_progress).add_child(format!("thread {}", thread_index)),
                        new_thread_state(),
                        resolve.clone(),
                        inspect_object.clone(),
                    )
                }
            },
            move |root_nodes, state| resolve::deltas(root_nodes, state, object_hash.len_in_bytes()),
            Reducer::new(num_objects, object_progress, size_progress, should_interrupt),
        )?;
        Ok(self.into_items())
    }
}

struct Reducer<'a, P1, P2> {
    item_count: usize,
    progress: OwnShared<Mutable<P1>>,
    start: std::time::Instant,
    size_progress: P2,
    should_interrupt: &'a AtomicBool,
}

impl<'a, P1, P2> Reducer<'a, P1, P2>
where
    P1: Progress,
    P2: Progress,
{
    pub fn new(
        num_objects: usize,
        progress: OwnShared<Mutable<P1>>,
        mut size_progress: P2,
        should_interrupt: &'a AtomicBool,
    ) -> Self {
        lock(&progress).init(Some(num_objects), progress::count("objects"));
        size_progress.init(None, progress::bytes());
        Reducer {
            item_count: 0,
            progress,
            start: std::time::Instant::now(),
            size_progress,
            should_interrupt,
        }
    }
}

impl<'a, P1, P2> parallel::Reduce for Reducer<'a, P1, P2>
where
    P1: Progress,
    P2: Progress,
{
    type Input = Result<(usize, u64), Error>;
    type FeedProduce = ();
    type Output = ();
    type Error = Error;

    fn feed(&mut self, input: Self::Input) -> Result<(), Self::Error> {
        let (num_objects, decompressed_size) = input?;
        self.item_count += num_objects;
        self.size_progress.inc_by(decompressed_size as usize);
        lock(&self.progress).set(self.item_count);
        if self.should_interrupt.load(Ordering::SeqCst) {
            return Err(Error::Interrupted);
        }
        Ok(())
    }

    fn finalize(mut self) -> Result<Self::Output, Self::Error> {
        lock(&self.progress).show_throughput(self.start);
        self.size_progress.show_throughput(self.start);
        Ok(())
    }
}
