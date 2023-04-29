use crate::{
    config,
    config::tree::{keys, Index, Key, Section},
};

impl Index {
    /// The `index.threads` key.
    pub const THREADS: IndexThreads =
        IndexThreads::new_with_validate("threads", &config::Tree::INDEX, validate::IndexThreads);
}

/// The `index.threads` key.
pub type IndexThreads = keys::Any<validate::IndexThreads>;

mod index_threads {
    use crate::bstr::BStr;
    use crate::config;
    use crate::config::key::GenericErrorWithValue;
    use crate::config::tree::index::IndexThreads;
    use std::borrow::Cow;

    impl IndexThreads {
        /// Parse `value` into the amount of threads to use, with `1` being single-threaded, or `0` indicating
        /// to select the amount of threads, with any other number being the specific amount of threads to use.
        pub fn try_into_index_threads(
            &'static self,
            value: Cow<'_, BStr>,
        ) -> Result<usize, config::key::GenericErrorWithValue> {
            gix_config::Integer::try_from(value.as_ref())
                .ok()
                .and_then(|i| i.to_decimal().and_then(|i| i.try_into().ok()))
                .or_else(|| {
                    gix_config::Boolean::try_from(value.as_ref())
                        .ok()
                        .map(|b| if b.0 { 0 } else { 1 })
                })
                .ok_or_else(|| GenericErrorWithValue::from_value(self, value.into_owned()))
        }
    }
}

impl Section for Index {
    fn name(&self) -> &str {
        "index"
    }

    fn keys(&self) -> &[&dyn Key] {
        &[&Self::THREADS]
    }
}

mod validate {
    use crate::{bstr::BStr, config::tree::keys};

    pub struct IndexThreads;
    impl keys::Validate for IndexThreads {
        fn validate(&self, value: &BStr) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
            super::Index::THREADS.try_into_index_threads(value.into())?;
            Ok(())
        }
    }
}
