//! Execution-detail records and source diagnostics, written in whole units.
//!
//! A *write unit* is what reaches the diagnostics writer in one call:
//! - an execution-detail record: one line from `metadata`, `information` or an
//!   `observe` arm, or a line the `test` workflow writes with `writeln!`, each
//!   rendered by one formatting call;
//! - each heading, rendered line and closing newline of a styled source
//!   diagnostic;
//! - a plain source diagnostic's final newline. Its `zetesis: ` prefix and each
//!   fragment of its rendering pass through unchanged, as before;
//! - the statistics report, which the driver writes through `write_whole`.
//!
//! `write_whole` renders a unit whole into a transient buffer, then hands it to
//! the writer in one `write_all` call. The writer is held by `UnitWriter`, which
//! admits only whole units and slices passed through, so no method can write to
//! the raw writer. A unit built from several calls through the diagnostics
//! writer would make one write per call, so such a unit is rendered inside
//! `write_whole` instead.
//!
//! The transient buffer belongs to one call. It holds 256 bytes before it first
//! grows, enough for every record the maintained programs write, and a longer
//! unit grows by doubling. It is dropped on return, so no unit's bytes reach the
//! writer after its call has returned, and no capacity outlives the unit.
//!
//! A writer whose acceptance and failure depend only on the bytes it is given,
//! in order, such as a full disk, a closed pipe, a blocking pipe or a regular
//! file, sees the same bytes, fails at the same byte and fails in the same
//! unit's call as when records were written fragment by fragment. A writer
//! that depends on call boundaries sees whole units. A unit whose rendering
//! fails writes nothing and returns an `Other` error.

mod unit_writer;

use super::ColorMode;
use std::{
    fmt,
    io::{self, Write},
};
use unit_writer::{UnitBuffer, UnitWriter};

pub(crate) enum Label {
    Source,
    Oracle,
    Comparisons,
    Grounding,
    Backend,
}
impl Label {
    const fn text(self) -> &'static str {
        match self {
            Self::Source => "Source",
            Self::Oracle => "Oracle",
            Self::Comparisons => "Comparisons",
            Self::Grounding => "Grounding",
            Self::Backend => "Backend",
        }
    }
}

// A borrowed or owned writer plus one explicit policy, with no environment
// lookup or terminal discovery. Metadata adds fixed label/style
// bytes and delegates value formatting and I/O costs to their implementations.
// Units reach the writer in the order of their calls, and their failures are
// returned from those calls.
pub(crate) struct Diagnostics<W> {
    units: UnitWriter<W>,
    color: ColorMode,
    quiet: bool,
    width: std::num::NonZeroUsize,
    core: Option<Core>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Core {
    Constraints,
    TerminalDefinitions,
    HybridTerminalDefinitions,
}
impl<W: Write> Diagnostics<W> {
    pub(crate) const fn new(writer: W, color: ColorMode) -> Self {
        Self {
            units: UnitWriter::new(writer),
            color,
            quiet: false,
            width: std::num::NonZeroUsize::new(80).unwrap(),
            core: None,
        }
    }

    /// Suppress informational setup and execution records; warnings and errors remain.
    pub(crate) const fn with_quiet(mut self, quiet: bool) -> Self {
        self.quiet = quiet;
        self
    }

    fn should_report(&self, observation: &crate::ExecutionObservation<'_>) -> bool {
        use crate::ExecutionObservation as Event;
        !self.quiet
            || matches!(
                observation,
                Event::KeyAnalysisStopped(_)
                    | Event::ObjectiveUnavailable(_)
                    | Event::ChoiceObjectiveBoundUnavailable(_)
                    | Event::ObjectiveBoundStopped(_)
                    | Event::ObjectiveTheoryMismatch
                    | Event::ObjectiveRestrictionStopped(_)
            )
    }

    pub(crate) fn for_solve(writer: W, color: ColorMode, options: &crate::Options) -> Self {
        let execution_details = options.json
            || (options.stats && matches!(options.statistics_view, crate::StatisticsView::Records));
        Self::new(writer, color).with_quiet(!execution_details)
    }

    pub(crate) const fn with_width(mut self, width: std::num::NonZeroUsize) -> Self {
        self.width = width;
        self
    }

    pub(crate) const fn layout(&self) -> zetesis_presentation::Layout {
        zetesis_presentation::Layout::new(self.width, self.color)
    }

    pub(crate) fn metadata(&mut self, label: Label, value: fmt::Arguments<'_>) -> io::Result<()> {
        if self.quiet {
            Ok(())
        } else {
            self.color.metadata(&mut self.units, label.text(), value)
        }
    }

    pub(crate) fn information(&mut self, value: fmt::Arguments<'_>) -> io::Result<()> {
        if self.quiet {
            Ok(())
        } else {
            writeln!(self.units, "{value}")
        }
    }

    /// Report which formula is checked and which original-source obligations remain.
    fn cpu_formula(
        &mut self,
        oracle: crate::Oracle,
        grounder: crate::Grounder,
        search: crate::SearchMethod,
    ) -> io::Result<()> {
        let oracle = if oracle == crate::Oracle::Auto {
            "Ferraris reduct membership"
        } else {
            "Ferraris reduct countermodel"
        };
        if self.core == Some(Core::TerminalDefinitions) {
            self.metadata(Label::Backend, format_args!("cpu; base oracle: {oracle}; search: {}; base grounding: eager; original answer reconstruction: host", search.label()))
        } else if self.core == Some(Core::HybridTerminalDefinitions) {
            self.metadata(Label::Backend, format_args!("cpu; base core oracle: {oracle}; search: {}; base grounding: hybrid (eager producer core); original answer reconstruction: host", search.label()))
        } else if self.core == Some(Core::Constraints) {
            self.metadata(
                Label::Backend,
                format_args!(
                    "cpu; retained-core oracle: {oracle}; search: {}; core grounding: eager",
                    search.label()
                ),
            )
        } else {
            self.metadata(
                Label::Backend,
                format_args!(
                    "cpu; oracle: {oracle}; search: {}; grounder: eager (requested {})",
                    search.label(),
                    grounder.label()
                ),
            )
        }
    }

    #[cfg(feature = "gpu")]
    fn device_formula_scope(&self) -> (&'static str, &'static str) {
        match self.core {
            None => ("grounder", ""),
            Some(Core::Constraints) => ("core grounding", "; original constraints: host"),
            Some(Core::TerminalDefinitions) => {
                ("base grounding", "; original answer reconstruction: host")
            }
            Some(Core::HybridTerminalDefinitions) => (
                "base core grounding",
                "; original constraints and answer reconstruction: host",
            ),
        }
    }

    pub(crate) fn diagnostic(&mut self, diagnostic: &impl fmt::Display) -> io::Result<()> {
        super::source_error::write(&mut self.units, self.color, diagnostic)
    }

    /// Render one write unit whole, then hand it to the writer in one
    /// `write_all` call, as every record is. If the rendering fails, nothing of
    /// the unit is written. The driver writes the statistics report through it,
    /// so the report's fragments are not each a write on unbuffered standard
    /// error, its order relative to other diagnostics is unchanged, and a write
    /// failure surfaces at this one write.
    pub(crate) fn write_whole(
        &mut self,
        render: impl FnOnce(&mut UnitBuffer) -> io::Result<()>,
    ) -> io::Result<()> {
        self.units.write_whole(render)
    }
}

impl<W: Write> Write for Diagnostics<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.units.write(bytes)
    }
    fn write_all(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.units.write_all(bytes)
    }
    fn write_fmt(&mut self, args: fmt::Arguments<'_>) -> io::Result<()> {
        self.units.write_fmt(args)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.units.flush()
    }
}

impl<W: Write> crate::ExecutionObserver for Diagnostics<W> {
    type Error = io::Error;
    fn observe(&mut self, observation: crate::ExecutionObservation<'_>) -> Result<(), Self::Error> {
        use crate::ExecutionObservation as Event;
        if !self.should_report(&observation) {
            return Ok(());
        }
        match observation {
            Event::SourceEvaluation => self.metadata(Label::Backend,
                format_args!("cpu (dependency-ordered source evaluation; constraints checked before publication)")),
            Event::StaticGrounding { requested, atoms, rules, limits } => self.metadata(
                Label::Grounding,
                format_args!("requested={}, effective=eager (static atoms={atoms}, rules={rules}; lowering caps atoms={}, rules={}, substitutions={})", requested.label(), limits.max_atoms, limits.max_ground_rules, limits.max_substitutions),
            ),
            Event::LazyGrounding { requested } => self.metadata(Label::Grounding,
                format_args!("requested={}, effective=lazy (source joins; no complete ground-rule store)", requested.label())),
            Event::TerminalDefinitions { requested, base: zetesis_themelios::BaseKind::Eager, deferred_templates, .. } => {
                self.core = Some(Core::TerminalDefinitions);
                self.metadata(Label::Grounding, format_args!("requested={}, effective=eager_base_terminal_definitions (eager base; {deferred_templates} terminal definitions reconstructed on the host before each original answer)", requested.label()))
            }
            Event::TerminalDefinitions { requested, base: zetesis_themelios::BaseKind::Hybrid, deferred_templates, streamed } => {
                self.core = Some(Core::HybridTerminalDefinitions);
                let (templates, instances) = streamed.map_or((0, 0), |streamed| (streamed.templates, streamed.instances));
                self.metadata(Label::Grounding, format_args!("requested={}, effective=hybrid_base_terminal_definitions (hybrid base: eager producer core, streamed constraints: {templates} templates, {instances} admitted instances; {deferred_templates} terminal definitions reconstructed on the host after each base answer is accepted)", requested.label()))
            }
            Event::HybridGrounding { requested, streamed_templates, streamed_instances } => {
                self.core = Some(Core::Constraints);
                self.metadata(Label::Grounding,
                    format_args!("requested={}, effective=hybrid (eager retained core; streamed constraints: {streamed_templates} templates, {streamed_instances} admitted instances)", requested.label()))
            }
            Event::CpuClosure { grounder: crate::Grounder::Eager, workers, .. } => self.metadata(
                Label::Backend, format_args!("cpu (eager static closure scans, {workers} workers)")),
            Event::CpuClosure { batching: crate::SourceBatching::Independent, workers, .. } => self.metadata(
                Label::Backend, format_args!("cpu (lazy source joins, {workers} workers)")),
            Event::CpuClosure { batching, workers, .. } => self.metadata(Label::Backend,
                format_args!("cpu (shared {} source rounds, {workers} workers; collective source and per-world evaluation budgets)", batching.label())),
            Event::CpuFormula { oracle, grounder, search } => self.cpu_formula(oracle, grounder, search),
            Event::ParallelRegions { workers } => writeln!(self,
                "Parallel regions: {workers} workers; models arrive in the schedule's order"),
            Event::ParallelProposals { workers } => writeln!(self,
                "Parallel candidate production: {workers} workers; joined before batch membership checking"),
            Event::ExactCompletion { workers, max_scratch_bytes } => writeln!(self,
                "Exact completion: requested workers={workers}; bounded logical scratch bytes={max_scratch_bytes}"),
            #[cfg(feature = "gpu")]
            Event::LazyDeviceGrounding { requested } => self.metadata(Label::Grounding,
                format_args!("requested={}, effective=lazy (host source joins; per-world device consequences; no complete ground-rule store)", requested.label())),
            #[cfg(feature = "gpu")]
            Event::DeviceClosure { adapter, static_counts } => match static_counts {
                None => self.metadata(Label::Backend, format_args!("gpu ({}, {}; vendor=0x{:04x}; lazy immutable reduct rounds)", adapter.name, adapter.backend, adapter.vendor_id)),
                Some((atoms, rules)) => self.metadata(Label::Backend, format_args!("gpu ({}, {}; vendor=0x{:04x}; static atoms={atoms}, rules={rules})", adapter.name, adapter.backend, adapter.vendor_id)),
            },
            #[cfg(feature = "gpu")]
            Event::DeviceFormula { adapter, grounder, search, batch_size, completion_workers, .. } => {
                let (scope, host) = self.device_formula_scope();
                self.metadata(Label::Backend,
                    format_args!("hybrid GPU propagation + exact CPU residual search ({}, {}; vendor=0x{:04x}); oracle: Ferraris reduct countermodel; search: {}; {scope}: eager (requested {}); batch={batch_size}; CPU completion requested workers={completion_workers}{host}", adapter.name, adapter.backend, adapter.vendor_id, search.label(), grounder.label()))
            },
            #[cfg(feature = "gpu")]
            Event::DeviceTight { adapter, grounder, search, batch_size } => {
                let (scope, host) = self.device_formula_scope();
                self.metadata(Label::Backend,
                    format_args!("GPU tight support ({}, {}; vendor=0x{:04x}); oracle: Ferraris ranked support; search: {}; {scope}: eager (requested {}); batch={batch_size}{host}", adapter.name, adapter.backend, adapter.vendor_id, search.label(), grounder.label()))
            },
            Event::Formula { atoms, nodes, operands, roots, keyed_constraints } if self.core == Some(Core::TerminalDefinitions) => writeln!(self,
                "Base formula: {atoms} atoms, {nodes} nodes, {roots} roots; {operands} operand occurrences; {keyed_constraints} constraints asked by key; terminal definitions reconstructed separately"),
            Event::Formula { atoms, nodes, operands, roots, keyed_constraints } if self.core == Some(Core::HybridTerminalDefinitions) => writeln!(self,
                "Retained base core formula: {atoms} atoms, {nodes} nodes, {roots} roots; {operands} operand occurrences; {keyed_constraints} constraints asked by key; streamed constraints and terminal definitions checked separately"),
            Event::Formula { atoms, nodes, operands, roots, keyed_constraints } if self.core == Some(Core::Constraints) => writeln!(self,
                "Retained core formula: {atoms} atoms, {nodes} nodes, {roots} roots; {operands} operand occurrences; {keyed_constraints} constraints asked by key; streamed constraints checked separately"),
            Event::Formula { atoms, nodes, operands, roots, keyed_constraints: 0 } => writeln!(self, "Formula: {atoms} atoms, {nodes} nodes, {roots} roots; {operands} operand occurrences"),
            Event::Formula { atoms, nodes, operands, roots, keyed_constraints } => writeln!(self,
                "Formula: {atoms} atoms, {nodes} nodes, {roots} roots; {operands} operand occurrences; {keyed_constraints} constraints asked by key"),
            Event::KeyAnalysisStopped(stop) => writeln!(self,
                "Keyed constraints: the key analysis stopped, {stop}; every constraint not yet asked was grounded as written"),
            Event::TightMembership if self.core == Some(Core::HybridTerminalDefinitions) => writeln!(self, "Base core membership: checked tight support certificate; original constraints and reconstruction still pending"),
            Event::TightMembership if self.core == Some(Core::TerminalDefinitions) => writeln!(self, "Base membership: checked tight support certificate; full reconstruction still pending"),
            Event::TightMembership if self.core == Some(Core::Constraints) => writeln!(self, "Core membership: checked tight support certificate; original constraints still pending"),
            Event::TightMembership => writeln!(self, "Membership: checked tight support certificate over the original theory"),
            Event::PositiveMembership if self.core == Some(Core::HybridTerminalDefinitions) => writeln!(self, "Base core membership: positive atomic-head theory; original constraints and reconstruction still pending"),
            Event::PositiveMembership if self.core == Some(Core::TerminalDefinitions) => writeln!(self, "Base membership: positive atomic-head theory; full reconstruction still pending"),
            Event::PositiveMembership if self.core == Some(Core::Constraints) => writeln!(self, "Core membership: positive atomic-head theory; original streamed constraints still pending"),
            Event::PositiveMembership => writeln!(self, "Membership: positive atomic-head theory; least consequences with original constraints"),
            Event::StratifiedMembership if self.core == Some(Core::HybridTerminalDefinitions) => writeln!(self, "Base core membership: stratified normal theory; original constraints and reconstruction still pending"),
            Event::StratifiedMembership if self.core == Some(Core::TerminalDefinitions) => writeln!(self, "Base membership: stratified normal theory; full reconstruction still pending"),
            Event::StratifiedMembership if self.core == Some(Core::Constraints) => writeln!(self, "Core membership: stratified normal theory; original streamed constraints still pending"),
            Event::StratifiedMembership => writeln!(self, "Membership: stratified normal theory; direct evaluation with original constraints"),
            Event::GeneralMembership(error) => writeln!(self, "Membership: general reduct; optional class certificate refused: {error}"),
            Event::ObjectiveUnavailable(error) => writeln!(self, "Objective preparation unavailable: {error}; detailed scoring is subject to the remaining objective allowance"),
            Event::ChoiceObjectiveBoundUnavailable(error) => writeln!(self, "Required-choice objective bound unavailable: {error}; exact objective bounds remain available"),
            Event::ObjectiveBoundStopped(error) => writeln!(self, "Objective pruning stopped: {error}; exact search continues"),
            Event::ObjectiveTheoryMismatch => writeln!(self, "Objective pruning stopped: original theory mismatch; exact search continues"),
            Event::ObjectiveRestrictionStopped(error) => writeln!(self, "Objective pruning stopped: {error}; exact search continues"),
            Event::ObjectiveBound { restrictions, costs, work } => writeln!(self, "Objective pruning: bound {restrictions}; cost <= {costs:?}; construction work {work}"),
        }
    }
}

#[cfg(test)]
mod tests;
