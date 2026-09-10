//! Bounded equality filtering over a borrowed immutable relation snapshot.

mod device;
mod packing;

use std::{fmt, mem::size_of, time::Duration};
use zetesis_core::relation::{self, Relation, Selection};
use zetesis_cpu::{Control, Stop};

use crate::{GpuError, GpuErrorKind};
pub use device::{GpuRelationExecutor, PreparedGpuRelation};

const SHADER: &str = include_str!("filter.wgsl");
const ROWS_PER_GROUP: u32 = 64;
const BITS_PER_WORD: u32 = 32;
const RECEIPT_WORDS: u32 = 4;
const RECEIPT_MARKER: u32 = 0x434f_4c31;
const PARAM_BYTES: u64 = 32;

/// Inclusive bounds for a prepared view and one equality-filter invocation.
#[derive(Clone, Copy, Debug)]
pub struct RelationGpuLimits {
    /// Queries in one batch, preserving repeated query occurrences.
    pub max_queries: usize,
    /// Authored resident columns, transport, host packing and returned masks.
    /// Borrowed source atoms, relation storage and caller query allocations are
    /// excluded and must be charged by the enclosing operation. Driver-private
    /// storage, allocator metadata and delayed retirement are also excluded.
    pub max_bytes: u64,
    /// Scheduled row flags, equality comparisons, bit-fold steps and receipts.
    /// For R rows, Q queries, T=ceil(R/64), W=ceil(R/32), and E total query
    /// equalities, the nonempty schedule charges Q*(64*T+32*W+1)+R*E.
    pub max_work: u64,
    /// Bounded wait for submitted work; control is polled between short waits.
    /// Device creation, shader compilation and driver allocations are outside it.
    pub timeout: Duration,
}

impl Default for RelationGpuLimits {
    fn default() -> Self {
        Self {
            max_queries: 256,
            max_bytes: 128 * 1024 * 1024,
            max_work: 100_000_000,
            timeout: Duration::from_secs(30),
        }
    }
}

/// Attempt evidence, reset before each new filtering invocation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RelationGpuActivity {
    /// Actual compute queue submissions.
    pub submissions: u64,
    /// Query occurrences included in submitted commands.
    pub submitted_queries: u64,
    /// Physical workgroups, separate from logical query occurrences.
    pub submitted_workgroups: u64,
    /// Full-scan primitive work scheduled by those submissions.
    pub scheduled_work: u64,
    /// Query occurrences with fully validated mask receipts.
    pub completed_queries: u64,
    /// Work belonging to those validated complete records.
    pub completed_work: u64,
    /// Authored upload payload, not measured bus traffic.
    pub uploaded_bytes: u64,
    /// Readback payload whose complete records were validated.
    pub downloaded_bytes: u64,
}

/// Successful filter dimensions and authored capacity, independent of RSS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RelationGpuStats {
    /// Original row occurrences in the prepared relation.
    pub rows: u64,
    /// Input query occurrences, including repetitions.
    pub queries: u64,
    /// Resident uploaded column payload, including empty-buffer padding.
    pub column_bytes: u64,
    /// Uniform, query, equality, result and readback payload.
    pub transport_bytes: u64,
    /// Conservative authored payload checked before host/device allocation.
    pub accounted_bytes: u64,
    /// Exact physical dispatch dimensions; zero dimensions denote no dispatch.
    pub workgroups: [u32; 3],
}

/// Ordered equality masks tied to their original immutable relation owner.
///
/// Mask position is the input query position; bit position is the original row
/// occurrence, not its external catalog index. A mask is an equality filter,
/// not a complete pattern match or an answer-set membership certificate.
pub struct RelationGpuMasks<'owner, 'source> {
    relation: &'owner Relation<'source>,
    queries: usize,
    words_per_query: usize,
    words: Vec<u32>,
}

impl<'owner, 'source> RelationGpuMasks<'owner, 'source> {
    /// Immutable owner supplying dictionary and original catalog mapping.
    #[must_use]
    pub const fn relation(&self) -> &'owner Relation<'source> {
        self.relation
    }

    /// Complete query occurrences in original input order.
    #[must_use]
    pub const fn query_count(&self) -> usize {
        self.queries
    }

    /// Packed equality selection for one query; unused tail bits are zero.
    #[must_use]
    pub fn words(&self, query: usize) -> Option<&[u32]> {
        if query >= self.queries {
            return None;
        }
        let start = query * self.words_per_query;
        Some(&self.words[start..start + self.words_per_query])
    }

    /// Reconstruct one checked ordered row selection without retaining all
    /// query selections simultaneously. The caller still performs the complete
    /// term-pattern match. The limit includes masks, relation and both temporary
    /// and returned row-position capacity; other live selections remain caller
    /// owned and must be charged separately. Work charges two scans of the mask
    /// words and three operations per selected row (decode, validate and copy).
    /// The returned Selection's work counter covers only its validation/copy.
    ///
    /// # Errors
    /// Refuses an absent query, exceeded limits or allocation failure. Structural
    /// row validation alone does not establish that a GPU mask is complete; that
    /// correspondence is the shader/decoder contract and device qualification.
    pub fn selection(
        &self,
        query: usize,
        limits: relation::Limits,
    ) -> Result<Selection<'owner, 'source>, RelationGpuError> {
        let words = self
            .words(query)
            .ok_or_else(|| capacity("relation query is out of range"))?;
        let scans = 2 * words.len() as u128;
        if scans > u128::from(limits.max_work) {
            return Err(capacity("relation mask scan exceeds work ceiling").into());
        }
        let count = words
            .iter()
            .map(|word| word.count_ones() as usize)
            .sum::<usize>();
        let total_work = scans + 3 * count as u128;
        if total_work > u128::from(limits.max_work) {
            return Err(capacity("relation mask reconstruction exceeds work ceiling").into());
        }
        let mask_bytes = size_of::<Self>() + self.words.capacity() * size_of::<u32>();
        let temporary_bytes =
            size_of::<Vec<usize>>() as u128 + count as u128 * size_of::<usize>() as u128;
        let retained = self.relation.storage().retained_bytes;
        let required = retained as u128
            + mask_bytes as u128
            + temporary_bytes
            + size_of::<Selection<'_, '_>>() as u128
            + count as u128 * size_of::<usize>() as u128;
        if required > limits.max_bytes as u128 {
            return Err(capacity("relation mask reconstruction exceeds byte ceiling").into());
        }
        let temporary_bytes = usize::try_from(temporary_bytes)
            .map_err(|_| capacity("relation reconstruction exceeds host addressing"))?;
        let mut rows = packing::vector(count)?;
        for (word_index, &word) in words.iter().enumerate() {
            let mut remaining = word;
            while remaining != 0 {
                rows.push(
                    word_index * BITS_PER_WORD as usize + remaining.trailing_zeros() as usize,
                );
                remaining &= remaining - 1;
            }
        }
        let available = limits
            .max_bytes
            .checked_sub(mask_bytes)
            .and_then(|bytes| bytes.checked_sub(temporary_bytes))
            .ok_or_else(|| capacity("relation masks exceed selection byte ceiling"))?;
        let used_work = u64::try_from(scans + count as u128)
            .map_err(|_| capacity("relation reconstruction work exceeds u64"))?;
        self.relation
            .selection(
                &rows,
                relation::Limits {
                    max_bytes: available,
                    max_work: limits.max_work - used_work,
                    ..limits
                },
            )
            .map_err(RelationGpuError::Relation)
    }
}

/// An incomplete filtering operation; never an empty-result substitution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RelationGpuError {
    /// Shared cancellation or deadline was observed.
    Stopped(Stop),
    /// Relation ownership, checked selection or resource failure.
    Relation(relation::Failure),
    /// Adapter, capacity, execution or readback failure.
    Gpu(GpuError),
}

impl fmt::Display for RelationGpuError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stopped(stop) => stop.fmt(f),
            Self::Relation(error) => error.fmt(f),
            Self::Gpu(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for RelationGpuError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Stopped(stop) => stop,
            Self::Relation(error) => error,
            Self::Gpu(error) => error,
        })
    }
}
impl From<GpuError> for RelationGpuError {
    fn from(error: GpuError) -> Self {
        match error.interruption {
            Some(stop) => Self::Stopped(stop),
            None => Self::Gpu(error),
        }
    }
}

fn capacity(detail: &str) -> GpuError {
    GpuError::new(GpuErrorKind::Capacity, detail)
}
fn poll(control: &Control) -> Result<(), GpuError> {
    control.poll().map_err(|stop| GpuError {
        kind: GpuErrorKind::Device,
        detail: "relation equality filtering interrupted".to_owned(),
        interruption: Some(stop),
    })
}

#[cfg(test)]
#[path = "../../tests/relation/module.rs"]
mod tests;
