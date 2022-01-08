use std::{
    collections::HashSet,
    time::{Duration, Instant},
};

use anyhow::anyhow;
use dashmap::DashSet;
use git_repository::{
    hash::ObjectId,
    objs::{bstr::BStr, tree::EntryRef},
    odb,
    prelude::*,
    refs::{file::ReferenceExt, peel},
    threading::OwnShared,
    traverse::{tree, tree::visit::Action},
};

const GITOXIDE_STATIC_CACHE_SIZE: usize = 64;
const GITOXIDE_CACHED_OBJECT_DATA_PER_THREAD_IN_BYTES: usize = 60_000_000;

fn main() -> anyhow::Result<()> {
    let (repo, commit_id) = {
        let mut args = std::env::args();
        let directory = args
            .nth(1)
            .ok_or_else(|| anyhow!("First argument is the .git directory to work in"))?;
        let repo = git_repository::discover(directory)?;
        let name = args.next().unwrap_or_else(|| "HEAD".into());
        let commit_id = repo
            .refs
            .find(&name)?
            .peel_to_id_in_place(&repo.refs, peel::none)?
            .to_owned();
        (repo, commit_id)
    };
    let db = repo.to_easy().objects;

    let start = Instant::now();
    let all_commits = commit_id
        .ancestors(|oid, buf| db.find_commit_iter(oid, buf).ok())
        .collect::<Result<Vec<_>, _>>()?;
    let elapsed = start.elapsed();
    println!(
        "gitoxide (uncached): collect all {} commits in {:?} ({:0.0} commits/s)",
        all_commits.len(),
        elapsed,
        all_commits.len() as f32 / elapsed.as_secs_f32()
    );

    for compute_mode in &[Computation::MultiThreaded, Computation::SingleThreaded] {
        let start = Instant::now();
        let (unique, entries) = do_gitoxide_tree_dag_traversal(
            &all_commits,
            db.store(),
            odb::pack::cache::lru::StaticLinkedList::<64>::default,
            *compute_mode,
        )?;
        let elapsed = start.elapsed();
        println!(
        "gitoxide {:?} (cache = 64 entries: confirmed {} entries ({} unique objects) in {} trees in {:?} ({:0.0} entries/s, {:0.0} trees/s)",
            compute_mode,
            entries,
            unique,
            all_commits.len(),
            elapsed,
            entries as f32 / elapsed.as_secs_f32(),
            all_commits.len() as f32 / elapsed.as_secs_f32()
        );
    }

    let repo = git2::Repository::open(repo.git_dir())?;
    let start = Instant::now();
    let (unique, entries) = do_libgit2_tree_dag_traversal(&all_commits, &repo)?;
    let elapsed = start.elapsed();
    println!(
        "libgit2: confirmed {} entries ({} unique objects) in {} trees in {:?} ({:0.0} entries/s, {:0.0} trees/s))",
        entries,
        unique,
        all_commits.len(),
        elapsed,
        entries as f32 / elapsed.as_secs_f32(),
        all_commits.len() as f32 / elapsed.as_secs_f32()
    );

    let start = Instant::now();
    let count = do_gitoxide_commit_graph_traversal(commit_id, db.clone(), || {
        odb::pack::cache::lru::MemoryCappedHashmap::new(GITOXIDE_CACHED_OBJECT_DATA_PER_THREAD_IN_BYTES)
    })?;
    let elapsed = start.elapsed();
    let objs_per_sec = |elapsed: Duration| count as f32 / elapsed.as_secs_f32();
    println!(
        "gitoxide (cache = {:.0}MB): confirmed {} commits in {:?} ({:0.0} commits/s)",
        GITOXIDE_CACHED_OBJECT_DATA_PER_THREAD_IN_BYTES as f32 / (1024 * 1024) as f32,
        count,
        elapsed,
        objs_per_sec(elapsed)
    );

    let start = Instant::now();
    let count = do_gitoxide_commit_graph_traversal(
        commit_id,
        db.clone(),
        odb::pack::cache::lru::StaticLinkedList::<GITOXIDE_STATIC_CACHE_SIZE>::default,
    )?;
    let elapsed = start.elapsed();
    let objs_per_sec = |elapsed: Duration| count as f32 / elapsed.as_secs_f32();
    println!(
        "gitoxide (static cache = {:.0} entries): confirmed {} commits in {:?} ({:0.0} commits/s)",
        GITOXIDE_STATIC_CACHE_SIZE,
        count,
        elapsed,
        objs_per_sec(elapsed)
    );

    let start = Instant::now();
    let count = do_gitoxide_commit_graph_traversal(commit_id, db, || odb::pack::cache::Never)?;
    let elapsed = start.elapsed();
    let objs_per_sec = |elapsed: Duration| count as f32 / elapsed.as_secs_f32();
    println!(
        "gitoxide (uncached): confirmed {} commits in {:?} ({:0.0} commits/s)",
        count,
        elapsed,
        objs_per_sec(elapsed)
    );

    let start = Instant::now();
    let count = do_libgit2_commit_graph_traversal(commit_id, &repo)?;
    let elapsed = start.elapsed();
    let objs_per_sec = |elapsed: Duration| count as f32 / elapsed.as_secs_f32();
    println!(
        "libgit2: confirmed {} commits in {:?} ({:0.0} commits/s)",
        count,
        elapsed,
        objs_per_sec(elapsed)
    );

    Ok(())
}

fn do_gitoxide_commit_graph_traversal<C>(
    tip: ObjectId,
    db: git_repository::OdbHandle,
    // TODO: make use of the cache
    _new_cache: impl FnOnce() -> C,
) -> anyhow::Result<usize>
where
    C: odb::pack::cache::DecodeEntry,
{
    let ancestors = tip.ancestors(|oid, buf| db.find_commit_iter(oid, buf).ok());
    let mut commits = 0;
    for commit_id in ancestors {
        let _ = commit_id?;
        commits += 1;
    }
    Ok(commits)
}

#[derive(Debug, Copy, Clone)]
enum Computation {
    SingleThreaded,
    MultiThreaded,
}

fn do_gitoxide_tree_dag_traversal<C>(
    commits: &[ObjectId],
    db: OwnShared<odb::Store>,
    new_cache: impl Fn() -> C + Send + Sync + Clone + 'static,
    mode: Computation,
) -> anyhow::Result<(usize, u64)>
where
    C: odb::pack::cache::DecodeEntry + Send + 'static,
{
    match mode {
        Computation::SingleThreaded => {
            #[derive(Default)]
            struct Count {
                entries: usize,
                seen: HashSet<ObjectId>,
            }

            impl tree::Visit for Count {
                fn pop_front_tracked_path_and_set_current(&mut self) {}
                fn push_back_tracked_path_component(&mut self, _component: &BStr) {}
                fn push_path_component(&mut self, _component: &BStr) {}
                fn pop_path_component(&mut self) {}
                fn visit_tree(&mut self, entry: &EntryRef<'_>) -> Action {
                    self.entries += 1;
                    let inserted = self.seen.insert(entry.oid.to_owned());
                    if !inserted {
                        tree::visit::Action::Skip
                    } else {
                        tree::visit::Action::Continue
                    }
                }
                fn visit_nontree(&mut self, entry: &EntryRef<'_>) -> Action {
                    self.entries += 1;
                    self.seen.insert(entry.oid.to_owned());
                    tree::visit::Action::Continue
                }
            }

            let mut buf = Vec::new();
            let mut buf2 = Vec::new();
            let mut state = tree::breadthfirst::State::default();
            let mut seen = HashSet::new();
            let mut entries = 0;

            let db = db.to_cache().with_pack_cache(move || Box::new(new_cache()));
            for commit in commits {
                let tree_id = db
                    .try_find(commit, &mut buf)?
                    .and_then(|o| o.try_into_commit_iter().and_then(|mut c| c.tree_id()))
                    .expect("commit as starting point");

                let mut count = Count { entries: 0, seen };
                db.find_tree_iter(tree_id, &mut buf2)?.traverse(
                    &mut state,
                    |oid, buf| db.find(oid, buf).ok().and_then(|o| o.try_into_tree_iter()),
                    &mut count,
                )?;
                entries += count.entries as u64;
                seen = count.seen;
            }
            Ok((seen.len(), entries))
        }
        Computation::MultiThreaded => {
            struct Count<'a> {
                entries: usize,
                seen: &'a DashSet<ObjectId>,
            }

            impl<'a> tree::Visit for Count<'a> {
                fn pop_front_tracked_path_and_set_current(&mut self) {}
                fn push_back_tracked_path_component(&mut self, _component: &BStr) {}
                fn push_path_component(&mut self, _component: &BStr) {}
                fn pop_path_component(&mut self) {}
                fn visit_tree(&mut self, entry: &EntryRef<'_>) -> Action {
                    self.entries += 1;
                    let inserted = self.seen.insert(entry.oid.to_owned());
                    if !inserted {
                        tree::visit::Action::Skip
                    } else {
                        tree::visit::Action::Continue
                    }
                }
                fn visit_nontree(&mut self, entry: &EntryRef<'_>) -> Action {
                    self.entries += 1;
                    self.seen.insert(entry.oid.to_owned());
                    tree::visit::Action::Continue
                }
            }
            use rayon::prelude::*;
            let seen = DashSet::new();
            let entries = std::sync::atomic::AtomicU64::new(0);

            commits
                .into_par_iter()
                .try_for_each_init::<_, _, _, anyhow::Result<_>>(
                    {
                        let seen = &seen;
                        {
                            move || {
                                (
                                    Count { entries: 0, seen },
                                    Vec::<u8>::new(),
                                    Vec::<u8>::new(),
                                    tree::breadthfirst::State::default(),
                                    db.to_cache().with_pack_cache({
                                        let new_cache = new_cache.clone();
                                        move || Box::new(new_cache())
                                    }),
                                )
                            }
                        }
                    },
                    |(count, buf, buf2, state, db), commit| {
                        let tid = db
                            .find_commit_iter(commit, buf)?
                            .tree_id()
                            .expect("commit as starting point");
                        count.entries = 0;
                        db.find_tree_iter(tid, buf2)?.traverse(
                            state,
                            |oid, buf| db.find_tree_iter(oid, buf).ok(),
                            count,
                        )?;
                        entries.fetch_add(count.entries as u64, std::sync::atomic::Ordering::Relaxed);
                        Ok(())
                    },
                )?;
            Ok((seen.len(), entries.load(std::sync::atomic::Ordering::Acquire)))
        }
    }
}

fn do_libgit2_tree_dag_traversal(commits: &[ObjectId], db: &git2::Repository) -> anyhow::Result<(usize, u64)> {
    let mut entries = 0;
    let mut seen = HashSet::new();
    for commit in commits {
        let commit = db.find_commit(git2::Oid::from_bytes(commit.as_bytes())?)?;
        commit.tree()?.walk(git2::TreeWalkMode::PreOrder, |_path, entry| {
            entries += 1;
            let was_inserted = seen.insert(entry.id());
            if was_inserted {
                git2::TreeWalkResult::Ok
            } else {
                git2::TreeWalkResult::Skip
            }
        })?;
    }
    Ok((seen.len(), entries))
}

fn do_libgit2_commit_graph_traversal(tip: ObjectId, db: &git2::Repository) -> anyhow::Result<usize> {
    let mut commits = 0;
    let mut walk = db.revwalk()?;
    walk.push(git2::Oid::from_bytes(tip.as_bytes())?)?;

    for commit_id in walk {
        let _ = commit_id?;
        commits += 1;
    }
    Ok(commits)
}
