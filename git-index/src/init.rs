mod from_tree {
    use crate::{
        entry::{Flags, Mode, Stat},
        Entry, PathStorage, State, Version,
    };
    use bstr::{BStr, BString, ByteSlice, ByteVec};
    use git_object::{
        tree::{self, EntryMode},
        TreeRefIter,
    };
    use git_traverse::tree::{breadthfirst, visit::Action, Visit};
    use std::collections::VecDeque;

    /// Initialization
    impl State {
        /// Create an index [`State`][crate::State] by traversing `tree` recursively, accessing sub-trees
        /// with `find`.
        ///
        /// **No extension data is currently produced**.
        pub fn from_tree<Find>(tree: &git_hash::oid, mut find: Find) -> Result<Self, breadthfirst::Error>
        where
            Find: for<'a> FnMut(&git_hash::oid, &'a mut Vec<u8>) -> Option<TreeRefIter<'a>>,
        {
            let mut buf = Vec::new();
            let root = find(tree, &mut buf).ok_or(breadthfirst::Error::NotFound { oid: tree.into() })?;

            let mut delegate = CollectEntries::new();
            breadthfirst(root, breadthfirst::State::default(), &mut find, &mut delegate)?;

            let CollectEntries {
                mut entries,
                path_backing,
                path: _,
                path_deque: _,
            } = delegate;

            entries.sort_by(|a, b| Entry::cmp_filepaths(a.path_in(&path_backing), b.path_in(&path_backing)));

            Ok(State {
                object_hash: tree.kind(),
                timestamp: filetime::FileTime::now(),
                version: Version::V2,
                entries,
                path_backing,
                is_sparse: false,
                tree: None,
                link: None,
                resolve_undo: None,
                untracked: None,
                fs_monitor: None,
            })
        }
    }

    struct CollectEntries {
        entries: Vec<Entry>,
        path_backing: PathStorage,
        path: BString,
        path_deque: VecDeque<BString>,
    }

    impl CollectEntries {
        pub fn new() -> CollectEntries {
            CollectEntries {
                entries: Vec::new(),
                path_backing: Vec::new(),
                path: BString::default(),
                path_deque: VecDeque::new(),
            }
        }

        fn push_element(&mut self, name: &BStr) {
            if !self.path.is_empty() {
                self.path.push(b'/');
            }
            self.path.push_str(name);
        }

        pub fn add_entry(&mut self, entry: &tree::EntryRef<'_>) {
            let mode = match entry.mode {
                EntryMode::Tree => unreachable!("visit_non_tree() called us"),
                EntryMode::Blob => Mode::FILE,
                EntryMode::BlobExecutable => Mode::FILE_EXECUTABLE,
                EntryMode::Link => Mode::SYMLINK,
                EntryMode::Commit => Mode::COMMIT,
            };

            let path_start = self.path_backing.len();
            self.path_backing.extend_from_slice(&self.path);

            let new_entry = Entry {
                stat: Stat::default(),
                id: entry.oid.into(),
                flags: Flags::empty(),
                mode,
                path: path_start..self.path_backing.len(),
            };

            self.entries.push(new_entry);
        }
    }

    impl Visit for CollectEntries {
        fn pop_front_tracked_path_and_set_current(&mut self) {
            self.path = self
                .path_deque
                .pop_front()
                .expect("every call is matched with push_tracked_path_component");
        }

        fn push_back_tracked_path_component(&mut self, component: &bstr::BStr) {
            self.push_element(component);
            self.path_deque.push_back(self.path.clone());
        }

        fn push_path_component(&mut self, component: &bstr::BStr) {
            self.push_element(component);
        }

        fn pop_path_component(&mut self) {
            if let Some(pos) = self.path.rfind_byte(b'/') {
                self.path.resize(pos, 0);
            } else {
                self.path.clear();
            }
        }

        fn visit_tree(&mut self, _entry: &git_object::tree::EntryRef<'_>) -> git_traverse::tree::visit::Action {
            Action::Continue
        }

        fn visit_nontree(&mut self, entry: &git_object::tree::EntryRef<'_>) -> git_traverse::tree::visit::Action {
            self.add_entry(entry);
            Action::Continue
        }
    }
}
