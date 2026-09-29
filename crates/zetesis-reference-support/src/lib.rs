//! The native reference the zetesis workspace's tests compare with clingo:
//! every answer set of a small program, found by checking each candidate
//! against the reduct.
//!
//! [`admit`] admits a source under the default options, [`formula`] under
//! the default limits as well, and [`exhaustive`] enumerates every subset of
//! its atoms, keeping those the reduct check (`zetesis_ferraris::check`)
//! accepts, each with its objective's costs; atoms are spelled as clingo
//! spells them ([`canonical`]). The crate stands above `zetesis-themelios`,
//! so only the tests that compare a native enumeration compile it. It is not
//! published or installed.

use zetesis_core::{Model, Sign};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{Interpretation, Limits, check};
use zetesis_test_support::records::Records;
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionLimits, FormulaFailure, FormulaLimits,
    admit_formula,
};

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
/// Panics if the admission refuses `source`.
#[must_use]
pub fn formula(source: &str) -> AdmittedFormula {
    admit(source, &FormulaLimits::default()).expect("admitted test formula")
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
}
