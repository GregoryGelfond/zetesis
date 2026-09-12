//! Bounded per-rule timing storage, independent of source semantics.

use std::cell::{Cell, RefCell};
use std::time::Instant;

use serde::Serialize;
use themelios_base::span::Location;
use zetesis_themelios::{GroundingObserver, GroundingOutcome, GroundingPhase, GroundingWork};

use super::{Error, Mode};

/// Original source coordinates; repeated expanded rules may share a span.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct SourceSpan {
    /// Source identity in the retained source catalog.
    pub source: u32,
    /// Inclusive original byte offset.
    pub start_byte: u32,
    /// Exclusive original byte offset.
    pub end_byte: u32,
}

/// One sequential phase callback pair, including a failed attempt.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct PhaseRecord {
    /// Phase identity; repeated rule phases retain their original order.
    #[serde(serialize_with = "phase_label")]
    pub phase: GroundingPhase,
    /// Original rule location, absent for whole-program phases.
    pub location: Option<SourceSpan>,
    /// Returned, failed or unwound phase; never a model verdict.
    #[serde(serialize_with = "outcome_label")]
    pub outcome: GroundingOutcome,
    /// Host monotonic duration; `None` means nanosecond conversion overflowed.
    pub elapsed_ns: Option<u64>,
    /// Exact selected phase-local event counts; unavailable fields remain null.
    #[serde(serialize_with = "serialize_work")]
    pub work: GroundingWork,
}

#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde serialize_with callbacks borrow the serialized field."
)]
fn phase_label<S: serde::Serializer>(value: &GroundingPhase, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(value.label())
}

#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde serialize_with callbacks borrow the serialized field."
)]
fn outcome_label<S: serde::Serializer>(value: &GroundingOutcome, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(value.label())
}

fn serialize_work<S: serde::Serializer>(work: &GroundingWork, s: S) -> Result<S::Ok, S::Error> {
    use serde::ser::SerializeStruct;
    let mut fields = s.serialize_struct("GroundingWork", 24)?;
    fields.serialize_field("support_rounds", &work.support_rounds)?;
    fields.serialize_field("support_atoms", &work.support_atoms)?;
    fields.serialize_field("support_index_entries", &work.support_index_entries)?;
    fields.serialize_field("join_probes", &work.join_probes)?;
    fields.serialize_field("join_rows", &work.join_rows)?;
    fields.serialize_field("indexed_probes", &work.indexed_probes)?;
    fields.serialize_field("table_inapplicable_probes", &work.table_inapplicable_probes)?;
    fields.serialize_field("table_preparations", &work.table_preparations)?;
    fields.serialize_field("table_reuses", &work.table_reuses)?;
    fields.serialize_field("table_probes", &work.table_probes)?;
    fields.serialize_field("table_rows", &work.table_rows)?;
    fields.serialize_field("table_prepare_work", &work.table_prepare_work)?;
    fields.serialize_field("table_query_work", &work.table_query_work)?;
    fields.serialize_field("table_index_bytes", &work.table_index_bytes)?;
    fields.serialize_field("support_peak_bytes", &work.support_peak_bytes)?;
    fields.serialize_field("binding_snapshots", &work.binding_snapshots)?;
    fields.serialize_field("readiness_nodes", &work.readiness_nodes)?;
    fields.serialize_field("expression_evaluations", &work.expression_evaluations)?;
    fields.serialize_field("expression_nodes", &work.expression_nodes)?;
    fields.serialize_field("atom_lookups", &work.atom_lookups)?;
    fields.serialize_field("atoms_inserted", &work.atoms_inserted)?;
    fields.serialize_field("node_lookups", &work.node_lookups)?;
    fields.serialize_field("nodes_inserted", &work.nodes_inserted)?;
    fields.serialize_field("roots", &work.roots)?;
    fields.end()
}

/// Why a callback prefix cannot establish complete phase attribution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureRefusal {
    /// The bounded record store was full; no later record was retained.
    RecordLimit,
    /// Callback nesting or pairing did not satisfy the observer contract.
    Callbacks,
    /// A duration could not be represented as unsigned nanoseconds.
    ClockOverflow,
    /// At least one selected work counter was unavailable after overflow.
    WorkUnavailable,
}

pub(super) struct Observer {
    mode: Mode,
    ground_start: Cell<Option<Instant>>,
    elapsed: Cell<Option<u64>>,
    phase_start: Cell<Option<(GroundingPhase, Instant)>>,
    records: RefCell<Vec<PhaseRecord>>,
    limit: usize,
    refusal: Cell<Option<CaptureRefusal>>,
}

impl Observer {
    pub(super) fn new(mode: Mode, limit: usize) -> Result<Self, Error> {
        let mut records = Vec::new();
        if mode == Mode::Detailed {
            records
                .try_reserve_exact(limit)
                .map_err(|_| Error::Allocation)?;
        }
        Ok(Self {
            mode,
            ground_start: Cell::new(None),
            elapsed: Cell::new(None),
            phase_start: Cell::new(None),
            records: RefCell::new(records),
            limit,
            refusal: Cell::new(None),
        })
    }

    fn refuse(&self, refusal: CaptureRefusal) {
        if self.refusal.get().is_none() {
            self.refusal.set(Some(refusal));
        }
    }

    fn duration(&self, start: Instant) -> Option<u64> {
        let elapsed = u64::try_from(start.elapsed().as_nanos()).ok();
        if elapsed.is_none() {
            self.refuse(CaptureRefusal::ClockOverflow);
        }
        elapsed
    }

    pub(super) fn finish(self) -> (Option<u64>, Vec<PhaseRecord>, Option<CaptureRefusal>) {
        if self.ground_start.get().is_some() || self.phase_start.get().is_some() {
            self.refuse(CaptureRefusal::Callbacks);
        }
        (
            self.elapsed.get(),
            self.records.into_inner(),
            self.refusal.get(),
        )
    }
}

impl GroundingObserver for Observer {
    fn enter(&self) {
        if self.ground_start.replace(Some(Instant::now())).is_some() {
            self.refuse(CaptureRefusal::Callbacks);
        }
    }

    fn exit(&self) {
        if let Some(start) = self.ground_start.take() {
            self.elapsed.set(self.duration(start));
        } else {
            self.refuse(CaptureRefusal::Callbacks);
        }
    }

    fn details_enabled(&self) -> bool {
        self.mode == Mode::Detailed
    }

    fn phase_enter(&self, phase: GroundingPhase, _location: Option<Location>) {
        if self
            .phase_start
            .replace(Some((phase, Instant::now())))
            .is_some()
        {
            self.refuse(CaptureRefusal::Callbacks);
        }
    }

    fn phase_exit(
        &self,
        phase: GroundingPhase,
        location: Option<Location>,
        outcome: GroundingOutcome,
        work: GroundingWork,
    ) {
        let Some((entered, start)) = self.phase_start.take() else {
            self.refuse(CaptureRefusal::Callbacks);
            return;
        };
        if entered != phase {
            self.refuse(CaptureRefusal::Callbacks);
        }
        let elapsed_ns = self.duration(start);
        if [
            work.support_rounds,
            work.support_atoms,
            work.support_index_entries,
            work.join_probes,
            work.join_rows,
            work.indexed_probes,
            work.table_inapplicable_probes,
            work.table_preparations,
            work.table_reuses,
            work.table_probes,
            work.table_rows,
            work.table_prepare_work,
            work.table_query_work,
            work.table_index_bytes,
            work.support_peak_bytes,
            work.binding_snapshots,
            work.readiness_nodes,
            work.expression_evaluations,
            work.expression_nodes,
            work.atom_lookups,
            work.atoms_inserted,
            work.node_lookups,
            work.nodes_inserted,
            work.roots,
        ]
        .iter()
        .any(Option::is_none)
        {
            self.refuse(CaptureRefusal::WorkUnavailable);
        }
        let mut records = self.records.borrow_mut();
        if records.len() == self.limit {
            self.refuse(CaptureRefusal::RecordLimit);
            return;
        }
        records.push(PhaseRecord {
            phase,
            location: location.map(|location| SourceSpan {
                source: location.source.get(),
                start_byte: location.span.start().get(),
                end_byte: location.span.end().get(),
            }),
            outcome,
            elapsed_ns,
            work,
        });
    }
}
