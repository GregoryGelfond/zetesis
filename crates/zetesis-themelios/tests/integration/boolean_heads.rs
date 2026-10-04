//! Boolean heads retain their original truth without supplying atom support.

use crate::support::objective_dependency_records as objective_dependencies;

mod cases;
mod comparisons;
mod elements;
use crate::support::finite_bindings as reference;
use crate::support::finite_bindings::expected;

use std::collections::BTreeSet;

use cases::CASES;
use proptest::prelude::*;
use reference::{Models, exhaustive, external, holds, native, values};
use zetesis_reference_support::{admit, canonical, formula};
use zetesis_themelios::{
    AdmissionOptions, BundleAdmissionOptions, BundleLimits, ExpansionFailure, ExpansionLimits,
    ExpansionResource, FormulaFailure, FormulaLimits, FormulaResource, SourceBundle,
    admit_bundle_formula, admit_formula,
};

#[test]
fn complete_models_match_original_contracts() {
    for &(source, records) in CASES {
        assert_eq!(native(&formula(source)), expected(records), "{source}");
    }
}

#[test]
fn stability_matches_independent_subset_enumeration() {
    for &(source, _) in CASES {
        let admitted = formula(source);
        assert_eq!(native(&admitted), exhaustive(&admitted), "{source}");
    }
}

#[test]
fn original_source_text_is_retained() {
    for &(source, _) in CASES {
        assert_eq!(
            formula(source).source().expect("source input").text(),
            source
        );
    }
}

#[test]
fn boolean_constants_introduce_no_atoms() {
    for source in [
        "#true.",
        "#false.",
        "not #true.",
        "not #false.",
        "not not #true.",
        "not not #false.",
    ] {
        assert!(formula(source).atoms().is_empty(), "{source}");
    }
}

#[test]
#[ignore = "requires clingo: original sources match clingo full models"]
fn original_sources_match_clingo_full_models() {
    let mut models = 0;
    for &(source, records) in CASES {
        let result = external(source, true);
        assert_eq!(result["Models"]["More"], "no");
        let mut actual = Models::new();
        let mut count = 0;
        for call in result["Call"].as_array().unwrap() {
            if let Some(witnesses) = call["Witnesses"].as_array() {
                for witness in witnesses {
                    assert!(witness["Costs"].is_null());
                    count += 1;
                    assert!(
                        actual.insert(
                            witness["Value"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .map(|value| value.as_str().unwrap().to_owned())
                                .collect()
                        )
                    );
                }
            }
        }
        assert_eq!(result["Models"]["Number"].as_u64(), Some(count));
        assert_eq!(actual, expected(records), "{source}");
        assert_eq!(native(&formula(source)), actual, "{source}");
        models += count;
    }
    println!("complete_sources={} full_models={models}", CASES.len());
}

/// A separate finite formula tree evaluates every subtree in the original M
/// before testing its reduct in J. It shares no lowering or truth machinery.
#[derive(Clone)]
enum Formula {
    Boolean(bool),
    Atom(usize),
    Or(Box<Self>, Box<Self>),
    Implies(Box<Self>, Box<Self>),
}

impl Formula {
    fn truth(&self, tested: usize, frozen: Option<usize>) -> bool {
        if frozen.is_some_and(|outer| !self.truth(outer, None)) {
            return false;
        }
        match self {
            Self::Boolean(value) => *value,
            Self::Atom(atom) => tested & (1 << atom) != 0,
            Self::Or(left, right) => left.truth(tested, frozen) || right.truth(tested, frozen),
            Self::Implies(left, right) => {
                !left.truth(tested, frozen) || right.truth(tested, frozen)
            }
        }
    }

    fn negated(self) -> Self {
        Self::Implies(Box::new(self), Box::new(Self::Boolean(false)))
    }

    fn or(self, other: Self) -> Self {
        Self::Or(Box::new(self), Box::new(other))
    }
}

const ATOMS: [&str; 3] = ["a", "b", "c"];

fn literal(operand: u8, negations: u8) -> (String, Formula) {
    let (text, mut formula) = match operand {
        0 => ("#false", Formula::Boolean(false)),
        1 => ("#true", Formula::Boolean(true)),
        atom => {
            let index = usize::from(atom - 2);
            (ATOMS[index], Formula::Atom(index))
        }
    };
    for _ in 0..negations {
        formula = formula.negated();
    }
    (
        format!("{}{text}", "not ".repeat(usize::from(negations))),
        formula,
    )
}

fn original_rules(heads: &[(u8, u8)], body: (u8, u8)) -> (String, Vec<Formula>) {
    // Independent choices ensure all three named atoms have producer evidence,
    // so candidate-only support guards are true in every original M.
    let mut theory: Vec<_> = (0..ATOMS.len())
        .map(|atom| Formula::Atom(atom).or(Formula::Atom(atom).negated()))
        .collect();
    let mut sources = Vec::new();
    let mut head = Formula::Boolean(false);
    for &(operand, negations) in heads {
        let (source, formula) = literal(operand, negations);
        sources.push(source);
        head = head.or(formula);
    }
    let (source_body, body) = literal(body.0, body.1);
    theory.push(Formula::Implies(Box::new(body), Box::new(head)));
    (
        format!("{{a;b;c}}.{}:-{source_body}.", sources.join("|")),
        theory,
    )
}

fn frozen_pairs(source: &str, theory: &[Formula]) {
    let admitted = formula(source);
    let names: Vec<_> = admitted.atoms().iter().map(canonical).collect();
    assert_eq!(
        names.iter().map(String::as_str).collect::<BTreeSet<_>>(),
        ATOMS.into()
    );
    let remap = |mask: usize| {
        ATOMS.iter().enumerate().fold(0, |result, (index, name)| {
            result
                | (usize::from(mask & (1 << index) != 0)
                    << names.iter().position(|value| value == name).unwrap())
        })
    };
    for outer in 0..1 << ATOMS.len() {
        let original = values(admitted.theory(), remap(outer), None);
        assert_eq!(
            holds(admitted.theory(), &original),
            theory.iter().all(|f| f.truth(outer, None)),
            "{source}: M={outer}"
        );
        for inner in 0..1 << ATOMS.len() {
            assert_eq!(
                holds(
                    admitted.theory(),
                    &values(admitted.theory(), remap(inner), Some(&original))
                ),
                theory.iter().all(|f| f.truth(inner, Some(outer))),
                "{source}: M={outer} J={inner}"
            );
        }
    }
}

#[test]
fn signed_constants_preserve_every_frozen_world() {
    let mut cases = 0;
    for boolean in 0..2 {
        for sign in 0..3 {
            for body in 0..5 {
                for body_sign in 0..3 {
                    let (source, theory) =
                        original_rules(&[(boolean, sign), (2, 0)], (body, body_sign));
                    frozen_pairs(&source, &theory);
                    cases += 1;
                }
            }
        }
    }
    println!("manual_programs={cases} frozen_pairs={}", cases * 64);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]
    #[test]
    fn generated_heads_preserve_every_frozen_world(
        heads in prop::collection::vec((0_u8..5, 0_u8..3), 1..5),
        body in (0_u8..5, 0_u8..3),
    ) {
        let (source, theory) = original_rules(&heads, body);
        frozen_pairs(&source, &theory);
    }
}

#[test]
fn tautologies_preserve_variable_safety() {
    for source in ["#true|p(X).", "#true:-not p(X).", "#true|p(2..1,X)."] {
        let error = admit(source, &FormulaLimits::default()).unwrap_err();
        assert!(!error.diagnostics().is_empty(), "{source}");
        assert!(
            matches!(error, FormulaFailure::UnsafeVariable { .. }),
            "{source}: {error}"
        );
    }
}

#[test]
fn conditional_booleans_preserve_eligibility() {
    for (source, records) in [
        ("#true:a;b.", vec![vec!["b"]]),
        ("#true:#false;b.", vec![vec!["b"]]),
        ("#true:1=1;b.", vec![vec![]]),
    ] {
        let expected: Models = records
            .into_iter()
            .map(|row| row.into_iter().map(str::to_owned).collect())
            .collect();
        assert_eq!(native(&formula(source)), expected, "{source}");
    }
}

#[test]
fn boolean_disjuncts_preserve_scored_answers() {
    for source in [
        "#true|a.#minimize{1@7:a}.",
        "#false|a.#minimize{1@7:a}.",
        "#true|not a.#minimize{1@7:a}.",
    ] {
        objective_dependencies::check(source);
    }
}

#[test]
fn head_element_limits_count_boolean_operands() {
    for (source, count) in [("#true.", 1), ("#true|a.", 2), ("not #false|a|b.", 3)] {
        let mut limits = FormulaLimits {
            max_disjunction_elements: count,
            ..FormulaLimits::default()
        };
        assert!(admit(source, &limits).is_ok());
        limits.max_disjunction_elements = count - 1;
        assert!(
            matches!(admit(source, &limits), Err(FormulaFailure::Limit { resource: FormulaResource::DisjunctionElements, observed, .. }) if observed == count as u128)
        );
    }
}

#[test]
fn tautologies_preserve_arithmetic_refusals() {
    for source in ["#true|p(1/0).", "#true|p(2147483647+1)."] {
        let error = admit(source, &FormulaLimits::default()).unwrap_err();
        assert!(!error.diagnostics().is_empty(), "{source}");
        assert!(
            matches!(
                error,
                FormulaFailure::Expansion(ExpansionFailure::Evaluation { .. })
            ),
            "{source}: {error}"
        );
    }
}

const BUDGET_SOURCE: &str = "d(1..2).#true|p(X+1):-d(X).";

fn first_success(mut attempt: impl FnMut(u64) -> bool) -> u64 {
    let mut high = 1;
    while !attempt(high) {
        high *= 2;
        assert!(high <= 1_048_576, "qualification threshold exceeded");
    }
    let mut low = 0;
    while low + 1 < high {
        let middle = low + (high - low) / 2;
        if attempt(middle) {
            high = middle;
        } else {
            low = middle;
        }
    }
    high
}

fn exact_formula_budget(resource: FormulaResource, configure: impl Fn(&mut FormulaLimits, u64)) {
    let attempt = |limit| {
        let mut limits = FormulaLimits::default();
        configure(&mut limits, limit);
        admit(BUDGET_SOURCE, &limits)
    };
    let threshold = first_success(|limit| attempt(limit).is_ok());
    assert_eq!(
        native(&attempt(threshold).unwrap()),
        native(&formula(BUDGET_SOURCE))
    );
    let error = attempt(threshold - 1).unwrap_err();
    assert!(!error.diagnostics().is_empty());
    assert!(
        matches!(error, FormulaFailure::Limit { resource: actual, observed, .. }
        if actual == resource && observed == u128::from(threshold)),
        "{resource:?}: {error}"
    );
    println!("resource={resource:?} inclusive_threshold={threshold}");
}

#[test]
fn tautologies_preserve_work_accounting() {
    exact_formula_budget(FormulaResource::Work, |limits, value| {
        limits.max_work = value;
    });
}

#[test]
fn tautologies_preserve_substitution_accounting() {
    exact_formula_budget(FormulaResource::Substitutions, |limits, value| {
        limits.max_substitutions = value;
    });
}

#[test]
fn tautologies_preserve_node_accounting() {
    exact_formula_budget(FormulaResource::Nodes, |limits, value| {
        limits.theory.max_nodes = usize::try_from(value).unwrap();
    });
}

#[test]
fn tautologies_preserve_atom_accounting() {
    exact_formula_budget(FormulaResource::Atoms, |limits, value| {
        limits.theory.max_atoms = usize::try_from(value).unwrap();
    });
}

#[test]
fn tautologies_preserve_term_work_accounting() {
    let attempt = |limit| {
        admit_formula(
            BUDGET_SOURCE.into(),
            AdmissionOptions::default(),
            ExpansionLimits {
                max_term_work: usize::try_from(limit).unwrap(),
                ..ExpansionLimits::default()
            },
            FormulaLimits::default(),
        )
    };
    let threshold = first_success(|limit| attempt(limit).is_ok());
    assert_eq!(
        native(&attempt(threshold).unwrap()),
        native(&formula(BUDGET_SOURCE))
    );
    let error = attempt(threshold - 1).unwrap_err();
    assert!(!error.diagnostics().is_empty());
    assert!(matches!(
        error,
        FormulaFailure::Expansion(ExpansionFailure::Limit {
            resource: ExpansionResource::TermWork,
            ..
        })
    ));
    println!("resource=TermWork inclusive_threshold={threshold}");
}

#[test]
fn duplicate_boolean_heads_retain_original_file_identity() {
    let directory = tempfile::tempdir().unwrap();
    let rule = "#true|p(1..2).";
    std::fs::write(
        directory.path().join("entry.lp"),
        format!("#include \"other.lp\".\n{rule}"),
    )
    .unwrap();
    std::fs::write(directory.path().join("other.lp"), rule).unwrap();
    let bundle =
        SourceBundle::load(directory.path().join("entry.lp"), BundleLimits::default()).unwrap();
    let admitted = admit_bundle_formula(
        bundle,
        BundleAdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    assert!(!admitted.formula_origins().is_empty());
    for origins in admitted.formula_origins() {
        assert_eq!(
            origins
                .iter()
                .map(|origin| origin.location().expect("parsed source").source)
                .collect::<BTreeSet<_>>()
                .len(),
            2
        );
        for origin in origins {
            assert_eq!(
                admitted
                    .bundle()
                    .get(origin.location().expect("parsed source").source)
                    .unwrap()
                    .source()
                    .slice(origin.location().expect("parsed source").span)
                    .unwrap(),
                rule
            );
        }
    }
}

#[test]
#[ignore = "requires clingo: tautological rules remain unsafe in clingo"]
fn tautological_rules_remain_unsafe_in_clingo() {
    for source in ["#true|p(X).", "#true:-not p(X).", "#true|p(2..1,X)."] {
        let result = external(source, false);
        assert_eq!(result["Result"], "UNKNOWN");
    }
}
