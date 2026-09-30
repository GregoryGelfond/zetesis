//! Finite bindings and evaluated heads compose without changing frozen theories.

use std::collections::BTreeSet;

use crate::support::finite_bindings::values;
use crate::support::finite_bindings::{holds, remap};
use zetesis_core::Sign;
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionLimits, FormulaLimits, admit_formula,
};

type Model = BTreeSet<String>;
type Models = BTreeSet<Model>;

struct Case {
    name: &'static str,
    source: &'static str,
    expanded: &'static str,
    atom_count: usize,
    models: &'static [&'static [&'static str]],
}

// These are the six exact sources in validation/upstream/cross-feature.jsonl.
// Each expansion is handwritten: intervals in a choice remain in one group,
// while separate outer bindings produce separate rules. No source rewriting
// helper or production grounding output supplies the reference programs.
const CASES: [Case; 6] = [
    Case {
        name: "bound-disjunction",
        source: "p(X+1);q(X):-0<X,X<3.",
        expanded: "p(2);q(1).p(3);q(2).",
        atom_count: 4,
        models: &[
            &["p(2)", "p(3)"],
            &["p(2)", "q(2)"],
            &["p(3)", "q(1)"],
            &["q(1)", "q(2)"],
        ],
    },
    Case {
        name: "bound-choice",
        source: "{p(X+1)}:-X>0,X<3.",
        expanded: "{p(2)}.{p(3)}.",
        atom_count: 2,
        models: &[&[], &["p(2)"], &["p(2)", "p(3)"], &["p(3)"]],
    },
    Case {
        name: "local-double-range",
        source: "1{p(X+Y):not not Y=1..2}1:-0<X,X<3.",
        expanded: "1{p(2);p(3)}1.1{p(3);p(4)}1.",
        atom_count: 3,
        models: &[&["p(2)", "p(4)"], &["p(3)"]],
    },
    Case {
        name: "tuple-disjunction",
        source: "p(X+1);q(X):-not not (X,2)=(1,2).",
        expanded: "p(2);q(1).",
        atom_count: 2,
        models: &[&["p(2)"], &["q(1)"]],
    },
    Case {
        name: "bound-choice-interval",
        source: "1{p(X..X+1)}1:-X>=1,X<=2.",
        expanded: "1{p(1);p(2)}1.1{p(2);p(3)}1.",
        atom_count: 3,
        models: &[&["p(1)", "p(3)"], &["p(2)"]],
    },
    Case {
        name: "bound-coherence",
        source: "-p(X+1);q(X):-0<X,X<2. p(2).",
        expanded: "-p(2);q(1).p(2).",
        atom_count: 3,
        models: &[&["p(2)", "q(1)"]],
    },
];

fn input(source: &str) -> AdmittedFormula {
    let admitted = admit_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap_or_else(|error| panic!("{source}: {error}"));
    assert_eq!(admitted.source().text(), source);
    admitted
}

// Evaluate topological nodes directly. Freezing replaces every M-false subtree
// with falsum, including compound implications. This does not call the native
// evaluator, reduct mask, candidate generator or minimality checker.

fn atom_text<'a>(atom: impl Into<zetesis_core::catalog::AtomRef<'a>>) -> String {
    let atom = atom.into();
    assert_eq!(atom.values().len(), 1);
    let zetesis_core::ValueNodeRef::Number(number) = atom.values().at(0).unwrap().descriptor()
    else {
        panic!("these fixtures have only unary numeric atoms: {atom:?}");
    };
    let sign = if atom.predicate().sign() == Sign::Negative {
        "-"
    } else {
        ""
    };
    format!("{sign}{}({number})", atom.predicate().name())
}

fn selected(input: &AdmittedFormula, mask: usize) -> Model {
    input
        .atoms()
        .iter()
        .enumerate()
        .filter(|(index, _)| mask & (1 << index) != 0)
        .map(|(_, atom)| atom_text(atom))
        .collect()
}

#[test]
fn compositions_preserve_every_original_world_and_frozen_subset() {
    let mut worlds = 0;
    let mut pairs = 0;
    for case in &CASES {
        let original = input(case.source);
        let expanded = input(case.expanded);
        assert_eq!(original.atoms().len(), case.atom_count, "{}", case.name);
        assert_eq!(
            original.atoms().iter().collect::<BTreeSet<_>>(),
            expanded.atoms().iter().collect::<BTreeSet<_>>(),
            "{}: exact signed atom carrier",
            case.name
        );
        for outer in 0..1_usize << case.atom_count {
            let original_values = values(original.theory(), outer, None);
            let expanded_values = values(
                expanded.theory(),
                remap(outer, original.atoms(), expanded.atoms()),
                None,
            );
            assert_eq!(
                holds(original.theory(), &original_values),
                holds(expanded.theory(), &expanded_values),
                "{}: original truth at M={outer}",
                case.name
            );
            worlds += 1;
            // Include J=M, J=empty and candidates that are not original models.
            let mut inner = outer;
            loop {
                assert_eq!(
                    holds(
                        original.theory(),
                        &values(original.theory(), inner, Some(&original_values))
                    ),
                    holds(
                        expanded.theory(),
                        &values(
                            expanded.theory(),
                            remap(inner, original.atoms(), expanded.atoms()),
                            Some(&expanded_values)
                        )
                    ),
                    "{}: frozen truth at M={outer}, J={inner}",
                    case.name
                );
                pairs += 1;
                if inner == 0 {
                    break;
                }
                inner = (inner - 1) & outer;
            }
        }
    }
    assert_eq!(worlds, 48);
    assert_eq!(pairs, 180);
}

fn exhaustive_models(input: &AdmittedFormula) -> Models {
    assert!(input.atoms().len() <= 4);
    let mut models = Models::new();
    for candidate in 0..1_usize << input.atoms().len() {
        let frozen = values(input.theory(), candidate, None);
        if !holds(input.theory(), &frozen) {
            continue;
        }
        let countermodel = (0..candidate).any(|subset| {
            subset & candidate == subset
                && holds(
                    input.theory(),
                    &values(input.theory(), subset, Some(&frozen)),
                )
        });
        if !countermodel {
            assert!(models.insert(selected(input, candidate)));
        }
    }
    models
}

fn native_models(input: &AdmittedFormula) -> Models {
    let mut search = zetesis_sat::StableModels::new(
        input.theory(),
        zetesis_sat::Limits::default(),
        zetesis_cpu::Cancellation::default(),
    )
    .unwrap();
    let mut models = Models::new();
    for model in search.by_ref() {
        assert!(
            models.insert(
                model
                    .unwrap()
                    .atoms()
                    .map(|index| atom_text(input.atoms().at(index).unwrap()))
                    .collect()
            )
        );
    }
    assert!(search.exhausted());
    models
}

#[test]
fn complete_native_models_match_handwritten_models_and_exhaustive_reduct_checks() {
    let mut count = 0;
    for case in &CASES {
        let expected: Models = case
            .models
            .iter()
            .map(|model| model.iter().map(|atom| (*atom).to_owned()).collect())
            .collect();
        assert_eq!(expected.len(), case.models.len());
        for source in [case.source, case.expanded] {
            let admitted = input(source);
            assert_eq!(
                exhaustive_models(&admitted),
                expected,
                "{}: finite reduct models of {source}",
                case.name
            );
            assert_eq!(
                native_models(&admitted),
                expected,
                "{}: complete native models of {source}",
                case.name
            );
        }
        count += expected.len();
    }
    assert_eq!(count, 15);
}
