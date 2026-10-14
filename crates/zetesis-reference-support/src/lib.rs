//! Test support over `zetesis-themelios`: admission of a source under default
//! or given limits, the native reference the zetesis workspace's tests compare
//! with clingo (every answer set of a small program, found by checking each
//! candidate against the reduct), preparation and hybrid grounding,
//! source-bundle fixtures, and, by the resource a refusal reports, the limits
//! an admission takes ([`resource_limits`] states which).
//!
//! [`admit`] admits a source under given formula limits, the rest at their
//! defaults, [`formula`] under the default limits throughout and [`normal`] on
//! the normal-program route. [`exhaustive`] enumerates every subset of an
//! admitted source's atoms, keeping those the reduct check
//! (`zetesis_ferraris::check`) accepts, each with its objective's costs; atoms
//! are spelled as clingo spells them ([`canonical`]). [`normal_answer_sets`]
//! finds a normal program's answer sets by exhausting its candidates, each
//! checked against the reduct (`zetesis_cpu::check`). [`prepare`] prepares a
//! source under the default options and limits, [`prepared`] when preparation
//! must accept it, and [`hybrid`] grounds it for hybrid execution;
//! [`BundleFixture`] writes a source bundle into a temporary directory for the
//! tests that load or run one. [`resource_limits`] sets those limits by the
//! resource a refusal reports, and reads the formula and expansion limits'
//! defaults by resource. The crate stands above `zetesis-themelios`, so only
//! the tests that need that closure compile it. It is not published or
//! installed.

use zetesis_core::{Model, Program, Sign};
use zetesis_cpu::{Cancellation, CandidateLimits, CandidateTermination, Candidates};
use zetesis_ferraris::{Interpretation, Limits, check};
use zetesis_test_support::records::Records;
use zetesis_themelios::{
    AdmissionOptions, Admitted, AdmittedFormula, ExpansionLimits, FormulaFailure, FormulaLimits,
    HybridFormula, PreparedFormula, admit_extended, admit_formula, prepare_formula,
};

mod bundle;
pub use bundle::BundleFixture;
pub mod resource_limits;

/// An atom as clingo spells it: a leading `-` for strong negation, and
/// arguments in clingo's notation, strings quoted.
///
/// # Panics
/// Panics if a string argument cannot be quoted as JSON, which never happens.
#[must_use]
pub fn canonical<'a>(atom: impl Into<zetesis_core::catalog::AtomRef<'a>>) -> String {
    let atom = atom.into();
    let sign = if atom.predicate().sign() == Sign::Negative {
        "-"
    } else {
        ""
    };
    let name = format!("{sign}{}", atom.predicate().name());
    let arguments: Vec<_> = atom
        .values()
        .iter()
        .map(|value| match value.descriptor() {
            zetesis_core::ValueNodeRef::Infimum => "#inf".into(),
            zetesis_core::ValueNodeRef::Supremum => "#sup".into(),
            zetesis_core::ValueNodeRef::Function { .. }
            | zetesis_core::ValueNodeRef::Tuple { .. } => value.to_string(),
            zetesis_core::ValueNodeRef::Number(value) => value.to_string(),
            zetesis_core::ValueNodeRef::Symbol(value) => value.to_owned(),
            zetesis_core::ValueNodeRef::String(value) => serde_json::to_string(value).unwrap(),
        })
        .collect();
    if arguments.is_empty() {
        name
    } else {
        format!("{name}({})", arguments.join(","))
    }
}

/// `source` admitted as a formula program under the default options and
/// expansion limits, and `limits`.
///
/// # Errors
/// Returns the admission's refusal.
pub fn admit(source: &str, limits: &FormulaLimits) -> Result<AdmittedFormula, FormulaFailure> {
    admit_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        *limits,
    )
}

/// `source` admitted as a formula program under the default options and
/// limits.
///
/// # Panics
/// Panics, naming `source` and the refusal, if the admission refuses it.
#[must_use]
pub fn formula(source: &str) -> AdmittedFormula {
    admit(source, &FormulaLimits::default()).unwrap_or_else(|error| panic!("{source}: {error}"))
}

/// `source` admitted on the normal-program route, the bounded extension of
/// S0, under the default options and limits.
///
/// # Panics
/// Panics if the admission refuses `source`.
#[must_use]
pub fn normal(source: &str) -> Admitted {
    admit_extended(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
    )
    .unwrap()
}

/// `source` prepared under the default options and limits.
///
/// # Errors
/// Returns the preparation's refusal.
pub fn prepare(source: &str) -> Result<PreparedFormula, FormulaFailure> {
    prepare_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
}

/// `source` prepared under the default options and limits.
///
/// # Panics
/// Panics, naming `source` and the refusal, if preparation refuses it.
#[must_use]
pub fn prepared(source: &str) -> PreparedFormula {
    prepare(source).unwrap_or_else(|error| panic!("{source}: {error}"))
}

/// `source` prepared under the default options and limits and grounded for
/// hybrid execution.
///
/// # Panics
/// Panics if preparation refuses `source` or hybrid grounding refuses it.
#[must_use]
pub fn hybrid(source: &str) -> HybridFormula {
    prepared(source)
        .ground_hybrid()
        .unwrap_or_else(|error| panic!("{source}: {error}"))
}

/// Every answer set of `input`, each with its objective's costs: every subset
/// of its atoms the reduct check accepts.
///
/// # Panics
/// Panics if `input` has more than twelve atoms, since the enumeration visits
/// every subset, or if a candidate's check or evaluation is refused.
#[must_use]
pub fn exhaustive(input: &AdmittedFormula) -> Records {
    let count = input.atoms().len();
    assert!(count <= 12, "small independent subset enumeration");
    let cancellation = Cancellation::default();
    let mut records = Records::new();
    for bits in 0..(1_usize << count) {
        let candidate = Interpretation::new(
            input.theory(),
            (0..count).filter(|&atom| bits & (1 << atom) != 0),
        )
        .unwrap();
        if !check(input.theory(), &candidate, Limits::default(), &cancellation)
            .unwrap()
            .accepted()
        {
            continue;
        }
        let atoms: Vec<_> = candidate
            .atoms()
            .map(|atom| input.atoms().at(atom).unwrap())
            .collect();
        let evaluated_model =
            Model::from_positions(input.atom_catalog(), candidate.atoms()).unwrap();
        let evaluation = zetesis_objective::evaluate(
            input.objectives(),
            &evaluated_model,
            zetesis_objective::Limits::default(),
            &cancellation,
        )
        .unwrap();
        let score = evaluation.score();
        let costs = score
            .is_present()
            .then(|| score.costs().iter().map(|&(_, value)| value).collect());
        assert!(records.insert((atoms.iter().copied().map(canonical).collect(), costs)));
    }
    records
}

/// Every answer set of the normal program `program`: the closure of each
/// candidate the reduct check (`zetesis_cpu::check`) accepts, the candidates
/// exhausted under the default limits.
///
/// # Panics
/// Panics if a candidate or its check is refused, or if the candidates stop
/// before they are exhausted.
#[must_use]
pub fn normal_answer_sets(program: &Program) -> Vec<Model> {
    let cancellation = Cancellation::default();
    let mut candidates = Candidates::new(program, CandidateLimits::default(), cancellation.clone());
    let answer_sets = candidates
        .by_ref()
        .filter_map(|seed| {
            let checked = zetesis_cpu::check(
                program,
                &seed.expect("the default candidate limits"),
                zetesis_cpu::Limits::default(),
                &cancellation,
            )
            .expect("the default reduct-check limits");
            checked.accepted().then(|| checked.closure().clone())
        })
        .collect();
    assert_eq!(
        candidates.termination(),
        Some(CandidateTermination::Exhausted)
    );
    answer_sets
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_choice_has_the_empty_and_the_full_answer_set() {
        let admitted = admit("{a}.", &FormulaLimits::default()).unwrap();
        let expected: Records = [([].into(), None), (["a".to_owned()].into(), None)].into();
        assert_eq!(exhaustive(&admitted), expected);
    }

    #[test]
    fn a_formula_is_its_source_admitted_under_the_default_limits() {
        let admitted = admit("{a}. b :- a.", &FormulaLimits::default()).unwrap();
        assert_eq!(exhaustive(&formula("{a}. b :- a.")), exhaustive(&admitted));
    }

    #[test]
    fn an_objective_gives_every_answer_set_its_costs() {
        let admitted = admit("{a}. #minimize{1:a}.", &FormulaLimits::default()).unwrap();
        let expected: Records = [
            ([].into(), Some(vec![0])),
            (["a".to_owned()].into(), Some(vec![1])),
        ]
        .into();
        assert_eq!(exhaustive(&admitted), expected);
    }

    #[test]
    fn a_hybrid_grounding_keeps_the_prepared_atoms() {
        let grounded = hybrid("{a}. b :- a.");
        assert_eq!(grounded.atom_catalog().atoms().len(), 2);
    }

    #[test]
    #[should_panic(expected = "q(1/0).")]
    fn a_refused_preparation_names_its_source() {
        let _ = prepared("q(1/0).");
    }

    #[test]
    fn a_normal_program_has_the_answer_sets_its_candidates_check_to() {
        let admitted = normal("a :- not b. b :- not a. c :- a.");
        let mut answer_sets: Vec<Vec<String>> = normal_answer_sets(admitted.program())
            .iter()
            .map(|model| model.atoms().iter().map(canonical).collect())
            .collect();
        for answer_set in &mut answer_sets {
            answer_set.sort();
        }
        answer_sets.sort();
        assert_eq!(answer_sets, [vec!["a", "c"], vec!["b"]]);
    }
}
