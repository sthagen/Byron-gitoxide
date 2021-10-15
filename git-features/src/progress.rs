//! Various `prodash` types along with various utilities for comfort.
use std::io;

pub use prodash::{
    progress::{Discard, DoOrDiscard, Either, ThroughputOnDrop},
    unit, Progress, Unit,
};

/// A unit for displaying bytes with throughput and progress percentage.
pub fn bytes() -> Option<Unit> {
    Some(unit::dynamic_and_mode(
        unit::Bytes,
        unit::display::Mode::with_throughput().and_percentage(),
    ))
}

/// A unit for displaying human readable numbers with throughput and progress percentage.
pub fn count(name: &'static str) -> Option<Unit> {
    Some(unit::dynamic_and_mode(
        unit::Human::new(
            {
                let mut f = unit::human::Formatter::new();
                f.with_decimals(1);
                f
            },
            name,
        ),
        unit::display::Mode::with_throughput().and_percentage(),
    ))
}

/// A predefined unit for displaying a multi-step progress
pub fn steps() -> Option<Unit> {
    Some(unit::dynamic(unit::Range::new("steps")))
}

/// A structure passing every [`read`][std::io::Read::read()] call through to the contained Progress instance using [`inc_by(bytes_read)`][Progress::inc_by()].
pub struct Read<R, P> {
    /// The implementor of [`std::io::Read`] to which progress is added
    pub reader: R,
    /// The progress instance receiving progress information on each invocation of `reader`
    pub progress: P,
}

impl<R, P> io::Read for Read<R, P>
where
    R: io::Read,
    P: Progress,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let bytes_read = self.reader.read(buf)?;
        self.progress.inc_by(bytes_read as usize);
        Ok(bytes_read)
    }
}

impl<R, P> io::BufRead for Read<R, P>
where
    R: io::BufRead,
    P: Progress,
{
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        self.reader.fill_buf()
    }

    fn consume(&mut self, amt: usize) {
        self.reader.consume(amt)
    }
}
