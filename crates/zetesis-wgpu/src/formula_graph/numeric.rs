//! Shared transport rows retain the admitted Theory's numeric row identities.

use super::{address, capacity};
use crate::{GpuError, GpuErrorKind};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{ExtremumCutRule, NodeView, Theory};

/// Sorted source row IDs and their column offsets. This host-only directory is
/// prepared once, charged as cold staging, and released after packing.
pub(crate) struct Rows {
    offsets: Vec<(usize, u32)>,
    pub(crate) tail_words: u32,
}
impl Rows {
    pub(super) fn new(
        theory: &Theory,
        start: u32,
        count: usize,
        max_staging_bytes: u64,
        cancellation: &Cancellation,
    ) -> Result<Self, GpuError> {
        let mut offsets = Vec::new();
        directory_budget(count, max_staging_bytes)?;
        cancellation.poll().map_err(GpuError::interrupted)?;
        offsets
            .try_reserve_exact(count)
            .map_err(|error| GpuError::new(GpuErrorKind::Allocation, error.to_string()))?;
        directory_budget(offsets.capacity(), max_staging_bytes)?;
        for index in 0..theory.nodes().len() {
            cancellation.poll().map_err(GpuError::interrupted)?;
            if let Some(row) = row(theory, index)
                && constant(
                    theory.view().node(index).expect("admitted node"),
                    cancellation,
                )?
                .is_none()
            {
                offsets.push((row, 0));
            }
        }
        // In-place sorting allocates no auxiliary storage. Like a single host
        // allocation, this bounded library call is polled on either side.
        cancellation.poll().map_err(GpuError::interrupted)?;
        offsets.sort_unstable_by_key(|&(row, _)| row);
        cancellation.poll().map_err(GpuError::interrupted)?;
        offsets.dedup_by_key(|entry| entry.0);
        let mut tail_words = start;
        for (row, offset) in &mut offsets {
            cancellation.poll().map_err(GpuError::interrupted)?;
            *offset = tail_words;
            let length = address(theory.parts().rows()[*row].length)?;
            tail_words = length
                .checked_mul(3)
                .and_then(|n| tail_words.checked_add(n))
                .ok_or_else(|| capacity("numeric column offsets exceed u32"))?;
        }
        Ok(Self {
            offsets,
            tail_words,
        })
    }
    pub(crate) fn release(&mut self) {
        self.offsets = Vec::new();
    }
    pub(crate) fn bytes(&self) -> Result<u64, GpuError> {
        u64::try_from(self.offsets.capacity())
            .ok()
            .and_then(|n| n.checked_mul(size_of::<(usize, u32)>() as u64))
            .ok_or_else(|| capacity("numeric directory size overflows"))
    }
    fn offset(&self, row: usize) -> Result<u32, GpuError> {
        self.offsets
            .binary_search_by_key(&row, |&(row, _)| row)
            .map(|index| self.offsets[index].1)
            .map_err(|_| capacity("numeric row is absent from the transport directory"))
    }
}
fn directory_budget(entries: usize, maximum: u64) -> Result<(), GpuError> {
    let bytes = u64::try_from(entries)
        .ok()
        .and_then(|n| n.checked_mul(size_of::<(usize, u32)>() as u64))
        .ok_or_else(|| capacity("numeric directory size overflows"))?;
    if bytes > maximum {
        return Err(capacity("numeric directory exceeds batch byte allowance"));
    }
    Ok(())
}

/// Physical constants preserve semantic node IDs without copying unused rows.
/// Admitted nonnegative sums fit i128; extrema use their carrier's endpoints.
pub(super) fn constant(
    node: NodeView<'_>,
    cancellation: &Cancellation,
) -> Result<Option<bool>, GpuError> {
    match node {
        NodeView::Threshold(threshold) => {
            if threshold.bound <= 0 {
                return Ok(Some(true));
            }
            let mut total = 0i128;
            for &weight in threshold.weights {
                cancellation.poll().map_err(GpuError::interrupted)?;
                total += i128::from(weight);
            }
            Ok((threshold.bound > total).then_some(false))
        }
        NodeView::ExtremumCut(cut) => {
            if let Some(value) = cut.endpoint_truth() {
                return Ok(Some(value));
            }
            for &rank in cut.row.ranks {
                cancellation.poll().map_err(GpuError::interrupted)?;
                if cut.qualifies(rank) {
                    return Ok(None);
                }
            }
            Ok(Some(false))
        }
        _ => Ok(None),
    }
}
fn row(theory: &Theory, index: usize) -> Option<usize> {
    let node = theory.nodes()[index];
    node.threshold_descriptor()
        .map(|descriptor| theory.parts().thresholds()[descriptor].row)
        .or_else(|| {
            node.extremum_cut_descriptor()
                .map(|descriptor| theory.parts().extremum_cuts()[descriptor].row)
        })
}
fn limbs(value: u128) -> [u32; 4] {
    let bytes = value.to_le_bytes();
    std::array::from_fn(|index| {
        u32::from_le_bytes(
            bytes[index * 4..index * 4 + 4]
                .try_into()
                .expect("four bytes"),
        )
    })
}
pub(super) fn append(
    theory: &Theory,
    rows: &Rows,
    words: &mut Vec<u32>,
    cancellation: &Cancellation,
) -> Result<(), GpuError> {
    for index in 0..theory.view().len() {
        cancellation.poll().map_err(GpuError::interrupted)?;
        match theory.view().node(index).expect("admitted node") {
            NodeView::And(children) | NodeView::Or(children) if children.len() > 2 => {
                for &child in children {
                    cancellation.poll().map_err(GpuError::interrupted)?;
                    super::append(words, &[address(child)?])?;
                }
            }
            NodeView::Threshold(threshold) if words[index * 4] == 7 => {
                let offset = rows.offset(row(theory, index).expect("threshold row"))?;
                // Constant bounds already have a constant physical header.
                let bound = u128::try_from(threshold.bound).expect("positive variable threshold");
                super::append(words, &[offset])?;
                super::append(words, &limbs(bound))?;
            }
            NodeView::ExtremumCut(cut) if matches!(words[index * 4], 8 | 9) => {
                let offset = rows.offset(row(theory, index).expect("ordinal row"))?;
                let (ExtremumCutRule::Prefix(bound) | ExtremumCutRule::Suffix(bound)) = *cut.rule;
                let bound = limbs(u128::from(bound));
                let count = limbs(u128::from(cut.row.rank_count));
                super::append(words, &[offset, bound[0], bound[1], count[0], count[1]])?;
            }
            _ => {}
        }
    }
    for &(row, _) in &rows.offsets {
        let row = theory.parts().rows()[row];
        let span = row.start..row.start + row.length;
        for &condition in &theory.parts().conditions()[span.clone()] {
            cancellation.poll().map_err(GpuError::interrupted)?;
            super::append(words, &[address(condition)?])?;
        }
        for limb in 0..2 {
            for &value in &theory.parts().numeric_values()[span.clone()] {
                cancellation.poll().map_err(GpuError::interrupted)?;
                super::append(words, &[limbs(u128::from(value))[limb]])?;
            }
        }
    }
    Ok(())
}
