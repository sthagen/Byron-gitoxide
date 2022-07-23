use git_hash::ObjectId;
use git_object::{bstr::BStr, tree};

/// Represents any possible change in order to turn one tree into another.
pub enum Change {
    /// An entry was added, like the addition of a file or directory.
    Addition {
        /// The mode of the added entry.
        entry_mode: tree::EntryMode,
        /// The object id of the added entry.
        oid: ObjectId,
    },
    /// An entry was deleted, like the deletion of a file or directory.
    Deletion {
        /// The mode of the deleted entry.
        entry_mode: tree::EntryMode,
        /// The object id of the deleted entry.
        oid: ObjectId,
    },
    /// An entry was modified, e.g. changing the contents of a file adjusts its object id and turning
    /// a file into a symbolic link adjusts its mode.
    Modification {
        /// The mode of the entry before the modification.
        previous_entry_mode: tree::EntryMode,
        /// The object id of the entry before the modification.
        previous_oid: ObjectId,

        /// The mode of the entry after the modification.
        entry_mode: tree::EntryMode,
        /// The object id of the after before the modification.
        oid: ObjectId,
    },
}

/// What to do after a [Change] was [recorded][Visit::visit()].
#[derive(Clone, Copy, PartialOrd, PartialEq, Ord, Eq, Hash)]
pub enum Action {
    /// Continue the traversal of changes.
    Continue,
    /// Stop the traversal of changes, making this the last call to [visit(…)][Visit::visit()].
    Cancel,
}

impl Action {
    /// Returns true if this action means to stop the traversal.
    pub fn cancelled(&self) -> bool {
        matches!(self, Action::Cancel)
    }
}

/// A trait to allow responding to a traversal designed to figure out the [changes][Change]
/// to turn tree A into tree B.
pub trait Visit {
    /// Sets the full path path in front of the queue so future calls to push and pop components affect it instead.
    fn pop_front_tracked_path_and_set_current(&mut self);
    /// Append a `component` to the end of a path, which may be empty.
    fn push_back_tracked_path_component(&mut self, component: &BStr);
    /// Append a `component` to the end of a path, which may be empty.
    fn push_path_component(&mut self, component: &BStr);
    /// Removes the last component from the path, which may leave it empty.
    fn pop_path_component(&mut self);
    /// Record a `change` and return an instruction whether to continue or not.
    ///
    /// The implementation may use the current path to lean where in the tree the change is located.
    fn visit(&mut self, change: Change) -> Action;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_of_change() {
        assert_eq!(
            std::mem::size_of::<Change>(),
            46,
            "this type shouldn't grow without us knowing"
        )
    }
}
