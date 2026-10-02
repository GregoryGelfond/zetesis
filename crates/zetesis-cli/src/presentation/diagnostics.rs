//! Streaming typed metadata actions; arbitrary diagnostic bytes pass unchanged.

use super::ColorMode;
use std::{
    fmt,
    io::{self, Write},
};

pub(crate) enum Label {
    Source,
    Oracle,
    Grounding,
    Backend,
}
impl Label {
    const fn text(self) -> &'static str {
        match self {
            Self::Source => "Source",
            Self::Oracle => "Oracle",
            Self::Grounding => "Grounding",
            Self::Backend => "Backend",
        }
    }
}

// A borrowed or owned writer plus one explicit policy, with no environment
// lookup or terminal discovery. Metadata adds fixed label/style
// bytes and delegates value formatting and I/O costs to their implementations.
// Write calls and their failures remain observable in order.
pub(crate) struct Diagnostics<W> {
    writer: W,
    color: ColorMode,
    quiet: bool,
    width: std::num::NonZeroUsize,
    core: Option<Core>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Core {
    Constraints,
    TerminalDefinitions,
}
impl<W: Write> Diagnostics<W> {
    pub(crate) const fn new(writer: W, color: ColorMode) -> Self {
        Self {
            writer,
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
            self.color.metadata(&mut self.writer, label.text(), value)
        }
    }

    pub(crate) fn information(&mut self, value: fmt::Arguments<'_>) -> io::Result<()> {
        if self.quiet {
            Ok(())
        } else {
            writeln!(self.writer, "{value}")
        }
    }

    pub(crate) fn diagnostic(&mut self, diagnostic: &impl fmt::Display) -> io::Result<()> {
        super::source_error::write(&mut self.writer, self.color, diagnostic)
    }
}

impl<W: Write> Write for Diagnostics<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.writer.write(bytes)
    }
    fn write_all(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.writer.write_all(bytes)
    }
    fn write_fmt(&mut self, args: fmt::Arguments<'_>) -> io::Result<()> {
        self.writer.write_fmt(args)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}

impl<W: Write> crate::ExecutionObserver for Diagnostics<W> {
    type Error = io::Error;
    fn observe(&mut self, observation: crate::ExecutionObservation<'_>) -> Result<(), Self::Error> {
        use crate::ExecutionObservation as Event;
        if self.quiet
            && !matches!(
                &observation,
                Event::KeyAnalysisStopped(_)
                    | Event::ObjectiveUnavailable(_)
                    | Event::ObjectiveBoundStopped(_)
                    | Event::ObjectiveTheoryMismatch
                    | Event::ObjectiveRestrictionStopped(_)
            )
        {
            return Ok(());
        }
        match observation {
            Event::StaticGrounding { requested, atoms, rules, limits } => self.metadata(
                Label::Grounding,
                format_args!("requested={}, effective=eager (static atoms={atoms}, rules={rules}; lowering caps atoms={}, rules={}, substitutions={})", requested.label(), limits.max_atoms, limits.max_ground_rules, limits.max_substitutions),
            ),
            Event::LazyGrounding { requested } => self.metadata(Label::Grounding,
                format_args!("requested={}, effective=lazy (source joins; no complete ground-rule store)", requested.label())),
            Event::TerminalDefinitions { requested, deferred_templates } => {
                self.core = Some(Core::TerminalDefinitions);
                self.metadata(Label::Grounding, format_args!("requested={}, effective=eager_base_terminal_definitions (eager base; {deferred_templates} terminal definitions reconstructed on the host before each original answer)", requested.label()))
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
            Event::CpuFormula { oracle, grounder, search } => {
                let oracle = if oracle == crate::Oracle::Auto { "Ferraris reduct membership" } else { "Ferraris reduct countermodel" };
                if self.core == Some(Core::TerminalDefinitions) {
                    self.metadata(Label::Backend, format_args!("cpu; base oracle: {oracle}; search: {}; base grounding: eager; original answer reconstruction: host", search.label()))
                } else if self.core == Some(Core::Constraints) {
                    self.metadata(Label::Backend, format_args!("cpu; retained-core oracle: {oracle}; search: {}; core grounding: eager", search.label()))
                } else {
                    self.metadata(Label::Backend, format_args!("cpu; oracle: {oracle}; search: {}; grounder: eager (requested {})", search.label(), grounder.label()))
                }
            }
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
            Event::DeviceFormula { adapter, grounder, search, batch_size, completion_workers, .. } => self.metadata(Label::Backend,
                format_args!("hybrid GPU propagation + exact CPU residual search ({}, {}; vendor=0x{:04x}); oracle: Ferraris reduct countermodel; search: {}; grounder: eager (requested {}); batch={batch_size}; CPU completion requested workers={completion_workers}", adapter.name, adapter.backend, adapter.vendor_id, search.label(), grounder.label())),
            #[cfg(feature = "gpu")]
            Event::DeviceTight { adapter, grounder, search, batch_size } => self.metadata(Label::Backend,
                format_args!("GPU tight support ({}, {}; vendor=0x{:04x}); oracle: Ferraris ranked support; search: {}; grounder: eager (requested {}); batch={batch_size}", adapter.name, adapter.backend, adapter.vendor_id, search.label(), grounder.label())),
            Event::Formula { atoms, nodes, roots, keyed_constraints } if self.core == Some(Core::TerminalDefinitions) => writeln!(self,
                "Base formula: {atoms} atoms, {nodes} nodes, {roots} roots; {keyed_constraints} constraints asked by key; terminal definitions reconstructed separately"),
            Event::Formula { atoms, nodes, roots, keyed_constraints } if self.core == Some(Core::Constraints) => writeln!(self,
                "Retained core formula: {atoms} atoms, {nodes} nodes, {roots} roots; {keyed_constraints} constraints asked by key; streamed constraints checked separately"),
            Event::Formula { atoms, nodes, roots, keyed_constraints: 0 } => writeln!(self, "Formula: {atoms} atoms, {nodes} nodes, {roots} roots"),
            Event::Formula { atoms, nodes, roots, keyed_constraints } => writeln!(self,
                "Formula: {atoms} atoms, {nodes} nodes, {roots} roots; {keyed_constraints} constraints asked by key"),
            Event::KeyAnalysisStopped(stop) => writeln!(self,
                "Keyed constraints: the key analysis stopped, {stop}; every constraint not yet asked was grounded as written"),
            Event::TightMembership if self.core == Some(Core::TerminalDefinitions) => writeln!(self, "Base membership: checked tight support certificate; full reconstruction still pending"),
            Event::TightMembership if self.core == Some(Core::Constraints) => writeln!(self, "Core membership: checked tight support certificate; original constraints still pending"),
            Event::TightMembership => writeln!(self, "Membership: checked tight support certificate over the original theory"),
            Event::PositiveMembership if self.core == Some(Core::TerminalDefinitions) => writeln!(self, "Base membership: positive atomic-head theory; full reconstruction still pending"),
            Event::PositiveMembership if self.core == Some(Core::Constraints) => writeln!(self, "Core membership: positive atomic-head theory; original streamed constraints still pending"),
            Event::PositiveMembership => writeln!(self, "Membership: positive atomic-head theory; least consequences with original constraints"),
            Event::GeneralMembership(error) => writeln!(self, "Membership: general reduct; optional class certificate refused: {error}"),
            Event::ObjectiveUnavailable(error) => writeln!(self, "Objective preparation unavailable: {error}; detailed scoring is subject to the remaining objective allowance"),
            Event::ObjectiveBoundStopped(error) => writeln!(self, "Objective pruning stopped: {error}; exact search continues"),
            Event::ObjectiveTheoryMismatch => writeln!(self, "Objective pruning stopped: original theory mismatch; exact search continues"),
            Event::ObjectiveRestrictionStopped(error) => writeln!(self, "Objective pruning stopped: {error}; exact search continues"),
            Event::ObjectiveBound { restrictions, costs, work } => writeln!(self, "Objective pruning: bound {restrictions}; cost <= {costs:?}; construction work {work}"),
        }
    }
}

#[cfg(test)]
mod tests;
