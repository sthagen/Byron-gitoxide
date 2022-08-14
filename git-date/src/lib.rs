//! Date and time parsing similar to what git can do.
//!
//! Note that this is not a general purpose time library.
//! ## Feature Flags
#![cfg_attr(
    feature = "document-features",
    cfg_attr(doc, doc = ::document_features::document_features!())
)]
#![forbid(unsafe_code)]
#![deny(missing_docs, rust_2018_idioms)]

///
pub mod time;

mod parse;
pub use parse::parse;

/// A timestamp with timezone.
#[derive(PartialEq, Eq, Debug, Hash, Ord, PartialOrd, Clone, Copy)]
#[cfg_attr(feature = "serde1", derive(serde::Serialize, serde::Deserialize))]
pub struct Time {
    /// time in seconds since epoch.
    pub seconds_since_unix_epoch: u32,
    /// time offset in seconds, may be negative to match the `sign` field.
    pub offset_in_seconds: i32,
    /// the sign of `offset`, used to encode `-0000` which would otherwise loose sign information.
    pub sign: time::Sign,
}
