//! One immutable base and a flat local segment directory, without lookup indexes.

use std::sync::Arc;

use super::{RowSegment, budget, locate};

#[derive(Debug)]
pub(super) struct RowBase {
    pub(super) segments: Vec<Arc<RowSegment>>,
    pub(super) atoms: usize,
    pub(super) bytes: u128,
}

#[derive(Clone, Copy)]
pub(super) struct RowView<'a> {
    pub(super) base: Option<&'a RowBase>,
    pub(super) segments: &'a Vec<Arc<RowSegment>>,
}

impl<'a> RowView<'a> {
    pub(super) fn segment(self, id: usize) -> Option<&'a RowSegment> {
        let segments = self
            .base
            .filter(|base| id < base.atoms)
            .map_or(self.segments.as_slice(), |base| base.segments.as_slice());
        locate(segments, id, |segment| segment.start)
    }

    /// Directory and payload capacities. Base bytes were measured at closure;
    /// a single checked metadata read reuses that immutable receipt.
    pub(super) fn bytes_with<E>(
        self,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<u128, E> {
        let mut bytes = budget::capacity(self.segments);
        if let Some(base) = self.base {
            before()?;
            bytes += base.bytes;
        }
        for segment in self.segments {
            bytes += segment.bytes_with(&mut before)?;
        }
        Ok(bytes)
    }

    pub(super) fn bytes(self) -> u128 {
        self.bytes_with(|| Ok::<_, std::convert::Infallible>(()))
            .unwrap_or_else(|never| match never {})
    }
}
