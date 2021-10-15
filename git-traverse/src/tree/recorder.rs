use git_hash::ObjectId;
use git_object::{
    bstr::{BStr, BString, ByteSlice, ByteVec},
    tree,
};

use crate::tree::{visit::Action, Recorder, Visit};

/// An owned entry as observed by a call to [`visit_(tree|nontree)(…)`][Visit::visit_tree()], enhanced with the full path to it.
/// Otherwise similar to [`git_object::tree::EntryRef`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// The kind of entry, similar to entries in a unix directory tree.
    pub mode: tree::EntryMode,
    /// The full path to the entry. A root entry would be `d`, and a file `a` within the directory would be `d/a`.
    ///
    /// This is independent of the platform and the path separators actually used there.
    pub filepath: BString,
    /// The id of the entry which can be used to locate it in an object database.
    pub oid: ObjectId,
}

impl Entry {
    fn new(entry: &tree::EntryRef<'_>, filepath: BString) -> Self {
        Entry {
            filepath,
            oid: entry.oid.to_owned(),
            mode: entry.mode,
        }
    }
}

impl Recorder {
    fn pop_element(&mut self) {
        if let Some(pos) = self.path.rfind_byte(b'/') {
            self.path.resize(pos, 0);
        } else {
            self.path.clear();
        }
    }

    fn push_element(&mut self, name: &BStr) {
        if !self.path.is_empty() {
            self.path.push(b'/');
        }
        self.path.push_str(name);
    }

    fn path_clone(&self) -> BString {
        self.path.clone()
    }
}

impl Visit for Recorder {
    fn pop_front_tracked_path_and_set_current(&mut self) {
        self.path = self
            .path_deque
            .pop_front()
            .expect("every call is matched with push_tracked_path_component");
    }

    fn push_back_tracked_path_component(&mut self, component: &BStr) {
        self.push_element(component);
        self.path_deque.push_back(self.path.clone());
    }

    fn push_path_component(&mut self, component: &BStr) {
        self.push_element(component);
    }

    fn pop_path_component(&mut self) {
        self.pop_element();
    }

    fn visit_tree(&mut self, entry: &tree::EntryRef<'_>) -> Action {
        self.records.push(Entry::new(entry, self.path_clone()));
        Action::Continue
    }

    fn visit_nontree(&mut self, entry: &tree::EntryRef<'_>) -> Action {
        self.records.push(Entry::new(entry, self.path_clone()));
        Action::Continue
    }
}
