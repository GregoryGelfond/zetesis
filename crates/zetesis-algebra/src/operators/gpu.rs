//! GPU realization of the same five relational operators.
//!
//! This module records the device-side contract so a later `zetesis-wgpu`
//! backend can implement it without inventing a second semantics. The CPU
//! columnar kernels ship first; this is the design, not a shipped device
//! path. Work and cancellation stay on the host accounts already used by
//! the CPU operators. Device buffers hold the same columnar `i32` cells as
//! [`super::Atoms`] and [`super::Bindings`]: little-endian, column major,
//! 4-byte aligned, no inter-cell padding.
//!
//! Each operator has a straightforward kernel shape:
//!
//! - **Bind:** a range fill or a generating walk (multiples, divisors, sums,
//!   products, absolute values) writes a destination column. Independent
//!   domains may be partitioned across workgroups.
//! - **Filter:** a predicate mask over columnar slices, then a compact.
//! - **Project:** a head extraction, then a unique sort of the key columns.
//! - **Union:** a merge of two sorted key buffers, or a sort-unique of the
//!   concatenation when that is simpler on the device.
//! - **Least:** the fixpoint loop stays on the host; each round uploads
//!   `X`, dispatches the transform (or a union of per-rule transforms),
//!   and downloads `Y` for the host-side union and equality test.
//!
//! Host code charges the existing meter before dispatch and polls
//! [`crate::Cancellation`] between submissions, the same way today's
//! relation filter in `zetesis-wgpu` already does. No SAT or CDNL path is
//! introduced, and the CPU backend remains for machines without a GPU, for
//! fallback, and for cross-checking.

/// Physical layout a device backend should use for uploaded columns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColumnLayout {
    width: u32,
    rows: u32,
    column_stride: u32,
}

impl ColumnLayout {
    /// Packed `i32` columns with no trailing padding.
    ///
    /// # Errors
    /// Returns `None` if `width` or `rows` does not fit in `u32`, or if the
    /// byte stride would overflow `u32`.
    #[must_use]
    pub fn packed(width: usize, rows: usize) -> Option<Self> {
        let width = u32::try_from(width).ok()?;
        let rows = u32::try_from(rows).ok()?;
        let column_stride = rows.checked_mul(4)?;
        Some(Self {
            width,
            rows,
            column_stride,
        })
    }

    /// Attribute count.
    #[must_use]
    pub fn width(self) -> u32 {
        self.width
    }

    /// Row count.
    #[must_use]
    pub fn rows(self) -> u32 {
        self.rows
    }

    /// Bytes in one packed column (`4 * rows` for this constructor).
    #[must_use]
    pub fn column_stride(self) -> u32 {
        self.column_stride
    }
}
