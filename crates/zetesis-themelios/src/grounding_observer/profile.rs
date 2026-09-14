//! Fixed-space work attribution without clocks or semantic control.

use std::cell::RefCell;
use std::rc::Rc;

use themelios_base::span::Location;

use super::GroundingObserver;

/// A coarse operation inside eager formula materialization.
///
/// These phases exclude parsing and prepared-IR/observation compilation. Their
/// callbacks are sequential; rule phases repeat in the existing prepared order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum GroundingPhase {
    /// Complete the finite possible-support relation, including its indices.
    SupportCompletion,
    /// Attempt optional domains over the exact normalized positive source.
    DomainAnalysis,
    /// Determine active objective templates and construct their program.
    ObjectiveActivation,
    /// Initialize formula storage and the false/true nodes.
    FormulaInitialization,
    /// Join one prepared rule, evaluate filters and emit its formulas together.
    RuleInstantiation,
    /// Add constraints for present opposite classical signs.
    Coherence,
    /// Add necessary-support guards after every producer has been collected.
    SupportGuards,
    /// Validate and construct the final immutable formula theory.
    TheoryValidation,
}

impl GroundingPhase {
    /// Every currently defined phase, in materialization order.
    ///
    /// This supports fixed-size caller aggregation without enum discriminants
    /// or retention of the individual per-rule callbacks.
    pub const ALL: [Self; 8] = [
        Self::SupportCompletion,
        Self::DomainAnalysis,
        Self::ObjectiveActivation,
        Self::FormulaInitialization,
        Self::RuleInstantiation,
        Self::Coherence,
        Self::SupportGuards,
        Self::TheoryValidation,
    ];

    /// Stable machine-readable identifier; no allocation or clock access.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::SupportCompletion => "support_completion",
            Self::DomainAnalysis => "domain_analysis",
            Self::ObjectiveActivation => "objective_activation",
            Self::FormulaInitialization => "formula_initialization",
            Self::RuleInstantiation => "rule_instantiation",
            Self::Coherence => "coherence",
            Self::SupportGuards => "support_guards",
            Self::TheoryValidation => "theory_validation",
        }
    }
}

/// How one phase returned; this is not a semantic membership verdict.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum GroundingOutcome {
    /// The phase returned `Ok`; later phases can still fail.
    Completed,
    /// The phase returned an admission or resource error.
    Failed,
    /// The phase did not return because its execution unwound.
    Unwound,
}

impl GroundingOutcome {
    /// Every currently defined phase outcome, for fixed-size caller aggregation.
    pub const ALL: [Self; 3] = [Self::Completed, Self::Failed, Self::Unwound];

    /// Stable machine-readable identifier; no allocation or clock access.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Unwound => "unwound",
        }
    }
}

/// Exact selected work populations for one phase, including partial failures.
///
/// Every field starts at `Some(0)`. `None` means that field overflowed `u64`;
/// other fields remain usable. Work counts and named capacity receipts are not
/// durations and do not alter resource admission. A failed operation contributes only
/// the events reached before its error. Recording uses constant space and one
/// checked addition or maximum per event; event collection itself has diagnostic overhead.
/// These fields do not cover every operation; for example, theory validation
/// can take time while every counter remains zero. Combine phase-local work
/// and publication counts by checked addition; combine capacity peaks by maximum.
/// Propagate `None` whenever a constituent is unavailable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct GroundingWork {
    /// Support rounds admitted by the existing round ceiling.
    pub support_rounds: Option<u64>,
    /// New support atoms inserted while publishing a support round.
    pub support_atoms: Option<u64>,
    /// Column/row associations inserted into support indices, including rebuilds.
    pub support_index_entries: Option<u64>,
    /// Bound-column probe attempts, including absent relations or empty matches.
    pub join_probes: Option<u64>,
    /// Probes executed by the indexed positive-row matcher.
    pub indexed_probes: Option<u64>,
    /// Completed-support table probes using indexed matching for structural
    /// patterns or absent relations. Support-growth phases remain indexed.
    pub table_inapplicable_probes: Option<u64>,
    /// Prepared table indices successfully retained by this grounding attempt.
    pub table_preparations: Option<u64>,
    /// Table probes reusing an already prepared predicate and alias scope.
    pub table_reuses: Option<u64>,
    /// Finite-table selections attempted over complete possible support.
    pub table_probes: Option<u64>,
    /// Table-selected row occurrences visited by the join, before any optional
    /// necessary-domain guard and the unchanged full matcher.
    pub table_rows: Option<u64>,
    /// Completed charged work inside finite-table index preparation.
    pub table_prepare_work: Option<u64>,
    /// Completed charged work inside finite-table row selection.
    pub table_query_work: Option<u64>,
    /// Additional index capacity successfully retained, excluding cache slots.
    pub table_index_bytes: Option<u64>,
    /// Highest reported admitted support/query capacity, including live masks
    /// and conservative operation peaks. Refused proposals do not contribute;
    /// actual allocator slack can. Not process RSS or a complete stack measure.
    pub support_peak_bytes: Option<u64>,
    /// Charged optional applicability, analysis and guard-preparation work.
    /// Includes completed logical work before a conservative analysis stop.
    pub domain_prepare_work: Option<u64>,
    /// Original rows inspected by a prepared final-rule domain guard.
    pub domain_guard_rows: Option<u64>,
    /// Domain equality-ID membership comparisons performed on those rows.
    pub domain_guard_checks: Option<u64>,
    /// Rows rejected before binding by a necessary-domain guard. These rows
    /// have no complete positive continuation in the admitted pure profile.
    pub domain_rejected_rows: Option<u64>,
    /// Existing support rows selected for an attempted pattern match.
    pub join_rows: Option<u64>,
    /// Base relational binding snapshots successfully copied by the join cursor.
    ///
    /// These may contain placeholders for later generated assignments. This is
    /// not the number of completed generated bindings or emitted ground rules.
    pub binding_snapshots: Option<u64>,
    /// Expression nodes visited while checking whether comparison inputs are bound.
    pub readiness_nodes: Option<u64>,
    /// Expression evaluations entered, including ones that subsequently fail.
    pub expression_evaluations: Option<u64>,
    /// Expression operations admitted by the work ceiling, including failed operations.
    pub expression_nodes: Option<u64>,
    /// Fully instantiated atom identities presented to the formula atom catalog.
    pub atom_lookups: Option<u64>,
    /// New atom identities inserted into the formula atom catalog.
    pub atoms_inserted: Option<u64>,
    /// Formula node identities presented to structural interning after the work check.
    pub node_lookups: Option<u64>,
    /// New structurally interned nodes, including imported aggregate nodes.
    pub nodes_inserted: Option<u64>,
    /// Formula roots and their origins successfully appended.
    pub roots: Option<u64>,
}

impl Default for GroundingWork {
    fn default() -> Self {
        Self {
            support_rounds: Some(0),
            support_atoms: Some(0),
            support_index_entries: Some(0),
            join_probes: Some(0),
            indexed_probes: Some(0),
            table_inapplicable_probes: Some(0),
            table_preparations: Some(0),
            table_reuses: Some(0),
            table_probes: Some(0),
            table_rows: Some(0),
            table_prepare_work: Some(0),
            table_query_work: Some(0),
            table_index_bytes: Some(0),
            support_peak_bytes: Some(0),
            domain_prepare_work: Some(0),
            domain_guard_rows: Some(0),
            domain_guard_checks: Some(0),
            domain_rejected_rows: Some(0),
            join_rows: Some(0),
            binding_snapshots: Some(0),
            readiness_nodes: Some(0),
            expression_evaluations: Some(0),
            expression_nodes: Some(0),
            atom_lookups: Some(0),
            atoms_inserted: Some(0),
            node_lookups: Some(0),
            nodes_inserted: Some(0),
            roots: Some(0),
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum Event {
    SupportRound,
    SupportAtom,
    SupportIndexEntry,
    JoinProbe,
    IndexedProbe,
    TableInapplicableProbe,
    TablePreparation,
    TableReuse,
    TableProbe,
    TableRow,
    TablePrepareWork(u64),
    TableQueryWork(u64),
    TableIndexBytes(usize),
    SupportPeakBytes(u128),
    DomainPrepareWork(u64),
    DomainGuardRow,
    DomainGuardCheck,
    DomainRejectedRow,
    JoinRow,
    BindingSnapshot,
    ReadinessNode,
    ExpressionEvaluation,
    ExpressionNode,
    AtomLookup,
    AtomInserted,
    NodeLookup,
    NodeInserted,
    Root,
}

impl GroundingWork {
    /// Combine snapshots in constant time and space: add work/capacity publication
    /// counts and retain the maximum support capacity peak.
    ///
    /// A field is unavailable if either input is unavailable or their sum
    /// overflows `u64`; other fields retain their exact sums. This operation
    /// performs no timing, allocation, resource admission or semantic check.
    #[must_use]
    pub fn checked_sum(self, other: Self) -> Self {
        let sum = |left: Option<u64>, right: Option<u64>| {
            left.and_then(|left| right.and_then(|right| left.checked_add(right)))
        };
        Self {
            support_rounds: sum(self.support_rounds, other.support_rounds),
            support_atoms: sum(self.support_atoms, other.support_atoms),
            support_index_entries: sum(self.support_index_entries, other.support_index_entries),
            join_probes: sum(self.join_probes, other.join_probes),
            indexed_probes: sum(self.indexed_probes, other.indexed_probes),
            table_inapplicable_probes: sum(
                self.table_inapplicable_probes,
                other.table_inapplicable_probes,
            ),
            table_preparations: sum(self.table_preparations, other.table_preparations),
            table_reuses: sum(self.table_reuses, other.table_reuses),
            table_probes: sum(self.table_probes, other.table_probes),
            table_rows: sum(self.table_rows, other.table_rows),
            table_prepare_work: sum(self.table_prepare_work, other.table_prepare_work),
            table_query_work: sum(self.table_query_work, other.table_query_work),
            table_index_bytes: sum(self.table_index_bytes, other.table_index_bytes),
            support_peak_bytes: self
                .support_peak_bytes
                .zip(other.support_peak_bytes)
                .map(|(a, b)| a.max(b)),
            join_rows: sum(self.join_rows, other.join_rows),
            domain_prepare_work: sum(self.domain_prepare_work, other.domain_prepare_work),
            domain_guard_rows: sum(self.domain_guard_rows, other.domain_guard_rows),
            domain_guard_checks: sum(self.domain_guard_checks, other.domain_guard_checks),
            domain_rejected_rows: sum(self.domain_rejected_rows, other.domain_rejected_rows),
            binding_snapshots: sum(self.binding_snapshots, other.binding_snapshots),
            readiness_nodes: sum(self.readiness_nodes, other.readiness_nodes),
            expression_evaluations: sum(self.expression_evaluations, other.expression_evaluations),
            expression_nodes: sum(self.expression_nodes, other.expression_nodes),
            atom_lookups: sum(self.atom_lookups, other.atom_lookups),
            atoms_inserted: sum(self.atoms_inserted, other.atoms_inserted),
            node_lookups: sum(self.node_lookups, other.node_lookups),
            nodes_inserted: sum(self.nodes_inserted, other.nodes_inserted),
            roots: sum(self.roots, other.roots),
        }
    }

    fn record(&mut self, event: Event) {
        let amount = match event {
            Event::TablePrepareWork(work)
            | Event::TableQueryWork(work)
            | Event::DomainPrepareWork(work) => Some(work),
            Event::TableIndexBytes(bytes) => u64::try_from(bytes).ok(),
            Event::SupportPeakBytes(bytes) => u64::try_from(bytes).ok(),
            _ => Some(1),
        };
        if matches!(event, Event::SupportPeakBytes(_)) {
            self.support_peak_bytes = self.support_peak_bytes.zip(amount).map(|(a, b)| a.max(b));
            return;
        }
        let field = match event {
            Event::SupportRound => &mut self.support_rounds,
            Event::SupportAtom => &mut self.support_atoms,
            Event::SupportIndexEntry => &mut self.support_index_entries,
            Event::JoinProbe => &mut self.join_probes,
            Event::IndexedProbe => &mut self.indexed_probes,
            Event::TableInapplicableProbe => &mut self.table_inapplicable_probes,
            Event::TablePreparation => &mut self.table_preparations,
            Event::TableReuse => &mut self.table_reuses,
            Event::TableProbe => &mut self.table_probes,
            Event::TableRow => &mut self.table_rows,
            Event::TablePrepareWork(_) => &mut self.table_prepare_work,
            Event::TableQueryWork(_) => &mut self.table_query_work,
            Event::TableIndexBytes(_) => &mut self.table_index_bytes,
            Event::SupportPeakBytes(_) => &mut self.support_peak_bytes,
            Event::DomainPrepareWork(_) => &mut self.domain_prepare_work,
            Event::DomainGuardRow => &mut self.domain_guard_rows,
            Event::DomainGuardCheck => &mut self.domain_guard_checks,
            Event::DomainRejectedRow => &mut self.domain_rejected_rows,
            Event::JoinRow => &mut self.join_rows,
            Event::BindingSnapshot => &mut self.binding_snapshots,
            Event::ReadinessNode => &mut self.readiness_nodes,
            Event::ExpressionEvaluation => &mut self.expression_evaluations,
            Event::ExpressionNode => &mut self.expression_nodes,
            Event::AtomLookup => &mut self.atom_lookups,
            Event::AtomInserted => &mut self.atoms_inserted,
            Event::NodeLookup => &mut self.node_lookups,
            Event::NodeInserted => &mut self.nodes_inserted,
            Event::Root => &mut self.roots,
        };
        *field = field.and_then(|value| amount.and_then(|amount| value.checked_add(amount)));
    }
}

/// Shared only between the phase guard and the existing mutable work counter.
/// No allocation occurs unless detailed attribution was explicitly enabled.
/// This serial-attempt bank makes its owning counter non-Send even when disabled.
/// A future parallel grounder should retain worker-local value counters and merge
/// their public snapshots, rather than add a global lock to these event sites.
#[derive(Clone, Default)]
pub(crate) struct Work(Option<Rc<RefCell<GroundingWork>>>);

impl Work {
    pub(crate) fn record(&self, event: Event) {
        if let Some(work) = &self.0 {
            work.borrow_mut().record(event);
        }
    }
}

pub(crate) struct Profile<'a> {
    observer: Option<&'a dyn GroundingObserver>,
    source_observer: Option<&'a dyn GroundingObserver>,
    work: Work,
}

impl<'a> Profile<'a> {
    pub(crate) fn new(observer: Option<&'a dyn GroundingObserver>) -> Self {
        let source_observer = observer;
        let observer = observer.filter(|observer| observer.details_enabled());
        let work = Work(observer.map(|_| Rc::new(RefCell::new(GroundingWork::default()))));
        Self {
            observer,
            source_observer,
            work,
        }
    }

    pub(crate) fn domain_analysis(&self, observation: super::DomainObservation<'_, '_>) {
        if let Some(observer) = self.source_observer {
            observer.domain_analysis(observation);
        }
    }

    pub(crate) fn work(&self) -> Work {
        self.work.clone()
    }

    pub(crate) fn phase<T, E>(
        &self,
        phase: GroundingPhase,
        location: Option<Location>,
        action: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, E> {
        let Some(observer) = self.observer else {
            return action();
        };
        *self
            .work
            .0
            .as_ref()
            .expect("enabled counter bank")
            .borrow_mut() = GroundingWork::default();
        observer.phase_enter(phase, location);
        let mut exit = Exit {
            profile: self,
            phase,
            location,
            outcome: GroundingOutcome::Unwound,
        };
        let result = action();
        exit.outcome = if result.is_ok() {
            GroundingOutcome::Completed
        } else {
            GroundingOutcome::Failed
        };
        result
    }
}

struct Exit<'a, 'b> {
    profile: &'a Profile<'b>,
    phase: GroundingPhase,
    location: Option<Location>,
    outcome: GroundingOutcome,
}

impl Drop for Exit<'_, '_> {
    fn drop(&mut self) {
        let work = *self
            .profile
            .work
            .0
            .as_ref()
            .expect("enabled counter bank")
            .borrow();
        self.profile.observer.expect("enabled observer").phase_exit(
            self.phase,
            self.location,
            self.outcome,
            work,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Observer(RefCell<Vec<(GroundingOutcome, GroundingWork)>>);

    impl GroundingObserver for Observer {
        fn enter(&self) {}
        fn exit(&self) {}
        fn details_enabled(&self) -> bool {
            true
        }
        fn phase_exit(
            &self,
            _phase: GroundingPhase,
            _location: Option<Location>,
            outcome: GroundingOutcome,
            work: GroundingWork,
        ) {
            self.0.borrow_mut().push((outcome, work));
        }
    }

    #[test]
    fn unwind_retains_the_phase_work_prefix() {
        let observer = Observer::default();
        let profile = Profile::new(Some(&observer));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            profile.phase(
                GroundingPhase::RuleInstantiation,
                None,
                || -> Result<(), ()> {
                    profile.work.record(Event::JoinRow);
                    panic!("controlled action unwind");
                },
            )
        }));
        assert!(result.is_err());
        let records = observer.0.borrow();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].0, GroundingOutcome::Unwound);
        assert_eq!(records[0].1.join_rows, Some(1));
    }

    #[test]
    fn overflow_is_local_to_one_counter_and_phase() {
        let observer = Observer::default();
        let profile = Profile::new(Some(&observer));
        for overflow in [true, false] {
            profile
                .phase(GroundingPhase::RuleInstantiation, None, || {
                    if overflow {
                        profile.work.0.as_ref().unwrap().borrow_mut().join_rows = Some(u64::MAX);
                    }
                    profile.work.record(Event::JoinRow);
                    profile.work.record(Event::NodeInserted);
                    Ok::<_, ()>(())
                })
                .unwrap();
        }
        let records = observer.0.borrow();
        assert_eq!(records[0].1.join_rows, None);
        assert_eq!(records[0].1.nodes_inserted, Some(1));
        assert_eq!(records[1].1.join_rows, Some(1));
        assert_eq!(records[1].1.nodes_inserted, Some(1));
    }

    #[test]
    fn disabled_profile_has_no_counter_allocation() {
        struct BoundaryOnly;
        impl GroundingObserver for BoundaryOnly {
            fn enter(&self) {}
            fn exit(&self) {}
            fn phase_enter(&self, _phase: GroundingPhase, _location: Option<Location>) {
                panic!("detail callbacks require explicit opt-in");
            }
        }
        for observer in [None, Some(&BoundaryOnly as &dyn GroundingObserver)] {
            let profile = Profile::new(observer);
            assert!(profile.work.0.is_none());
            assert_eq!(
                profile.phase(GroundingPhase::SupportCompletion, None, || Err::<(), _>(7)),
                Err(7)
            );
        }
    }
}
