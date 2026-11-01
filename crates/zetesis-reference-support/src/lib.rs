//! Test support over `zetesis-themelios`: admission of a source under default
//! or given limits, the native reference the zetesis workspace's tests compare
//! with clingo (every answer set of a small program, found by checking each
//! candidate against the reduct), preparation and hybrid grounding,
//! source-bundle fixtures, and, by the resource a refusal reports, the limits
//! an admission takes and those of three other operations whose tests span
//! crates ([`resource_limits`] states which).
//!
//! [`admit`] admits a source under given formula limits, the rest at their
//! defaults, and [`formula`] under the default limits throughout. [`exhaustive`]
//! enumerates every subset of an
//! admitted source's atoms, keeping those the reduct check
//! (`zetesis_ferraris::check`) accepts, each with its objective's costs; atoms
//! are spelled as clingo spells them ([`canonical`]). [`exhaustive_models`] and
//! [`exhaustive_bundle_models`] retain typed atoms from the same subset reference.
//! [`prepare`] prepares a
//! source under the default options and limits, [`prepared`] when preparation
//! must accept it, and [`hybrid`] grounds it for hybrid execution;
//! [`BundleFixture`] writes a source bundle into a temporary directory for the
//! tests that load or run one. [`resource_limits`] sets those limits by the
//! resource a refusal reports, and reads the formula and expansion limits'
//! defaults by resource. The crate stands above `zetesis-themelios`, so only
//! the tests that need these references compile it. It is not published or
//! installed.

use zetesis_algebra::Cancellation;
use zetesis_core::{Model, Sign};
use zetesis_ferraris::{Interpretation, Limits, check};
use zetesis_test_support::records::Records;
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, AdmittedFormulaBundle, ExpansionLimits, FormulaFailure,
    FormulaLimits, HybridFormula, PreparedFormula, admit_formula, prepare_formula,
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
    let cancellation = Cancellation::default();
    let mut records = Records::new();
    for model in exhaustive_models(input) {
        let evaluation = zetesis_objective::evaluate(
            input.objectives(),
            &model,
            zetesis_objective::Limits::default(),
            &cancellation,
        )
        .unwrap();
        let score = evaluation.score();
        let costs = score
            .is_present()
            .then(|| score.costs().iter().map(|&(_, value)| value).collect());
        assert!(records.insert((model.atoms().iter().map(canonical).collect(), costs)));
    }
    records
}

/// Every answer set of `input`, retaining its catalog and typed atom values.
///
/// Enumerates every subset independently of the production candidate walk,
/// accepting exactly the subsets admitted by the Ferraris reduct check. This
/// shares the membership checker, not an independent membership oracle.
///
/// # Panics
/// Panics if `input` has more than twelve atoms, a membership check refuses,
/// or an accepted interpretation cannot be read from the original catalog.
#[must_use]
pub fn exhaustive_models(input: &AdmittedFormula) -> Vec<Model> {
    models(input.theory(), input.atom_catalog())
}

/// Every answer set of a formula bundle, preserving its typed atom catalog.
///
/// Uses the same exhaustive subset reference as [`exhaustive_models`].
///
/// # Panics
/// Panics under the same bounds and membership failures as [`exhaustive_models`].
#[must_use]
pub fn exhaustive_bundle_models(input: &AdmittedFormulaBundle) -> Vec<Model> {
    models(input.theory(), input.atom_catalog())
}

fn models(theory: &zetesis_ferraris::Theory, catalog: &zetesis_core::AtomCatalog) -> Vec<Model> {
    let count = catalog.atoms().len();
    assert!(count <= 12, "small independent subset enumeration");
    let cancellation = Cancellation::default();
    let mut models = Vec::new();
    for bits in 0..(1_usize << count) {
        let candidate =
            Interpretation::new(theory, (0..count).filter(|&atom| bits & (1 << atom) != 0))
                .unwrap();
        if check(theory, &candidate, Limits::default(), &cancellation)
            .unwrap()
            .accepted()
        {
            models.push(Model::from_positions(catalog, candidate.atoms()).unwrap());
        }
    }
    models
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
    fn normal_rules_preserve_their_complete_answer_family() {
        let admitted = formula("a :- not b. b :- not a. c :- a.");
        let mut answer_sets: Vec<Vec<String>> = exhaustive_models(&admitted)
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
