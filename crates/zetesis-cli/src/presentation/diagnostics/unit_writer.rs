//! The diagnostics writer: whole write units, and slices passed through.

use std::{
    fmt,
    io::{self, Write},
};

/// The bytes a unit's buffer holds before it first grows, sized for the
/// execution-detail records of an ordinary run. A longer unit, such as the
/// statistics report or a record carrying an error's text, grows as a `Vec`
/// does, at least doubling.
const UNIT_CAPACITY: usize = 256;

/// The diagnostics writer. It admits whole write units, and slices passed
/// through unchanged, and nothing else.
///
/// Each `write_fmt` call and each [`Self::write_whole`] call reaches the
/// underlying writer as one `write_all` of the unit's bytes, or, if the unit's
/// rendering fails, not at all. `write` and `write_all` hand their slice to the
/// writer unchanged, and `flush` delegates; `Write`'s provided methods reach the
/// writer only through these. Nothing is retained between calls.
pub(super) struct UnitWriter<W> {
    writer: W,
}

impl<W: Write> UnitWriter<W> {
    pub(super) const fn new(writer: W) -> Self {
        Self { writer }
    }

    /// Render one write unit whole, then hand it to the writer in one
    /// `write_all` call. If the rendering fails, nothing of the unit is written
    /// and its error is returned.
    ///
    /// The unit's buffer belongs to this call: it is created here with
    /// [`UNIT_CAPACITY`] bytes and dropped on return. So no unit's bytes reach
    /// the writer after its call has returned, and no capacity outlives the
    /// unit.
    pub(super) fn write_whole(
        &mut self,
        render: impl FnOnce(&mut UnitBuffer) -> io::Result<()>,
    ) -> io::Result<()> {
        let mut unit = UnitBuffer {
            bytes: Vec::with_capacity(UNIT_CAPACITY),
        };
        render(&mut unit)?;
        self.writer.write_all(&unit.bytes)
    }
}

impl<W: Write> Write for UnitWriter<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.writer.write(bytes)
    }
    fn write_all(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.writer.write_all(bytes)
    }
    fn write_fmt(&mut self, args: fmt::Arguments<'_>) -> io::Result<()> {
        self.write_whole(|unit| unit.write_fmt(args))
    }
    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}

/// One write unit's bytes while it is rendered.
///
/// `write` and `write_all` append. Its `write_fmt` renders through a sink that
/// cannot fail, so a formatting error is a `Display` implementation's own; it
/// becomes the presentation's formatting failure, an `Other` error, where the
/// standard library would panic.
pub(crate) struct UnitBuffer {
    bytes: Vec<u8>,
}

impl Write for UnitBuffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn write_all(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
    fn write_fmt(&mut self, args: fmt::Arguments<'_>) -> io::Result<()> {
        fmt::write(&mut Append(&mut self.bytes), args)
            .map_err(|fmt::Error| super::super::formatting_failure())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Appends rendered text to a unit's bytes; it never fails.
struct Append<'a>(&'a mut Vec<u8>);

impl fmt::Write for Append<'_> {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        self.0.extend_from_slice(text.as_bytes());
        Ok(())
    }
}
