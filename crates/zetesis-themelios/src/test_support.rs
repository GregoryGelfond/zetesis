//! Helpers the crate's unit tests share across its modules.

use std::cell::Cell;

use themelios_base::source::SourceId;
use themelios_base::span::{ByteOffset, Location, Span};

use zetesis_core::{Atom, ValueLimits};
use zetesis_ferraris::{Interpretation, Theory};

use crate::formula::Compiled;
use crate::{
    AdmissionOptions, AdmittedFormula, ExpansionLimits, FormulaLimits, GroundingObserver,
    GroundingOutcome, GroundingPhase, GroundingWork, HybridFormula, PreparedFormula,
};

/// The empty span at the start of source 0.
pub(crate) fn location() -> crate::ProgramSite {
    site(0, 0)
}

/// An empty span neither in the first source nor at the start of one, so that
/// a site an operation reports is seen to be the one it was given rather than
/// a default.
pub(crate) fn given_site() -> crate::ProgramSite {
    site(37, 12)
}

fn site(source: u32, offset: u32) -> crate::ProgramSite {
    crate::ProgramSite::source(Location {
        source: SourceId::new(source),
        span: Span::empty(ByteOffset::new(offset)),
    })
}

/// A grounding observer that keeps the work of the last phase to exit.
#[derive(Default)]
pub(crate) struct Observer(pub(crate) Cell<GroundingWork>);

impl GroundingObserver for Observer {
    fn enter(&self) {}
    fn exit(&self) {}
    fn details_enabled(&self) -> bool {
        true
    }
    fn phase_exit(
        &self,
        _: GroundingPhase,
        _: Option<crate::ProgramSite>,
        _: GroundingOutcome,
        work: GroundingWork,
    ) {
        self.0.set(work);
    }
}

/// The interpretation of `theory` holding the atoms whose bits `bits` sets.
pub(crate) fn interpretation(theory: &Theory, bits: usize) -> Interpretation {
    Interpretation::new(
        theory,
        (0..theory.atom_count()).filter(|atom| bits & (1 << atom) != 0),
    )
    .unwrap()
}

/// The atoms of `compiled`, in catalog order.
pub(crate) fn compiled_atoms(compiled: &Compiled) -> Vec<Atom> {
    compiled
        .atoms
        .atoms()
        .iter()
        .map(|atom| atom.to_atom(ValueLimits::default()).unwrap())
        .collect()
}

/// `source` admitted as a formula program under the default options and
/// limits, which must admit it.
pub(crate) fn formula(source: &str) -> AdmittedFormula {
    crate::admit_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap_or_else(|error| panic!("{source}: {error}"))
}

/// `source` prepared under the default options and limits.
pub(crate) fn prepare(source: &str) -> Result<PreparedFormula, crate::FormulaFailure> {
    crate::prepare_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
}

/// `source` prepared under the default options and limits, which must admit
/// it.
pub(crate) fn prepared(source: &str) -> PreparedFormula {
    prepare(source).unwrap_or_else(|error| panic!("{source}: {error}"))
}

/// `source` prepared under the default options and limits and grounded for
/// hybrid execution, which it must allow.
pub(crate) fn hybrid(source: &str) -> HybridFormula {
    prepared(source).ground_hybrid().unwrap()
}

thread_local! {
    /// Whether constraint grounding collects pairwise exclusions on this
    /// thread: on, as in every run, except inside `without_pair_exclusions`.
    static PAIR_EXCLUSIONS: Cell<bool> = const { Cell::new(true) };
}

/// Whether pairwise-exclusion collection is on for this thread.
pub(crate) fn pair_exclusions_enabled() -> bool {
    PAIR_EXCLUSIONS.get()
}

/// Run `run` with pairwise-exclusion collection off on this thread, restoring
/// it afterwards even if `run` unwinds: every pairwise constraint then takes
/// the route it would take were it not a pair family.
pub(crate) fn without_pair_exclusions<T>(run: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            PAIR_EXCLUSIONS.set(self.0);
        }
    }
    let _restore = Restore(PAIR_EXCLUSIONS.replace(false));
    run()
}

/// `source` grounded for hybrid execution with its constraints streamed: the
/// fixture of the streamed route's own tests. Capture would move a pairwise
/// family whose every component is complete into the core, and these tests
/// exercise the stream, not that disposition.
pub(crate) fn streamed_hybrid(source: &str) -> HybridFormula {
    without_pair_exclusions(|| hybrid(source))
}
