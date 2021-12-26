use std::{fs, io::Read, path::PathBuf};

use git_features::zlib;

use crate::store_impls::loose::{hash_path, Store, HEADER_READ_UNCOMPRESSED_BYTES};

/// Returned by [`Store::try_find()`]
#[derive(thiserror::Error, Debug)]
#[allow(missing_docs)]
pub enum Error {
    #[error("decompression of loose object at '{path}' failed")]
    DecompressFile {
        source: zlib::inflate::Error,
        path: PathBuf,
    },
    #[error(transparent)]
    Decode(#[from] git_object::decode::LooseHeaderDecodeError),
    #[error("Could not {action} data at '{path}'")]
    Io {
        source: std::io::Error,
        action: &'static str,
        path: PathBuf,
    },
}

/// Object lookup
impl Store {
    const OPEN_ACTION: &'static str = "open";

    /// Returns true if the given id is contained in our repository.
    pub fn contains(&self, id: impl AsRef<git_hash::oid>) -> bool {
        debug_assert_eq!(self.object_hash, id.as_ref().kind());
        hash_path(id.as_ref(), self.path.clone()).is_file()
    }

    /// Return the object identified by the given [`ObjectId`][git_hash::ObjectId] if present in this database,
    /// writing its raw data into the given `out` buffer.
    ///
    /// Returns `Err` if there was an error locating or reading the object. Returns `Ok<None>` if
    /// there was no such object.
    pub fn try_find<'a>(
        &self,
        id: impl AsRef<git_hash::oid>,
        out: &'a mut Vec<u8>,
    ) -> Result<Option<git_object::Data<'a>>, Error> {
        debug_assert_eq!(self.object_hash, id.as_ref().kind());
        match self.find_inner(id.as_ref(), out) {
            Ok(obj) => Ok(Some(obj)),
            Err(err) => match err {
                Error::Io {
                    source: err,
                    action,
                    path,
                } => {
                    if action == Self::OPEN_ACTION && err.kind() == std::io::ErrorKind::NotFound {
                        Ok(None)
                    } else {
                        Err(Error::Io {
                            source: err,
                            action,
                            path,
                        })
                    }
                }
                err => Err(err),
            },
        }
    }

    fn find_inner<'a>(&self, id: &git_hash::oid, buf: &'a mut Vec<u8>) -> Result<git_object::Data<'a>, Error> {
        let path = hash_path(id, self.path.clone());

        let mut inflate = zlib::Inflate::default();
        let ((status, consumed_in, consumed_out), bytes_read) = {
            let mut istream = fs::File::open(&path).map_err(|e| Error::Io {
                source: e,
                action: Self::OPEN_ACTION,
                path: path.to_owned(),
            })?;

            buf.clear();
            let bytes_read = istream.read_to_end(buf).map_err(|e| Error::Io {
                source: e,
                action: "read",
                path: path.to_owned(),
            })?;
            buf.resize(bytes_read + HEADER_READ_UNCOMPRESSED_BYTES, 0);
            let (input, output) = buf.split_at_mut(bytes_read);
            (
                inflate
                    .once(&input[..bytes_read], output)
                    .map_err(|e| Error::DecompressFile {
                        source: e,
                        path: path.to_owned(),
                    })?,
                bytes_read,
            )
        };
        assert_ne!(
            status,
            zlib::Status::BufError,
            "Buffer errors might mean we encountered huge headers"
        );

        let decompressed_start = bytes_read;
        let (kind, size, header_size) =
            git_object::decode::loose_header(&buf[decompressed_start..decompressed_start + consumed_out])?;

        if status == zlib::Status::StreamEnd {
            let decompressed_body_bytes_sans_header =
                decompressed_start + header_size..decompressed_start + consumed_out;
            assert_eq!(
                consumed_out,
                size + header_size,
                "At this point we have decompressed everything and given 'size' should match"
            );
            buf.copy_within(decompressed_body_bytes_sans_header, 0);
        } else {
            buf.resize(bytes_read + size + header_size, 0);
            {
                let (input, output) = buf.split_at_mut(bytes_read);
                let num_decompressed_bytes = zlib::stream::inflate::read(
                    &mut &input[consumed_in..],
                    &mut inflate.state,
                    &mut output[consumed_out..],
                )
                .map_err(|e| Error::Io {
                    source: e,
                    action: "deflate",
                    path: path.to_owned(),
                })?;
                assert_eq!(
                    num_decompressed_bytes + consumed_out,
                    size + header_size,
                    "Object should have been decompressed entirely and match given 'size'"
                );
            };
            buf.copy_within(decompressed_start + header_size.., 0);
        }
        buf.resize(size, 0);
        Ok(git_object::Data { kind, data: buf })
    }
}
