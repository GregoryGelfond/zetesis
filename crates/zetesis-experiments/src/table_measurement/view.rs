use std::{io, mem::size_of};

use serde::{Serialize, Serializer, ser::SerializeStruct};
use zetesis_cpu::table;

use super::{Configuration, Subject};

/// Complete row/domain contract shared by the three routes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Output {
    /// Surviving row positions, preserving original occurrence order/aliasing.
    pub rows: Vec<usize>,
    /// Canonically ordered typed value IDs in each variable's projected domain.
    /// IDs decode through the subject's complete typed `values` list.
    pub domains: Vec<Vec<usize>>,
}
impl Output {
    pub(super) fn retained_bytes(&self) -> usize {
        self.rows.capacity() * size_of::<usize>()
            + self.domains.capacity() * size_of::<Vec<usize>>()
            + self
                .domains
                .iter()
                .map(|domain| domain.capacity() * size_of::<usize>())
                .sum::<usize>()
    }
}

/// Actual CPU procedure, not an inference from a worker request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Route {
    /// Complete typed row scan with prepared sorted domain memberships.
    Scan,
    /// Scalar repeated projection against one immutable support index.
    Table,
    /// Independent projections share that same index through an owned pool.
    Rayon,
}
/// Sample role; initial does not mean hardware caches were cold.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Phase {
    /// First retained batch for a route.
    Initial,
    /// Explicit warmup; remains in the raw report.
    Warmup,
    /// Timed repetition after initial/warmup batches.
    Timed,
}

/// One complete query or an explicit finite-table applicability refusal.
#[derive(Debug, Serialize)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
pub enum Outcome {
    /// Complete primitive result and conversion to the common output contract.
    Complete {
        /// Position in the original ordered domain-query sequence.
        query: usize,
        /// Primitive evaluation including its allocation and table domain resolution.
        projection_ns: u128,
        /// Conversion into common ordered row/value ID vectors.
        conversion_ns: u128,
        /// Scan comparator invocations only; absent for indexed-table routes.
        #[serde(skip_serializing_if = "Option::is_none")]
        scan_comparisons: Option<u64>,
        /// Table operation receipt; absent for scan, with different work units.
        #[serde(skip_serializing_if = "Option::is_none")]
        table: Option<Receipt>,
        /// Dynamic vector capacity of the common result, excluding its fixed
        /// envelope (counted once in the enclosing batch) and borrowed inputs.
        output_bytes: usize,
        /// Exact full output, validated outside all batch operation timers.
        output: Output,
    },
    /// A finite table work/capacity limit refused this query; no output is invented.
    Refused {
        /// Original query position.
        query: usize,
        /// Actual failed-operation interval, not successful projection time.
        attempt_ns: u128,
        /// Original typed failure with exact charged prefix and capacity peak.
        #[serde(serialize_with = "serialize_failure")]
        failure: table::Failure,
    },
}

/// Table work/capacity receipt. These numbers do not measure process RSS.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct Receipt {
    /// Table's charged logical operations and typed-comparison payload bytes.
    pub work: u64,
    /// Returned object capacity excluding its borrowed table/relation.
    pub retained_bytes: usize,
    /// Conservative live operation capacity including borrowed inputs and scratch.
    pub peak_bytes: usize,
}
impl From<table::Statistics> for Receipt {
    fn from(value: table::Statistics) -> Self {
        Self {
            work: value.work,
            retained_bytes: value.retained_bytes,
            peak_bytes: value.peak_bytes,
        }
    }
}

/// One-time construction observations, excluded from repeated projection times.
#[derive(Debug, Serialize)]
pub struct Preparation {
    /// Complete row occurrences admitted to the immutable relation.
    pub rows: usize,
    /// Rows satisfying all column aliases before domain narrowing.
    pub coherent_rows: usize,
    /// Packed row words per support entry, including the partial last word.
    pub row_words: usize,
    /// Actual prepared variable/value entries; absent after preparation refusal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub support_entries: Option<usize>,
    /// Bounded original rows, catalog mapping and query generation.
    pub fixture_ns: u128,
    /// Independent source-reference construction, never used as an execution index.
    pub reference_ns: u128,
    /// Sorted/deduplicated borrowed domain memberships for the scan.
    pub scan_domains_ns: u128,
    /// Shared column/dictionary construction.
    pub relation_ns: u128,
    /// Table preparation attempt, including a refused attempt when applicable.
    pub table_ns: u128,
    /// Independently owned Rayon pool creation.
    pub pool_ns: u128,
    /// Actual pool width; scan/table scalar routes use one caller thread.
    pub pool_workers: usize,
    /// Conservative fixed-generator fixture/view/scratch capacity envelope.
    pub fixture_bound_bytes: usize,
    /// Retained sorted borrowed domain vectors, excluding source payloads.
    pub scan_domain_bytes: usize,
    /// Retained relation-view capacity, excluding original source payloads.
    pub relation_bytes: usize,
    /// Relation construction work in its own documented units.
    pub relation_work: u128,
    /// Stored original-reference result vector capacities, including their fixed cells.
    pub reference_bytes: usize,
    /// Successful table preparation receipt, absent after refusal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub table: Option<Receipt>,
}

/// Streaming report, including prior successful work before a failure.
#[derive(Serialize)]
#[serde(tag = "event", rename_all = "kebab-case")]
pub enum Event<'a> {
    /// Configuration admitted before fixture construction.
    Start {
        /// Report schema version.
        schema: u32,
        /// Complete finite request.
        configuration: Configuration,
    },
    /// Complete typed subject with immutable row/query identity and setup costs.
    Subject {
        /// SHA-256 of compact serialized subject bytes.
        sha256: &'a str,
        /// Exact original data and ordered domains.
        subject: &'a Subject,
        /// One-time preparation and its capacity observations.
        preparation: &'a Preparation,
    },
    /// Table preparation failed its inclusive finite ceiling; scan may still run.
    PreparationRefused {
        /// Original typed finite-limit refusal.
        #[serde(serialize_with = "serialize_failure")]
        failure: &'a table::Failure,
    },
    /// One complete ordered batch; primitive/conversion times exclude verification.
    Batch {
        /// Immutable source/query identity.
        subject_sha256: &'a str,
        /// Actual executed procedure.
        route: Route,
        /// Initial, warmup or timed population.
        phase: Phase,
        /// Zero-based repetition in its own population.
        repetition: usize,
        /// Batch wall interval including scheduling and common output allocation.
        batch_ns: u128,
        /// Simultaneously retained common result capacity, excluding all inputs.
        output_bytes: usize,
        /// Per-query receipts and exact results in original order.
        outcomes: &'a [Outcome],
    },
    /// Every scheduled executable batch was checked and published.
    Complete {
        /// Complete original query count per batch.
        queries: usize,
        /// Executed batches; skipped table routes are reflected by passed=false.
        batches: usize,
        /// Named procedures skipped because no prepared table was admitted.
        skipped_routes: &'a [Route],
        /// True exactly when no preparation or query applicability refusal occurred.
        passed: bool,
    },
}

fn serialize_failure<S: Serializer>(
    value: &table::Failure,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let mut state = serializer.serialize_struct("TableFailure", 3)?;
    let cause = match &value.cause {
        table::Cause::Scope => FailureCause::Scope,
        table::Cause::Domains => FailureCause::Domains,
        table::Cause::Relation => FailureCause::Relation,
        table::Cause::Overflow => FailureCause::Overflow,
        table::Cause::Allocation => FailureCause::Allocation,
        table::Cause::Interrupted(reason) => FailureCause::Interrupted {
            reason: reason.to_string(),
        },
        table::Cause::Limit {
            resource,
            observed,
            limit,
        } => FailureCause::Limit {
            resource: match resource {
                table::Resource::Entries => "entries",
                table::Resource::Bytes => "bytes",
                table::Resource::Work => "work",
            },
            observed: *observed,
            limit: *limit,
        },
    };
    state.serialize_field("cause", &cause)?;
    state.serialize_field("work", &value.work)?;
    state.serialize_field("peak_bytes", &value.peak_bytes)?;
    state.end()
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum FailureCause {
    Scope,
    Domains,
    Relation,
    Overflow,
    Allocation,
    Interrupted {
        reason: String,
    },
    Limit {
        resource: &'static str,
        observed: u128,
        limit: u128,
    },
}

pub(super) struct BoundedWriter<W> {
    inner: W,
    remaining: usize,
}
impl<W> BoundedWriter<W> {
    pub fn new(inner: W, remaining: usize) -> Self {
        Self { inner, remaining }
    }
}
impl<W: io::Write> io::Write for BoundedWriter<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.remaining {
            return Err(io::Error::other("finite-table report byte ceiling"));
        }
        let count = self.inner.write(bytes)?;
        self.remaining -= count;
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
