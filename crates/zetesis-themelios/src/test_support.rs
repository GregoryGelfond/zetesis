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

/// `source` admitted under the default options and limits, which must admit
/// it.
pub(crate) fn admitted(source: &str) -> AdmittedFormula {
    crate::admit_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap_or_else(|error| panic!("{source}: {error}"))
}

/// `source` prepared under the default options and limits, which must admit
/// it.
pub(crate) fn prepared(source: &str) -> PreparedFormula {
    crate::prepare_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap_or_else(|error| panic!("{source}: {error}"))
}

/// `source` prepared under the default options and limits and grounded for
/// hybrid execution, which it must allow.
pub(crate) fn hybrid(source: &str) -> HybridFormula {
    prepared(source).ground_hybrid().unwrap()
}
