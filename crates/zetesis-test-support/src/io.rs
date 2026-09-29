//! Sinks that fail on purpose, for tests of output failure paths.
//!
//! Each models one failure a destination can meet: a bounded destination that
//! fills ([`BoundedWriter`], and [`BoundedText`] for text), a closed stream
//! ([`Closed`]), and a failure at a chosen record ([`FailAt`]). A failing byte
//! sink returns a broken pipe whose message a test can compare.

use std::fmt;
use std::io::{self, Write};

/// The message of the error a full [`BoundedWriter`] returns.
pub const FULL: &str = "bounded sink full";

/// The message of the error a [`Closed`] sink returns.
pub const CLOSED: &str = "sink closed";

/// The message of the error a [`FailAt`] sink returns from its marker on.
pub const FAILED: &str = "sink failed at its marker";

/// A byte sink that accepts at most a fixed number of bytes and keeps the
/// prefix it accepted.
///
/// A write that would exceed the capacity is accepted in part. Once the sink
/// is full, every non-empty write fails with a broken pipe ([`FULL`]); an
/// empty write succeeds.
#[derive(Debug)]
pub struct BoundedWriter {
    remaining: usize,
    bytes: Vec<u8>,
}

impl BoundedWriter {
    /// A sink accepting at most `capacity` bytes.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            remaining: capacity,
            bytes: Vec::new(),
        }
    }

    /// The bytes accepted so far.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

impl Write for BoundedWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        if self.remaining == 0 {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, FULL));
        }
        let count = bytes.len().min(self.remaining);
        self.bytes.extend_from_slice(&bytes[..count]);
        self.remaining -= count;
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// A byte sink whose every write fails with a broken pipe ([`CLOSED`]), as a
/// closed stream does. Flushing succeeds.
#[derive(Debug)]
pub struct Closed;

impl Write for Closed {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::new(io::ErrorKind::BrokenPipe, CLOSED))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// A byte sink that accepts every write until one contains its marker, then
/// fails that write and every later one with a broken pipe ([`FAILED`]).
///
/// A test places the failure at a chosen record, such as the first line of a
/// statistics block, without affecting the records before it.
#[derive(Debug)]
pub struct FailAt {
    marker: &'static [u8],
    failed: bool,
}

impl FailAt {
    /// A sink failing from the first write containing `marker`.
    #[must_use]
    pub fn new(marker: &'static [u8]) -> Self {
        Self {
            marker,
            failed: false,
        }
    }
}

impl Write for FailAt {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.failed |= bytes
            .windows(self.marker.len())
            .any(|window| window == self.marker);
        if self.failed {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, FAILED));
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// A text sink that accepts at most a fixed number of bytes and keeps the
/// text it accepted.
///
/// [`fmt::Write`] has no partial writes, so a write that would exceed the
/// capacity is refused whole.
#[derive(Debug)]
pub struct BoundedText {
    capacity: usize,
    text: String,
}

impl BoundedText {
    /// A sink accepting at most `capacity` bytes of text.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            text: String::new(),
        }
    }

    /// The text accepted so far.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }
}

impl fmt::Write for BoundedText {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        if text.len() > self.capacity - self.text.len() {
            return Err(fmt::Error);
        }
        self.text.push_str(text);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::Write as _;

    #[test]
    fn a_bounded_writer_keeps_the_accepted_prefix_and_then_refuses() {
        let mut sink = BoundedWriter::new(3);
        assert_eq!(sink.write(b"ab").unwrap(), 2);
        assert_eq!(sink.write(b"cde").unwrap(), 1);
        let error = sink.write(b"f").unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
        assert_eq!(error.to_string(), FULL);
        assert_eq!(sink.bytes(), b"abc");
    }

    #[test]
    fn a_full_bounded_writer_accepts_an_empty_write() {
        let mut sink = BoundedWriter::new(0);
        assert_eq!(sink.write(b"").unwrap(), 0);
    }

    #[test]
    fn a_closed_sink_refuses_every_write_with_a_broken_pipe() {
        let error = Closed.write(b"a").unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
        assert_eq!(error.to_string(), CLOSED);
        assert!(Closed.flush().is_ok());
    }

    #[test]
    fn a_fail_at_sink_fails_from_the_write_containing_its_marker() {
        let mut sink = FailAt::new(b"Statistics:");
        assert_eq!(sink.write(b"Answer: 1\n").unwrap(), 10);
        let error = sink.write(b"Statistics: 1\n").unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
        assert_eq!(error.to_string(), FAILED);
        assert!(sink.write(b"later\n").is_err());
    }

    #[test]
    fn bounded_text_refuses_a_write_that_would_exceed_it_whole() {
        let mut sink = BoundedText::new(4);
        sink.write_str("abc").unwrap();
        assert!(sink.write_str("de").is_err());
        assert_eq!(sink.text(), "abc");
        sink.write_str("d").unwrap();
        assert_eq!(sink.text(), "abcd");
    }
}
