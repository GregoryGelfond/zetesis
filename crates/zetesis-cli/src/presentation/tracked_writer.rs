//! A publication attempt is observable even when the sink returns an error.

use std::io::{self, Write};

/// A failed writer can already have modified its sink. Tracking calls rather
/// than accepted bytes prevents appending a second machine document to a prefix.
pub(crate) struct TrackedWriter<'a, W> {
    output: &'a mut W,
    attempted: bool,
}

impl<'a, W> TrackedWriter<'a, W> {
    pub(crate) fn new(output: &'a mut W) -> Self {
        Self {
            output,
            attempted: false,
        }
    }

    pub(crate) const fn write_attempted(&self) -> bool {
        self.attempted
    }
}

impl<W: Write> Write for TrackedWriter<'_, W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.attempted = true;
        self.output.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.output.flush()
    }
}
