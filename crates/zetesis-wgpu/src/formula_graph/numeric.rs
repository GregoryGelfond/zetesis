//! Shared transport rows retain the admitted Theory's numeric row identities.

use super::{address, capacity};
use crate::{GpuError, GpuErrorKind};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{ExtremumCutRule, NodeView, Theory};

mod summary;
use summary::{Summary, constant, endpoint};

struct Row {
    source: usize,
    offset: u32,
    summary: Summary,
    guards: std::ops::Range<usize>,
}

/// A cold, bounded directory of source rows and their live guard IDs. Summaries
/// are computed once per source row, then released with the packing directory.
/// No candidate truth or second numeric semantic owner is retained here.
pub(crate) struct Rows {
    entries: Vec<Row>,
    guards: Vec<(usize, u32)>,
    constant_only: bool,
    pub(crate) tail_words: u32,
}
impl Rows {
    pub(super) fn new(
        theory: &Theory,
        max_staging_bytes: u64,
        cancellation: &Cancellation,
    ) -> Result<Self, GpuError> {
        let mut count = 0;
        for index in 0..theory.view().len() {
            poll(cancellation)?;
            if row(theory, index).is_some()
                && endpoint(theory.view().node(index).expect("admitted node")).is_none()
            {
                count += 1;
            }
        }
        if count == 0 {
            poll(cancellation)?;
            return Ok(Self {
                entries: Vec::new(),
                guards: Vec::new(),
                constant_only: true,
                tail_words: 0,
            });
        }
        if directory_bytes(0, count)? > max_staging_bytes {
            return Self::constant_only(theory, cancellation);
        }
        let mut guards = Vec::new();
        guards
            .try_reserve_exact(count)
            .map_err(|error| allocation(&error))?;
        if directory_bytes(0, guards.capacity())? > max_staging_bytes {
            drop(guards);
            return Self::constant_only(theory, cancellation);
        }
        for index in 0..theory.view().len() {
            poll(cancellation)?;
            if let Some(row) = row(theory, index)
                && endpoint(theory.view().node(index).expect("admitted node")).is_none()
            {
                guards.push((row, address(index)?));
            }
        }
        // Bounded in-place sorting allocates no auxiliary storage. As with a
        // host allocation, cancellation is polled on either side of the call.
        poll(cancellation)?;
        guards.sort_unstable();
        poll(cancellation)?;
        let mut unique = 0;
        let mut previous = None;
        for &(source, _) in &guards {
            poll(cancellation)?;
            if previous != Some(source) {
                unique += 1;
                previous = Some(source);
            }
        }
        if directory_bytes(unique, guards.capacity())? > max_staging_bytes {
            drop(guards);
            return Self::constant_only(theory, cancellation);
        }
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(unique)
            .map_err(|error| allocation(&error))?;
        if directory_bytes(entries.capacity(), guards.capacity())? > max_staging_bytes {
            drop(entries);
            drop(guards);
            return Self::constant_only(theory, cancellation);
        }
        let mut result = Self {
            entries,
            guards,
            constant_only: false,
            tail_words: 0,
        };
        result.summarize(theory, cancellation)?;
        Ok(result)
    }

    fn summarize(&mut self, theory: &Theory, cancellation: &Cancellation) -> Result<(), GpuError> {
        let mut read = 0;
        let mut write = 0;
        while read < self.guards.len() {
            poll(cancellation)?;
            let (source, first) = self.guards[read];
            let summary = Summary::new(
                theory.view().node(first as usize).expect("numeric node"),
                cancellation,
            )?;
            let start = write;
            while read < self.guards.len() && self.guards[read].0 == source {
                poll(cancellation)?;
                let guard = self.guards[read];
                let node = theory.view().node(guard.1 as usize).expect("numeric node");
                if constant(node, Some(&summary), cancellation)?.is_none() {
                    self.guards[write] = guard;
                    write += 1;
                }
                read += 1;
            }
            self.entries.push(Row {
                source,
                offset: 0,
                summary,
                guards: start..write,
            });
        }
        self.guards.truncate(write);
        if self.guards.is_empty() {
            // A larger staging allowance must not make an all-constant graph
            // retain a directory that its allocation-free path does not need.
            self.release();
            self.constant_only = true;
        }
        Ok(())
    }

    /// Preserve the allocation-free constant-only boundary under a tight staging
    /// allowance. This bounded compatibility scan never admits a live numeric row.
    fn constant_only(theory: &Theory, cancellation: &Cancellation) -> Result<Self, GpuError> {
        for index in 0..theory.view().len() {
            poll(cancellation)?;
            if row(theory, index).is_some()
                && constant(
                    theory.view().node(index).expect("admitted node"),
                    None,
                    cancellation,
                )?
                .is_none()
            {
                return Err(capacity("numeric directory exceeds batch byte allowance"));
            }
        }
        Ok(Self {
            entries: Vec::new(),
            guards: Vec::new(),
            constant_only: true,
            tail_words: 0,
        })
    }

    pub(super) fn constant(
        &self,
        theory: &Theory,
        index: usize,
        cancellation: &Cancellation,
    ) -> Result<Option<bool>, GpuError> {
        let node = theory.view().node(index).expect("admitted node");
        if row(theory, index).is_none() || endpoint(node).is_some() {
            return Ok(endpoint(node));
        }
        let summary = if self.constant_only {
            None
        } else {
            let index = self.entry(row(theory, index).expect("numeric node"))?;
            Some(&self.entries[index].summary)
        };
        constant(node, summary, cancellation)
    }

    pub(super) fn locate(
        &mut self,
        theory: &Theory,
        start: u32,
        cancellation: &Cancellation,
    ) -> Result<(), GpuError> {
        self.tail_words = start;
        for entry in &mut self.entries {
            poll(cancellation)?;
            if entry.guards.is_empty() {
                continue;
            }
            entry.offset = self.tail_words;
            let length = address(theory.parts().rows()[entry.source].length)?;
            self.tail_words = length
                .checked_mul(1 + entry.summary.value_words())
                .and_then(|n| n.checked_add(3))
                .and_then(|n| n.checked_add(u32::try_from(entry.guards.len()).ok()?))
                .and_then(|n| self.tail_words.checked_add(n))
                .ok_or_else(|| capacity("numeric column offsets exceed u32"))?;
        }
        Ok(())
    }

    pub(crate) fn release(&mut self) {
        self.entries = Vec::new();
        self.guards = Vec::new();
    }
    pub(crate) fn bytes(&self) -> Result<u64, GpuError> {
        directory_bytes(self.entries.capacity(), self.guards.capacity())
    }
    fn entry(&self, source: usize) -> Result<usize, GpuError> {
        self.entries
            .binary_search_by_key(&source, |entry| entry.source)
            .map_err(|_| capacity("numeric row is absent from the transport directory"))
    }
    fn offset(&self, row: usize) -> Result<u32, GpuError> {
        Ok(self.entries[self.entry(row)?].offset)
    }
}
fn directory_bytes(rows: usize, guards: usize) -> Result<u64, GpuError> {
    rows.checked_mul(size_of::<Row>())
        .and_then(|n| n.checked_add(guards.checked_mul(size_of::<(usize, u32)>())?))
        .and_then(|n| u64::try_from(n).ok())
        .ok_or_else(|| capacity("numeric directory size overflows"))
}
fn poll(cancellation: &Cancellation) -> Result<(), GpuError> {
    cancellation.poll().map_err(GpuError::interrupted)
}
fn allocation(error: &std::collections::TryReserveError) -> GpuError {
    GpuError::new(GpuErrorKind::Allocation, error.to_string())
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
    for entry in &rows.entries {
        poll(cancellation)?;
        if entry.guards.is_empty() {
            continue;
        }
        let guards = &rows.guards[entry.guards.clone()];
        super::append(
            words,
            &[guards[0].1, address(guards.len())?, entry.summary.width()],
        )?;
        let row = theory.parts().rows()[entry.source];
        let span = row.start..row.start + row.length;
        for &condition in &theory.parts().conditions()[span.clone()] {
            cancellation.poll().map_err(GpuError::interrupted)?;
            super::append(words, &[address(condition)?])?;
        }
        for limb in 0..entry.summary.value_words() as usize {
            for &value in &theory.parts().numeric_values()[span.clone()] {
                cancellation.poll().map_err(GpuError::interrupted)?;
                super::append(words, &[limbs(u128::from(value))[limb]])?;
            }
        }
        for &(_, guard) in guards {
            poll(cancellation)?;
            super::append(words, &[guard])?;
        }
    }
    Ok(())
}
