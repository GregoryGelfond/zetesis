//! Complete formula and frozen-reduct comparison against explicit Cartesian rules.

#[path = "support/stable_models.rs"]
mod stable_models;

use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

use stable_models::stable;
use themelios_base::span::Location;
use zetesis_core::{Atom, Predicate, Value};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{Interpretation, Limits, models, models_reduct};
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionLimits, FormulaLimits, FormulaResource,
    GroundingObserver, GroundingOutcome, GroundingPhase, GroundingWork, admit_formula,
    admit_formula_with_grounding_observer,
};

fn input(source: &str) -> AdmittedFormula {
    admit_formula(
        source.to_owned(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap_or_else(|error| panic!("{source}: {error}"))
}

fn interpretation(
    input: &AdmittedFormula,
    indices: &BTreeMap<zetesis_core::catalog::AtomRef<'_>, usize>,
    mask: usize,
) -> Interpretation {
    Interpretation::new(
        input.theory(),
        input
            .atoms()
            .iter()
            .enumerate()
            .filter_map(|(index, atom)| (mask & (1 << indices[&atom]) != 0).then_some(index)),
    )
    .expect("same-theory atom indices")
}

fn equivalent(left: &str, right: &str) {
    let left = input(left);
    let right = input(right);
    let universe: BTreeSet<_> = left.atoms().iter().collect();
    assert_eq!(universe, right.atoms().iter().collect());
    assert!(universe.len() <= 6);
    let indices = universe
        .into_iter()
        .enumerate()
        .map(|(index, atom)| (atom, index))
        .collect();
    let cancellation = Cancellation::default();
    for candidate in 0..1 << left.atoms().len() {
        let lm = interpretation(&left, &indices, candidate);
        let rm = interpretation(&right, &indices, candidate);
        assert_eq!(
            models(left.theory(), &lm, Limits::default(), &cancellation).unwrap(),
            models(right.theory(), &rm, Limits::default(), &cancellation).unwrap()
        );
        for tested in 0..1 << left.atoms().len() {
            let lj = interpretation(&left, &indices, tested);
            let rj = interpretation(&right, &indices, tested);
            assert_eq!(
                models_reduct(left.theory(), &lm, &lj, Limits::default(), &cancellation).unwrap(),
                models_reduct(right.theory(), &rm, &rj, Limits::default(), &cancellation).unwrap(),
                "M={candidate}, J={tested}"
            );
        }
    }
}

#[test]
fn independent_components_preserve_every_frozen_candidate_and_tested_interpretation() {
    let choices = "{a(1);a(2);b(1);b(2)}.";
    for (extra, local) in [("", ""), (",not h", ",not h"), (",not not h", ",not not h")] {
        let lifted = format!("{choices} h:-a(X),b(Y){local}.");
        let mut expanded = choices.to_owned();
        for x in 1..=2 {
            for y in 1..=2 {
                write!(expanded, "h:-a({x}),b({y}){extra}.").unwrap();
            }
        }
        equivalent(&lifted, &expanded);
    }
    let mut expanded = choices.to_owned();
    for x in 1..=2 {
        for y in 1..=2 {
            write!(expanded, "h:-a({x}),not b({x}),b({y}),not a({y}).").unwrap();
        }
    }
    equivalent(
        &format!("{choices} h:-a(X),not b(X),b(Y),not a(Y)."),
        &expanded,
    );
    equivalent(
        &format!("{choices} h:-a(X),b(Y),(X,1)!=(2,1),Y!=2."),
        &format!("{choices} h:-a(1),b(1)."),
    );
}

#[test]
fn independent_constraint_components_and_empty_extensions_are_exact() {
    equivalent(
        "{a(1);a(2);b(1);b(2)}. :-a(X),b(Y).",
        "{a(1);a(2);b(1);b(2)}. :-a(1),b(1). :-a(1),b(2). :-a(2),b(1). :-a(2),b(2).",
    );
    equivalent(
        "{a(1);a(2);b(1);b(2)}. h:-a(X),X!=1,X!=2,b(Y).",
        "{a(1);a(2);b(1);b(2)}.",
    );
}

fn atom(name: &str, values: Vec<Value>) -> Atom {
    Atom::new(Predicate::new(name, values.len()).unwrap(), values).unwrap()
}

#[test]
fn factored_projections_keep_constants_and_independent_bindings_in_each_component() {
    // Every blocker is independently optional. Only the first and fourth match
    // their component; treating a copied constant as a wildcard changes models.
    for (left_text, right_text, left, right) in [
        (
            "k",
            "other",
            Value::Symbol("k".into()),
            Value::Symbol("other".into()),
        ),
        (
            "\"k k\"",
            "\"other\"",
            Value::String("k k".into()),
            Value::String("other".into()),
        ),
    ] {
        let blockers = [
            atom(
                "blocked",
                vec![Value::Number(1), left.clone(), Value::Number(9)],
            ),
            atom(
                "blocked",
                vec![Value::Number(1), right.clone(), Value::Number(9)],
            ),
            atom("blocked", vec![Value::Number(2), left, Value::Number(9)]),
            atom("blocked", vec![Value::Number(2), right, Value::Number(9)]),
        ];
        for left_present in [false, true] {
            for right_present in [false, true] {
                let negation = |present| if present { "not not" } else { "not" };
                let source = format!(
                    "left(1). right(2). \
                     {{blocked(1,{left_text},9);blocked(1,{right_text},9);\
                     blocked(2,{left_text},9);blocked(2,{right_text},9)}}. \
                     h :- left(X), {} blocked(X,{left_text},_), \
                     right(Y), {} blocked(Y,{right_text},_).",
                    negation(left_present),
                    negation(right_present)
                );
                let expected: BTreeSet<_> = (0_u8..16)
                    .map(|mask| {
                        let mut model = BTreeSet::from([
                            atom("left", vec![Value::Number(1)]),
                            atom("right", vec![Value::Number(2)]),
                        ]);
                        for (index, blocker) in blockers.iter().enumerate() {
                            if mask & (1 << index) != 0 {
                                model.insert(blocker.clone());
                            }
                        }
                        if (mask & 1 != 0) == left_present && (mask & 8 != 0) == right_present {
                            model.insert(atom("h", vec![]));
                        }
                        model
                    })
                    .collect();
                assert_eq!(expected.len(), 16, "independent optional blockers");
                assert_eq!(stable(&input(&source)), expected, "{source}");
            }
        }
    }
}

#[test]
fn head_constants_repetition_other_producers_and_negative_projection_remain_exact() {
    let lifted = input("d(1;2). e(1;2). h(3,3). {p(1,0)}. h(X,X):-d(X),e(Y),not p(X,_).");
    let expanded = input(
        "d(1;2). e(1;2). h(3,3). {p(1,0)}. h(1,1):-d(1),e(1),not p(1,_). h(1,1):-d(1),e(2),not p(1,_). h(2,2):-d(2),e(1),not p(2,_). h(2,2):-d(2),e(2),not p(2,_).",
    );
    assert_eq!(stable(&lifted), stable(&expanded));
}

#[test]
fn a_false_component_filter_excludes_the_substitution() {
    let source = "a(0).b(1).p:-a(X),1/X>0,b(Y),Y=2.";
    // The positive join has the one substitution X=0,Y=1. Y=2 is defined and
    // false over it, so the substitution is excluded and X's undefined
    // division is never reached, whichever component the join reads first.
    // The family is {a(0),b(1)}, as the external reference records.
    let expected = BTreeSet::from([BTreeSet::from([
        atom("a", vec![Value::Number(0)]),
        atom("b", vec![Value::Number(1)]),
    ])]);
    assert_eq!(stable(&input(source)), expected);
}

const CARTESIAN_SOURCE: &str = "a(1..40). b(1..40). h:-a(X),b(Y).";

#[derive(Default)]
struct InstantiationRows(Cell<u64>);

impl GroundingObserver for InstantiationRows {
    fn enter(&self) {}
    fn exit(&self) {}

    fn details_enabled(&self) -> bool {
        true
    }

    fn phase_exit(
        &self,
        phase: GroundingPhase,
        _: Option<Location>,
        outcome: GroundingOutcome,
        work: GroundingWork,
    ) {
        if phase == GroundingPhase::RuleInstantiation {
            assert_eq!(outcome, GroundingOutcome::Completed);
            self.0.set(
                self.0
                    .get()
                    .checked_add(work.join_rows.expect("finite component row visits"))
                    .unwrap(),
            );
        }
    }
}

#[test]
fn factored_cartesian_bodies_visit_only_component_rows() {
    let visits = InstantiationRows::default();
    let mut limits = FormulaLimits::default();
    limits.theory.max_roots = 200;
    let admitted = admit_formula_with_grounding_observer(
        CARTESIAN_SOURCE.to_owned(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        limits,
        Some(&visits),
    )
    .unwrap();
    // Canonical compilation and final publication also consume global work.
    // Count the actual factorized route: two 40-row components, rather than
    // the 40 + 40*40 row visits of a complete Cartesian join. The compact root
    // ceiling independently prevents publishing that Cartesian formula family.
    assert_eq!(visits.0.get(), 40 + 40);
    assert_eq!(stable(&admitted), stable(&input("a(1..40).b(1..40).h.")));
}

#[test]
fn a_factored_root_retains_its_source_location() {
    let source = CARTESIAN_SOURCE;
    let admitted = input(source);
    assert!(admitted.formula_origins().iter().flatten().any(|location| {
        let span = location.span;
        &source[usize::try_from(span.start().get()).unwrap()
            ..usize::try_from(span.end().get()).unwrap()]
            == "h:-a(X),b(Y)."
    }));
}

#[test]
fn a_factored_source_respects_the_zero_root_ceiling() {
    let mut limits = FormulaLimits::default();
    limits.theory.max_roots = 0;
    assert!(matches!(
        admit_formula(
            CARTESIAN_SOURCE.to_owned(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            limits
        ),
        Err(zetesis_themelios::FormulaFailure::Limit {
            resource: FormulaResource::Roots,
            ..
        })
    ));
}
