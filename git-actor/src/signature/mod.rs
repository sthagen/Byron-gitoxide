mod _ref {
    use bstr::ByteSlice;

    use crate::{signature::decode, Signature, SignatureRef};

    impl<'a> SignatureRef<'a> {
        /// Deserialize a signature from the given `data`.
        pub fn from_bytes<E>(data: &'a [u8]) -> Result<SignatureRef<'a>, nom::Err<E>>
        where
            E: nom::error::ParseError<&'a [u8]> + nom::error::ContextError<&'a [u8]>,
        {
            decode(data).map(|(_, t)| t)
        }

        /// Create an owned instance from this shared one.
        pub fn to_owned(&self) -> Signature {
            Signature {
                name: self.name.to_owned(),
                email: self.email.to_owned(),
                time: self.time,
            }
        }

        /// Trim whitespace surrounding the name and email and return a new signature.
        pub fn trim(&self) -> SignatureRef<'a> {
            SignatureRef {
                name: self.name.trim().as_bstr(),
                email: self.email.trim().as_bstr(),
                time: self.time,
            }
        }
    }
}

mod convert {
    use crate::{Signature, SignatureRef};

    impl Signature {
        /// An empty signature, similar to 'null'.
        pub fn empty() -> Self {
            Signature::default()
        }

        /// Borrow this instance as immutable
        pub fn to_ref(&self) -> SignatureRef<'_> {
            SignatureRef {
                name: self.name.as_ref(),
                email: self.email.as_ref(),
                time: self.time,
            }
        }
    }

    impl From<SignatureRef<'_>> for Signature {
        fn from(other: SignatureRef<'_>) -> Signature {
            let SignatureRef { name, email, time } = other;
            Signature {
                name: name.to_owned(),
                email: email.to_owned(),
                time,
            }
        }
    }
}

mod write {
    use std::io;

    use bstr::{BStr, ByteSlice};
    use quick_error::quick_error;

    use crate::{Signature, SignatureRef};

    quick_error! {
        /// The Error produced by [`Signature::write_to()`].
        #[derive(Debug)]
        #[allow(missing_docs)]
        enum Error {
            IllegalCharacter {
                display("Signature name or email must not contain '<', '>' or \\n")
            }
        }
    }

    impl From<Error> for io::Error {
        fn from(err: Error) -> Self {
            io::Error::new(io::ErrorKind::Other, err)
        }
    }

    /// Output
    impl Signature {
        /// Serialize this instance to `out` in the git serialization format for actors.
        pub fn write_to(&self, out: impl io::Write) -> io::Result<()> {
            self.to_ref().write_to(out)
        }
        /// Computes the number of bytes necessary to serialize this signature
        pub fn size(&self) -> usize {
            self.to_ref().size()
        }
    }

    impl<'a> SignatureRef<'a> {
        /// Serialize this instance to `out` in the git serialization format for actors.
        pub fn write_to(&self, mut out: impl io::Write) -> io::Result<()> {
            out.write_all(validated_token(self.name)?)?;
            out.write_all(b" ")?;
            out.write_all(b"<")?;
            out.write_all(validated_token(self.email)?)?;
            out.write_all(b"> ")?;
            self.time.write_to(out)
        }
        /// Computes the number of bytes necessary to serialize this signature
        pub fn size(&self) -> usize {
            self.name.len() + 2 /* space <*/ + self.email.len() +  2 /* > space */ + self.time.size()
        }
    }

    fn validated_token(name: &BStr) -> Result<&BStr, Error> {
        if name.find_byteset(b"<>\n").is_some() {
            return Err(Error::IllegalCharacter);
        }
        Ok(name)
    }
}

mod init {
    use bstr::BString;

    use crate::{Signature, Time};

    impl Signature {
        /// Return an actor identified `name` and `email` at the current local time, that is a time with a timezone offset from
        /// UTC based on the hosts configuration.
        #[cfg(feature = "local-time-support")]
        pub fn now_local(
            name: impl Into<BString>,
            email: impl Into<BString>,
        ) -> Result<Self, git_features::time::tz::Error> {
            let offset = git_features::time::tz::current_utc_offset()?;
            Ok(Signature {
                name: name.into(),
                email: email.into(),
                time: Time {
                    seconds_since_unix_epoch: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .expect("the system time doesn't run backwards that much")
                        .as_secs() as u32,
                    offset_in_seconds: offset,
                    sign: offset.into(),
                },
            })
        }

        /// Return an actor identified `name` and `email` at the current local time, or UTC time if the current time zone could
        /// not be obtained.
        #[cfg(feature = "local-time-support")]
        pub fn now_local_or_utc(name: impl Into<BString>, email: impl Into<BString>) -> Self {
            let offset = git_features::time::tz::current_utc_offset().unwrap_or(0);
            Signature {
                name: name.into(),
                email: email.into(),
                time: Time {
                    seconds_since_unix_epoch: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .expect("the system time doesn't run backwards that much")
                        .as_secs() as u32,
                    offset_in_seconds: offset,
                    sign: offset.into(),
                },
            }
        }

        /// Return an actor identified by `name` and `email` at the current time in UTC.
        ///
        /// This would be most useful for bot users, otherwise the [`now_local()`][Signature::now_local()] method should be preferred.
        pub fn now_utc(name: impl Into<BString>, email: impl Into<BString>) -> Self {
            let utc_offset = 0;
            Signature {
                name: name.into(),
                email: email.into(),
                time: Time {
                    seconds_since_unix_epoch: seconds_since_unix_epoch(),
                    offset_in_seconds: utc_offset,
                    sign: utc_offset.into(),
                },
            }
        }
    }

    fn seconds_since_unix_epoch() -> u32 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("the system time doesn't run backwards that much")
            .as_secs() as u32
    }
}

///
mod decode;
pub use decode::decode;
