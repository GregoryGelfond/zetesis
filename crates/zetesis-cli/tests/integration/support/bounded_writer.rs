//! A real bounded byte sink that preserves the successfully written prefix.

use std::io::{self, Write};

pub(crate) struct BoundedWriter {
    remaining: usize,
    bytes: Vec<u8>,
}

impl BoundedWriter {
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            remaining: capacity,
            bytes: Vec::new(),
        }
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

impl Write for BoundedWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        if self.remaining == 0 {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "diagnostic sink closed",
            ));
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
