/// The error returned when any config value couldn't be instantiated due to malformed input.
#[derive(Debug, thiserror::Error, Eq, PartialEq)]
#[allow(missing_docs)]
#[error("Could not decode '{input}': {message}")]
pub struct Error {
    pub message: &'static str,
    pub input: bstr::BString,
    #[source]
    pub utf8_err: Option<std::str::Utf8Error>,
}

impl Error {
    pub(crate) fn new(message: &'static str, input: impl Into<bstr::BString>) -> Self {
        Error {
            message,
            input: input.into(),
            utf8_err: None,
        }
    }

    pub(crate) fn with_err(mut self, err: std::str::Utf8Error) -> Self {
        self.utf8_err = Some(err);
        self
    }
}

mod normalize;
pub use normalize::{normalize, normalize_bstr, normalize_bstring};
