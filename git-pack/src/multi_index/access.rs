use std::path::{Path, PathBuf};

use byteorder::{BigEndian, ByteOrder};

use crate::{
    data,
    multi_index::{File, PackIndex, Version},
};

/// Represents an entry within a multi index file, effectively mapping object [`IDs`][git_hash::ObjectId] to pack data
/// files and the offset within.
#[derive(PartialEq, Eq, Debug, Hash, Ord, PartialOrd, Clone)]
#[cfg_attr(feature = "serde1", derive(serde::Serialize, serde::Deserialize))]
pub struct Entry {
    /// The ID of the object.
    pub oid: git_hash::ObjectId,
    /// The offset to the object's header in the pack data file.
    pub pack_offset: data::Offset,
    /// The index of the pack matching our [`File::index_names()`] slice.
    pub pack_index: PackIndex,
}

/// Access methods
impl File {
    /// Returns the verion of the multi-index file.
    pub fn version(&self) -> Version {
        self.version
    }
    /// Returns the path from which the multi-index file was loaded.
    ///
    /// Note that it might have changed in the mean time, or might have been removed as well.
    pub fn path(&self) -> &Path {
        &self.path
    }
    /// Returns the amount of indices stored in this multi-index file. It's the same as [File::index_names().len()][File::index_names()].
    pub fn num_indices(&self) -> u32 {
        self.num_indices
    }
    /// Returns the total amount of objects available for lookup.
    pub fn num_objects(&self) -> u32 {
        self.num_objects
    }
    /// Returns the kind of hash function used for object ids available in this index.
    pub fn object_hash(&self) -> git_hash::Kind {
        self.object_hash
    }
    /// Returns the checksum over the entire content of the file (excluding the checksum itself).
    ///
    /// It can be used to validate it didn't change after creation.
    pub fn checksum(&self) -> git_hash::ObjectId {
        git_hash::ObjectId::from(&self.data[self.data.len() - self.hash_len..])
    }
    /// Return all names of index files (`*.idx`) whose objects we contain.
    ///
    /// The corresponding pack can be found by replacing the `.idx` extension with `.pack`.
    pub fn index_names(&self) -> &[PathBuf] {
        &self.index_names
    }
}

impl File {
    /// Return the object id at the given `index`, which ranges from 0 to [File::num_objects()].
    pub fn oid_at_index(&self, index: u32) -> &git_hash::oid {
        debug_assert!(index < self.num_objects, "index out of bounds");
        let index: usize = index as usize;
        let start = self.lookup_ofs + index * self.hash_len;
        git_hash::oid::from_bytes_unchecked(&self.data[start..][..self.hash_len])
    }

    /// Find the index ranging from 0 to [File::num_objects()] that belongs to data associated with `id`, or `None` if it wasn't found.
    ///
    /// Use this index for finding additional information via [`File::pack_offset_and_pack_id_at_index()`].
    pub fn lookup(&self, id: impl AsRef<git_hash::oid>) -> Option<u32> {
        let id = id.as_ref();
        let first_byte = id.first_byte() as usize;
        let mut upper_bound = self.fan[first_byte];
        let mut lower_bound = if first_byte != 0 { self.fan[first_byte - 1] } else { 0 };

        // Bisect using indices
        while lower_bound < upper_bound {
            let mid = (lower_bound + upper_bound) / 2;
            let mid_sha = self.oid_at_index(mid);

            use std::cmp::Ordering::*;
            match id.cmp(mid_sha) {
                Less => upper_bound = mid,
                Equal => return Some(mid),
                Greater => lower_bound = mid + 1,
            }
        }
        None
    }

    /// Given the `index` ranging from 0 to [File::num_objects()], return the pack index and its absolute offset into the pack.
    ///
    /// The pack-index refers to an entry in the [`index_names`][File::index_names()] list, from which the pack can be derived.
    pub fn pack_offset_and_pack_id_at_index(&self, index: u32) -> (PackIndex, data::Offset) {
        const OFFSET_ENTRY_SIZE: usize = 4 + 4;
        let index = index as usize;
        let start = self.offsets_ofs + index * OFFSET_ENTRY_SIZE;

        const HIGH_BIT: u32 = 1 << 31;

        let pack_index = BigEndian::read_u32(&self.data[start..][..4]);
        let offset = &self.data[start + 4..][..4];
        let ofs32 = BigEndian::read_u32(offset);
        let pack_offset = if (ofs32 & HIGH_BIT) == HIGH_BIT {
            let offsets_64 = self
                .large_offsets_ofs
                .expect("non-malformed file that has large offsets if these are contained");
            let from = offsets_64 + (ofs32 ^ HIGH_BIT) as usize * 8;
            BigEndian::read_u64(&self.data[from..][..8])
        } else {
            ofs32 as u64
        };
        (pack_index, pack_offset)
    }

    /// Return an iterator over all entries within this file.
    pub fn iter(&self) -> impl Iterator<Item = Entry> + '_ {
        (0..self.num_objects).map(move |idx| {
            let (pack_index, pack_offset) = self.pack_offset_and_pack_id_at_index(idx);
            Entry {
                oid: self.oid_at_index(idx).to_owned(),
                pack_offset,
                pack_index,
            }
        })
    }
}
