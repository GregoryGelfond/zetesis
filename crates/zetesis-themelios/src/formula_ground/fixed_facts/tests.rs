use std::cell::Cell;

use crate::formula::Compiled;
use crate::formula_ground::{Builder, Purpose, ground};
use crate::formula_support::testing;
use crate::test_support::compiled_atoms;
use crate::{ExpansionLimits, FormulaFailure, FormulaLimits, FormulaResource};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{Interpretation, Limits, models, models_reduct};

thread_local! {
    static ENABLED: Cell<bool> = const { Cell::new(true) };
}

pub(super) fn enabled() -> bool {
    ENABLED.get()
}

fn without<T>(run: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            ENABLED.set(self.0);
        }
    }
    let _restore = Restore(ENABLED.replace(false));
    run()
}

fn compile(source: &str) -> Compiled {
    ground(testing::prepare(source), None, None).unwrap()
}

fn interpretations(compiled: &Compiled) -> Vec<Interpretation> {
    let atoms = compiled_atoms(compiled);
    assert!(atoms.len() <= 8, "bounded exhaustive population");
    (0..1_usize << atoms.len())
        .map(|bits| {
            Interpretation::new(
                &compiled.theory,
                (0..atoms.len()).filter(|&atom| bits & (1 << atom) != 0),
            )
            .unwrap()
        })
        .collect()
}

const CASES: &[&str] = &[
    "{p}. q :- d,p. d.",
    "{p}. q :- d,p,not r. r :- not p. d.",
    "{p}. q :- not d,p. d.",
    "{p}. q :- not not d,p. d.",
    "{p}. q|r :- d,p. d.",
    "{-p}. -q :- -d,-p. -d.",
    "{p(1);p(2)}. q(X) :- d(X),p(X). d(1;2).",
    "{p}. q :- 2 <= #count { a:d; b:p }. d.",
];

#[test]
fn fixed_body_facts_preserve_original_truth() {
    let cancellation = Cancellation::default();
    for source in CASES {
        let actual = compile(source);
        let reference = without(|| compile(source));
        assert_eq!(
            compiled_atoms(&actual),
            compiled_atoms(&reference),
            "{source}"
        );
        for (actual_i, reference_i) in interpretations(&actual)
            .iter()
            .zip(interpretations(&reference))
        {
            assert_eq!(
                models(&actual.theory, actual_i, Limits::default(), &cancellation).unwrap(),
                models(
                    &reference.theory,
                    &reference_i,
                    Limits::default(),
                    &cancellation
                )
                .unwrap(),
                "{source}",
            );
        }
    }
}

#[test]
fn fixed_body_facts_preserve_arbitrary_frozen_truth() {
    let cancellation = Cancellation::default();
    for source in CASES {
        let actual = compile(source);
        let reference = without(|| compile(source));
        assert_eq!(
            compiled_atoms(&actual),
            compiled_atoms(&reference),
            "{source}"
        );
        let actual_i = interpretations(&actual);
        let reference_i = interpretations(&reference);
        for (candidate, actual_m) in actual_i.iter().enumerate() {
            for (tested, actual_j) in actual_i.iter().enumerate() {
                assert_eq!(
                    models_reduct(
                        &actual.theory,
                        actual_m,
                        actual_j,
                        Limits::default(),
                        &cancellation
                    )
                    .unwrap(),
                    models_reduct(
                        &reference.theory,
                        &reference_i[candidate],
                        &reference_i[tested],
                        Limits::default(),
                        &cancellation
                    )
                    .unwrap(),
                    "I={candidate}, J={tested}: {source}",
                );
            }
        }
    }
}

#[test]
fn late_facts_reduce_both_body_emission_routes() {
    let source = "{p(1)}. h(X) :- d(X),e(X),p(X). d(1). e(1).";
    for producer in [false, true] {
        let (actual, visits) =
            crate::formula_ground::producer::testing::scoped(producer, || compile(source));
        let (reference, _) = crate::formula_ground::producer::testing::scoped(producer, || {
            without(|| compile(source))
        });
        assert_eq!(compiled_atoms(&actual), compiled_atoms(&reference));
        assert!(actual.theory.nodes().len() < reference.theory.nodes().len());
        if producer {
            assert!(visits.published > 0, "complete witness producer route");
        }
    }
}

#[test]
fn possible_support_does_not_become_a_fact() {
    let source = "{d(1);p(1)}. h(X) :- d(X),p(X).";
    let actual = compile(source);
    let reference = without(|| compile(source));
    assert_eq!(actual.theory.nodes(), reference.theory.nodes());
    assert_eq!(actual.theory.operands(), reference.theory.operands());
    assert_eq!(actual.theory.roots(), reference.theory.roots());
}

#[test]
fn fact_rules_and_blocks_supply_the_same_authority() {
    let source = "{p(1)}. h(X) :- d(X),e(X),p(X). d(1). e(1).";
    let actual = compile(source);
    let reference = crate::formula_ir::testing::with_facts_as_rules(|| compile(source));
    assert_eq!(compiled_atoms(&actual), compiled_atoms(&reference));
    assert_eq!(actual.theory.nodes(), reference.theory.nodes());
    assert_eq!(actual.theory.operands(), reference.theory.operands());
    assert_eq!(actual.theory.roots(), reference.theory.roots());
}

#[test]
fn fixed_body_facts_keep_independent_scalar_errors() {
    for source in [
        "d(0). :- d(X),A=X+1,A=0,Z=1/X.",
        "d(1). :- d(X),not X/2=0,X+2147483647=0.",
        "d(0). h(1/X) :- d(X).",
    ] {
        let failure = || {
            crate::admit_formula(
                source.into(),
                crate::AdmissionOptions::default(),
                ExpansionLimits::default(),
                FormulaLimits::default(),
            )
            .expect_err("independent reached scalar failure")
        };
        assert_eq!(
            format!("{:?}", failure()),
            format!("{:?}", without(failure)),
            "{source}"
        );
    }
}

#[test]
fn stopped_fact_collection_publishes_no_formula() {
    testing::with_completed_source("d. {p}. q :- d,p.", |prepared, _, computation, counters| {
        let before = counters.accounting.work;
        let limits = FormulaLimits {
            max_work: before + 1,
            ..FormulaLimits::default()
        };
        let mut budget = testing::budget();
        let mut builder = Builder::empty(
            computation,
            &limits,
            &mut budget,
            counters,
            Purpose::Theory,
            None,
            crate::test_support::location(),
        )
        .unwrap();
        let result = builder.collect_fixed_facts(prepared.items());
        assert!(matches!(
            result,
            Err(FormulaFailure::Limit {
                resource: FormulaResource::Work,
                ..
            })
        ));
        assert!(builder.nodes.view().is_empty());
        assert!(builder.roots.is_empty());
        assert_eq!(builder.catalog.len(), 0);
    });
}
