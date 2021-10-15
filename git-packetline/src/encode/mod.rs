use quick_error::quick_error;

use crate::MAX_DATA_LEN;

quick_error! {
    /// The error returned by most functions in the [`encode`][crate::encode] module
    #[derive(Debug)]
    #[allow(missing_docs)]
    pub enum Error {
        DataLengthLimitExceeded(length_in_bytes: usize) {
            display("Cannot encode more than {} bytes, got {}", MAX_DATA_LEN, length_in_bytes)
        }
        DataIsEmpty {
            display("Empty lines are invalid")
        }
    }
}

#[cfg(all(not(feature = "blocking-io"), feature = "async-io"))]
mod async_io;
#[cfg(all(not(feature = "blocking-io"), feature = "async-io"))]
pub use async_io::*;

#[cfg(feature = "blocking-io")]
mod blocking_io;
#[cfg(feature = "blocking-io")]
pub use blocking_io::*;

pub(crate) fn u16_to_hex(value: u16) -> [u8; 4] {
    let mut buf = [0u8; 4];
    hex::encode_to_slice((value as u16).to_be_bytes(), &mut buf).expect("two bytes to 4 hex chars never fails");
    buf
}
