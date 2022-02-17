pub trait InsertImmutable<Item: Eq + std::hash::Hash> {
    fn insert(&self, item: Item) -> bool;
}

mod trait_impls {
    use std::{cell::RefCell, hash::Hash};

    use dashmap::DashSet;
    use hash_hasher::{HashBuildHasher, HashedSet};

    use super::InsertImmutable;

    impl<T: Eq + Hash> InsertImmutable<T> for DashSet<T, HashBuildHasher> {
        fn insert(&self, item: T) -> bool {
            self.insert(item)
        }
    }

    impl<T: Eq + Hash> InsertImmutable<T> for RefCell<HashedSet<T>> {
        fn insert(&self, item: T) -> bool {
            self.borrow_mut().insert(item)
        }
    }
}

pub struct Chunks<I> {
    pub size: usize,
    pub iter: I,
}

impl<I, Item> Iterator for Chunks<I>
where
    I: Iterator<Item = Item>,
{
    type Item = Vec<Item>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut res = Vec::with_capacity(self.size);
        let mut items_left = self.size;
        for item in &mut self.iter {
            res.push(item);
            items_left -= 1;
            if items_left == 0 {
                break;
            }
        }
        (!res.is_empty()).then(|| res)
    }
}
