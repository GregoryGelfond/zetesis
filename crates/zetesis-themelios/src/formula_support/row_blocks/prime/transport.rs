//! Packed occurrence coordinates for the row-block tape.
//!
//! Each staged occurrence is one packed integer (a `u32` holding two `u16`s,
//! or a `u64` holding two `u32`s), so reserve and push charge the same work as
//! the former `Occurrence` buffer. The values are positions into interned
//! snapshot columns, never `TermRef`s: a later intern cache and TermRef-ordered
//! emit can sit on the coordinator publish side without changing this tape.

use super::{CoordWidth, coord_width};
use crate::formula_support::{Buffer, Computation, Counters};
use crate::{FormulaFailure, FormulaLimits, ProgramSite};

/// Column-packed occurrence tape. The live buffer is one element per
/// occurrence, packed to the narrowest width the source indexes allow.
pub(in crate::formula_support::row_blocks) struct PackedCoords {
    data: Data,
}

enum Data {
    U16(Buffer<u32>),
    U32(Buffer<u64>),
}

impl PackedCoords {
    pub(in crate::formula_support::row_blocks) fn new(
        width: CoordWidth,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Self, FormulaFailure> {
        Ok(Self {
            data: match width {
                CoordWidth::U16 => Data::U16(Buffer::new(computation, limits, counters, location)?),
                CoordWidth::U32 => Data::U32(Buffer::new(computation, limits, counters, location)?),
            },
        })
    }

    pub(in crate::formula_support::row_blocks) fn width(&self) -> CoordWidth {
        match self.data {
            Data::U16(_) => CoordWidth::U16,
            Data::U32(_) => CoordWidth::U32,
        }
    }

    pub(in crate::formula_support::row_blocks) fn len(&self) -> usize {
        match &self.data {
            Data::U16(buffer) => buffer.len(),
            Data::U32(buffer) => buffer.len(),
        }
    }

    pub(in crate::formula_support::row_blocks) fn reserve(
        &mut self,
        additional: usize,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        match &mut self.data {
            Data::U16(buffer) => {
                buffer.reserve(additional, computation, limits, counters, location)
            }
            Data::U32(buffer) => {
                buffer.reserve(additional, computation, limits, counters, location)
            }
        }
    }

    pub(in crate::formula_support::row_blocks) fn push(
        &mut self,
        position: usize,
        source_index: usize,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        match &mut self.data {
            Data::U16(buffer) => {
                let position = u16::try_from(position).map_err(|_| overflow(location))?;
                let source_index = u16::try_from(source_index).map_err(|_| overflow(location))?;
                buffer.push(
                    u32::from(position) | u32::from(source_index) << 16,
                    computation,
                    limits,
                    counters,
                    location,
                )
            }
            Data::U32(buffer) => {
                let position = u32::try_from(position).map_err(|_| overflow(location))?;
                let source_index = u32::try_from(source_index).map_err(|_| overflow(location))?;
                buffer.push(
                    u64::from(position) | u64::from(source_index) << 32,
                    computation,
                    limits,
                    counters,
                    location,
                )
            }
        }
    }

    pub(in crate::formula_support::row_blocks) fn get(
        &self,
        index: usize,
    ) -> Option<(usize, usize)> {
        match &self.data {
            Data::U16(buffer) => {
                let packed = *buffer.slice().get(index)?;
                Some((
                    usize::from(u16::try_from(packed & 0xFFFF).expect("low 16 bits")),
                    usize::from(u16::try_from(packed >> 16).expect("high 16 bits")),
                ))
            }
            Data::U32(buffer) => {
                let packed = *buffer.slice().get(index)?;
                Some((
                    usize::try_from(u32::try_from(packed & 0xFFFF_FFFF).expect("low 32 bits"))
                        .expect("u32 fits usize"),
                    usize::try_from(u32::try_from(packed >> 32).expect("high 32 bits"))
                        .expect("u32 fits usize"),
                ))
            }
        }
    }

    /// Overwrite the source index so restore authentication can be tested.
    /// The packed width cannot hold `usize::MAX` on a 64-bit host; the
    /// all-ones index for this width is used instead.
    #[cfg(test)]
    pub(in crate::formula_support::row_blocks) fn poison_source_index(&mut self, index: usize) {
        match &mut self.data {
            Data::U16(buffer) => {
                let packed = &mut buffer.slice_mut()[index];
                *packed = (*packed & 0x0000_FFFF) | u32::from(u16::MAX) << 16;
            }
            Data::U32(buffer) => {
                let packed = &mut buffer.slice_mut()[index];
                *packed = (*packed & 0x0000_0000_FFFF_FFFF) | u64::from(u32::MAX) << 32;
            }
        }
    }

    pub(in crate::formula_support::row_blocks) fn embedded(
        self,
        location: ProgramSite,
    ) -> Result<Self, FormulaFailure> {
        Ok(Self {
            data: match self.data {
                Data::U16(buffer) => Data::U16(buffer.embedded(location)?),
                Data::U32(buffer) => Data::U32(buffer.embedded(location)?),
            },
        })
    }
}

/// Choose a packed width, or report overflow when an index does not fit `u32`.
pub(in crate::formula_support::row_blocks) fn width_for_index(
    max_index: usize,
    location: ProgramSite,
) -> Result<CoordWidth, FormulaFailure> {
    coord_width(max_index).ok_or_else(|| overflow(location))
}

fn overflow(location: ProgramSite) -> FormulaFailure {
    FormulaFailure::SupportRelation {
        error: zetesis_core::relation::Failure::Overflow,
        location,
    }
}
