pub(crate) const SIZE: usize = 4 /*signature*/ + 4 /*version*/ + 4 /* num entries */;

use crate::{util::from_be_u32, Version};

mod error {
    use quick_error::quick_error;

    quick_error! {
        #[derive(Debug)]
        pub enum Error {
            Corrupt(message: &'static str) {
                display("{}", message)
            }
            UnsupportedVersion(version: u32) {
                display("Index version {} is not supported", version)
            }
        }
    }
}

pub use error::Error;

pub(crate) fn decode(data: &[u8], object_hash: git_hash::Kind) -> Result<(crate::Version, u32, &[u8]), Error> {
    if data.len() < (3 * 4) + object_hash.len_in_bytes() {
        return Err(Error::Corrupt(
            "File is too small even for header with zero entries and smallest hash",
        ));
    }

    const SIGNATURE: &[u8] = b"DIRC";
    let (signature, data) = data.split_at(4);
    if signature != SIGNATURE {
        return Err(Error::Corrupt(
            "Signature mismatch - this doesn't claim to be a header file",
        ));
    }

    let (version, data) = data.split_at(4);
    let version = match from_be_u32(version) {
        2 => Version::V2,
        3 => Version::V3,
        4 => Version::V4,
        unknown => return Err(Error::UnsupportedVersion(unknown)),
    };
    let (entries, data) = data.split_at(4);
    let entries = from_be_u32(entries);

    Ok((version, entries, data))
}
