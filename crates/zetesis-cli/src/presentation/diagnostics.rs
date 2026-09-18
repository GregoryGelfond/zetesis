//! Streaming typed metadata actions; arbitrary diagnostic bytes pass unchanged.

use super::{ColorMode, RESET};
use std::{
    fmt,
    io::{self, Write},
};

pub(crate) enum Label {
    Source,
    Oracle,
    Grounding,
    Backend,
    Auto,
}
impl Label {
    const fn text(self) -> &'static str {
        match self {
            Self::Source => "Source",
            Self::Oracle => "Oracle",
            Self::Grounding => "Grounding",
            Self::Backend => "Backend",
            Self::Auto => "Auto",
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
}
impl<W: Write> Diagnostics<W> {
    pub(crate) const fn new(writer: W, color: ColorMode) -> Self {
        Self { writer, color }
    }

    pub(crate) fn metadata(&mut self, label: Label, value: fmt::Arguments<'_>) -> io::Result<()> {
        let label = label.text();
        if self.color == ColorMode::Always {
            writeln!(self.writer, "{BLUE}{label}:{ITALIC_GRAY} {value}{RESET}")
        } else {
            writeln!(self.writer, "{label}: {value}")
        }
    }

    pub(crate) fn error(&mut self, error: &impl fmt::Display) -> io::Result<()> {
        super::source_error::write(&mut self.writer, self.color, error)
    }
}

const BLUE: &str = "\u{1b}[34m";
const ITALIC_GRAY: &str = "\u{1b}[3;90m";
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
        match observation {
            Event::StaticGrounding { requested, atoms, rules, limits } => self.metadata(
                Label::Grounding,
                format_args!("requested={}, effective=eager (static atoms={atoms}, rules={rules}; lowering caps atoms={}, rules={}, substitutions={})", requested.label(), limits.max_atoms, limits.max_ground_rules, limits.max_substitutions),
            ),
            Event::LazyGrounding { requested } => self.metadata(Label::Grounding,
                format_args!("requested={}, effective=lazy (source joins; no complete ground-rule store)", requested.label())),
            Event::CpuClosure { grounder: crate::Grounder::Eager, workers, .. } => self.metadata(
                Label::Backend, format_args!("cpu (eager static closure scans, {workers} workers)")),
            Event::CpuClosure { batching: crate::SourceBatching::Independent, workers, .. } => self.metadata(
                Label::Backend, format_args!("cpu (lazy source joins, {workers} workers)")),
            Event::CpuClosure { batching, workers, .. } => self.metadata(Label::Backend,
                format_args!("cpu (shared {} source rounds, {workers} workers; collective source and per-world evaluation budgets)", batching.label())),
            Event::CpuFormula { oracle, grounder, candidates } => {
                let oracle = if oracle == crate::Oracle::Auto { "Ferraris reduct membership" } else { "Ferraris reduct countermodel" };
                self.metadata(Label::Backend, format_args!("cpu; oracle: {oracle}; candidates: {}; grounder: eager (requested {})", candidates.label(), grounder.label()))
            }
            Event::ExactCompletion { workers, max_scratch_bytes } => writeln!(self,
                "Exact completion: requested workers={workers}; bounded logical scratch bytes={max_scratch_bytes}"),
            Event::AutomaticCpu => self.metadata(Label::Auto,
                format_args!("CPU selected; no measured GPU crossover for this execution profile.")),
            Event::SharedCpu => self.metadata(Label::Auto, format_args!("explicit shared source batching selects CPU without device discovery.")),
            Event::DeviceNotCompiled => self.metadata(Label::Auto,
                format_args!("GPU support was not compiled; using CPU without device discovery.")),
            #[cfg(feature = "gpu")]
            Event::LazyDeviceGrounding { requested } => self.metadata(Label::Grounding,
                format_args!("requested={}, effective=lazy (host source joins; per-world device consequences; no complete ground-rule store)", requested.label())),
            #[cfg(feature = "gpu")]
            Event::DeviceClosure { adapter, static_counts } => match static_counts {
                None => self.metadata(Label::Backend, format_args!("gpu ({}, {}; vendor=0x{:04x}; lazy immutable reduct rounds)", adapter.name, adapter.backend, adapter.vendor_id)),
                Some((atoms, rules)) => self.metadata(Label::Backend, format_args!("gpu ({}, {}; vendor=0x{:04x}; static atoms={atoms}, rules={rules})", adapter.name, adapter.backend, adapter.vendor_id)),
            },
            #[cfg(feature = "gpu")]
            Event::DeviceFormula { adapter, grounder, batch_size, completion_workers, .. } => self.metadata(Label::Backend,
                format_args!("hybrid GPU propagation + exact CPU residual search ({}, {}; vendor=0x{:04x}); oracle: Ferraris reduct countermodel; grounder: eager (requested {}); batch={batch_size}; CPU completion requested workers={completion_workers}", adapter.name, adapter.backend, adapter.vendor_id, grounder.label())),
            Event::Formula { atoms, nodes, roots, keyed_constraints: 0 } => writeln!(self, "Formula: {atoms} atoms, {nodes} nodes, {roots} roots"),
            Event::Formula { atoms, nodes, roots, keyed_constraints } => writeln!(self,
                "Formula: {atoms} atoms, {nodes} nodes, {roots} roots; {keyed_constraints} constraints asked by key"),
            Event::TightMembership => writeln!(self, "Membership: checked tight support certificate; exact reduct residual completion"),
            Event::PositiveMembership => writeln!(self, "Membership: positive atomic-head theory; least consequences with original constraints"),
            Event::GeneralMembership(error) => writeln!(self, "Membership: general reduct; optional class certificate refused: {error}"),
            Event::ObjectiveUnavailable(error) => writeln!(self, "Objective pruning unavailable: {error}; exact search continues"),
            Event::ObjectiveBoundStopped(error) => writeln!(self, "Objective pruning stopped: {error}; exact search continues"),
            Event::ObjectiveTheoryMismatch => writeln!(self, "Objective pruning stopped: original theory mismatch; exact search continues"),
            Event::ObjectiveRestrictionStopped(error) => writeln!(self, "Objective pruning stopped: {error}; exact search continues"),
            Event::ObjectiveBound { restrictions, costs, work } => writeln!(self, "Objective pruning: bound {restrictions}; cost <= {costs:?}; construction work {work}"),
        }
    }
}

#[cfg(test)]
#[path = "../../tests/support/objective_writer_contracts.rs"]
mod objective_diagnostic_tests;
