//! Helpers the crate's unit tests share across its modules.

use std::cell::Cell;
use std::collections::BTreeSet;

use themelios_base::source::SourceId;
use themelios_base::span::{ByteOffset, Location, Span};

use zetesis_algebra::Cancellation;
use zetesis_core::{Atom, AtomCatalog, ValueLimits};
use zetesis_ferraris::{Interpretation, Limits, Theory, check};

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
    catalog_atoms(&compiled.atoms)
}

/// The atoms of `catalog`, in its order.
fn catalog_atoms(catalog: &AtomCatalog) -> Vec<Atom> {
    catalog
        .atoms()
        .iter()
        .map(|atom| atom.to_atom(ValueLimits::default()).unwrap())
        .collect()
}

/// The atoms of `compiled` whose predicate is named `name`, in catalog order.
pub(crate) fn atoms_named(compiled: &Compiled, name: &str) -> Vec<Atom> {
    compiled_atoms(compiled)
        .into_iter()
        .filter(|atom| atom.predicate().name() == name)
        .collect()
}

/// The answer sets of `theory` by the definition, each as the atoms of
/// `catalog` it holds: the interpretations the exhaustive reduct check
/// accepts. The check visits every interpretation and the proper subsets of
/// each model, so this is for theories of a few atoms.
pub(crate) fn answer_sets(theory: &Theory, catalog: &AtomCatalog) -> BTreeSet<BTreeSet<Atom>> {
    let atoms = catalog_atoms(catalog);
    assert_eq!(atoms.len(), theory.atom_count());
    assert!(atoms.len() <= 10, "bounded exhaustive population");
    (0..1_usize << atoms.len())
        .filter(|&bits| {
            check(
                theory,
                &interpretation(theory, bits),
                Limits::default(),
                &Cancellation::default(),
            )
            .unwrap()
            .accepted()
        })
        .map(|bits| {
            atoms
                .iter()
                .enumerate()
                .filter(|&(atom, _)| bits & (1 << atom) != 0)
                .map(|(_, atom)| atom.clone())
                .collect()
        })
        .collect()
}

/// The sets of atoms `sets` lists, each in any order: what [`answer_sets`]
/// returns for a theory whose answer sets are exactly those.
pub(crate) fn atom_sets<const N: usize>(sets: [&[Atom]; N]) -> BTreeSet<BTreeSet<Atom>> {
    sets.into_iter()
        .map(|set| set.iter().cloned().collect())
        .collect()
}

/// The text of each statement of `source` that `warnings` reports a zero
/// divisor for, in the warnings' order. Each warning has one entry, so a
/// comparison of the result states how many warnings there are as well as the
/// statements they name.
pub(crate) fn zero_divisor_statements<'source>(
    source: &'source str,
    warnings: &[crate::FormulaWarning],
) -> Vec<&'source str> {
    warnings
        .iter()
        .map(|warning| {
            let crate::FormulaWarning::ZeroDivisor { location } = warning;
            statement(source, *location)
        })
        .collect()
}

/// The text of the statement of `source` at which `failure` and `reference`
/// both refuse for an arithmetic overflow. Each must be that refusal, and the
/// two must name the same site.
pub(crate) fn overflow_statement<'source>(
    source: &'source str,
    failure: &crate::FormulaFailure,
    reference: &crate::FormulaFailure,
) -> &'source str {
    let site = |failure: &crate::FormulaFailure| {
        let crate::FormulaFailure::Expansion(crate::ExpansionFailure::Evaluation {
            error: crate::observation::EvaluationError::Overflow,
            location,
        }) = failure.cause()
        else {
            panic!("{source}: not an arithmetic overflow: {failure:?}")
        };
        *location
    };
    assert_eq!(site(failure), site(reference), "{source}");
    statement(source, site(failure))
}

/// The text of the statement of `source` that `site` names.
fn statement(source: &str, site: crate::ProgramSite) -> &str {
    let offset = |at: ByteOffset| usize::try_from(at.get()).unwrap();
    let span = site
        .location()
        .expect("a statement of a parsed source")
        .span;
    &source[offset(span.start())..offset(span.end())]
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

/// Source preparation and occurrence-raising edge cases, including refusals.
pub(crate) const PREPARATION_CASES: &[&str] = &[
    "a.",
    "#const n=2. p(1..n).",
    "{p}.",
    "1{p;q}1.",
    "p|q.",
    "p(1). q(X) :- p(X), X+1=2.",
    "#show p/0. #show p/0. p.",
    "#show p(X) : p(X). p(1).",
    "#project p/0. {p}.",
    ":~ p. [1@0] {p}.",
    "p(X) :- not q(X).",
    "#const a=b. #const b=a. p(a).",
    "p(1/0).",
    "p(2147483648).",
    "#program step(t). p(t).",
];
