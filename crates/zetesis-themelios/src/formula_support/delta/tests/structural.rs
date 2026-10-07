//! Structural delta coverage uses the existing matcher and fixed-point builder.

use super::*;
use crate::formula::Compiled;
use crate::formula_pattern::{ArgumentPattern, PatternAtom, PatternNode};
use crate::formula_support::{Context, Probe, testing};
use crate::grounding_observer::{GroundingPhase, Profile};
use crate::test_support::Observer;
use crate::{FormulaFailure, FormulaResource, GroundingWork as Receipt};
use zetesis_core::{Sign, ValueNode, ValueNodeRef};
use zetesis_cpu::{Cancellation, Stop};
use zetesis_ferraris::{Interpretation, Limits, models, models_reduct};
use zetesis_test_support::programs::{atom, function, numbered};

fn value(nodes: Vec<ValueNode>) -> Value {
    Value::from_nodes(nodes, ValueLimits::default()).unwrap()
}

fn nested(left: i32, right: i32, last: i32) -> Value {
    value(vec![
        function("f", 4),
        function("g", 1),
        ValueNode::Number(left),
        ValueNode::Number(right),
        ValueNode::Number(7),
        ValueNode::Number(last),
    ])
}

fn constructor(
    fixture: &mut Fixture,
    name: &str,
    arity: usize,
) -> crate::formula_support::components::Constructor {
    fixture.constructor(
        ValueNodeRef::Function {
            sign: Sign::Positive,
            name,
            arity,
        },
        location(),
    )
}

fn structural(fixture: &mut Fixture, capture: usize, slot: usize) -> LiteralIr {
    let shape = constructor(fixture, "f", 4);
    let nested = constructor(fixture, "g", 1);
    let constant = fixture.scalar(&Value::Number(7), location());
    LiteralIr::PatternAtom(PatternAtom {
        atom: pattern(fixture, "p", &[capture]),
        arguments: vec![ArgumentPattern {
            position: 0,
            nodes: vec![
                PatternNode::Constructor(shape),
                PatternNode::Constructor(nested),
                PatternNode::Slot(slot),
                PatternNode::Slot(slot),
                PatternNode::Constant(constant),
                PatternNode::Wildcard,
            ],
        }],
    })
}

#[test]
fn structural_occurrences_partition_new_bindings() {
    // The middle row fails repeated-variable matching. The last is the only
    // new row, and the wildcard must not equate its value with the old row's.
    let mut fixture = Fixture::from_atoms(
        [nested(1, 1, 0), nested(9, 8, 0), nested(2, 2, 1)].map(|term| atom("p", vec![term])),
        location(),
    );
    let body = vec![
        structural(&mut fixture, 2, 0),
        literal(&mut fixture, DefaultNegation::Not, "absent", &[0]),
        structural(&mut fixture, 3, 1),
    ];
    let rule = rule(&mut fixture, body, 4);
    let limits = FormulaLimits::default();
    fixture.with(location(), |support, computation, counters| {
        let predicate = Predicate::new("p", 1).unwrap();
        assert_eq!(support.old_rows(&predicate), 2);
        assert_eq!(support.row_count(&predicate), 3);
        let mut schedule = variants(&rule, support, false, &limits, counters).unwrap();
        let mut actual = Vec::new();
        let mut pivots = Vec::new();
        while let Some(variant) = schedule.next(&limits, counters).unwrap() {
            let Variant::Delta(pivot) = variant else {
                panic!("structural delta route")
            };
            pivots.push(pivot);
            actual.extend(selected(&rule, support, Some(pivot), computation, counters));
        }
        assert_eq!(pivots, [0, 2]);
        let mut expected = selected(&rule, support, None, computation, counters);
        assert_eq!(expected.len(), 4);
        expected.retain(|row| row[..2] != [Value::Number(1), Value::Number(1)]);
        actual.sort();
        expected.sort();
        assert_eq!(actual, expected);
        assert_eq!(actual.len(), 3);
        assert!(actual.windows(2).all(|pair| pair[0] != pair[1]));
    });
}

#[test]
fn structural_postings_intersect_original_occurrences() {
    let wrapped = |number| value(vec![function("f", 1), ValueNode::Number(number)]);
    let mut fixture = Fixture::from_atoms(
        [
            numbered("k", &[7, 1]),
            atom("p", vec![wrapped(1), Value::Number(10)]),
            atom("p", vec![wrapped(2), Value::Number(20)]),
            atom("p", vec![wrapped(1), Value::Number(30)]),
        ],
        location(),
    );
    let shape = constructor(&mut fixture, "f", 1);
    let body = vec![
        LiteralIr::PatternAtom(PatternAtom {
            atom: pattern(&mut fixture, "p", &[2, 1]),
            arguments: vec![ArgumentPattern {
                position: 0,
                nodes: vec![PatternNode::Constructor(shape), PatternNode::Slot(0)],
            }],
        }),
        LiteralIr::Atom(
            DefaultNegation::None,
            fixture.pattern(
                &AtomPattern::new(
                    Predicate::new("k", 2).unwrap(),
                    vec![Term::Constant(Value::Number(7)), Term::Variable(0)],
                )
                .unwrap(),
                location(),
            ),
        ),
    ];
    let rule = rule(&mut fixture, body, 3);
    let limits = FormulaLimits::default();
    fixture.with(location(), |support, computation, counters| {
        let mut budget = testing::budget();
        let mut join = Join::variant_rule(
            &rule,
            Variant::Delta(0),
            support,
            None,
            &mut budget,
            Context::new(computation, &limits, counters, location()),
        )
        .unwrap();
        assert_eq!(
            join.plan
                .patterns
                .iter()
                .map(|p| p.source)
                .collect::<Vec<_>>(),
            [1, 0]
        );
        let row = join
            .next(computation, &limits, &mut budget, counters, location())
            .unwrap()
            .unwrap();
        assert_eq!(
            row.read(1, computation.read(), location()).unwrap(),
            Value::Number(30)
        );
        let Some(Probe::Indexed(super::super::Rows::Posting(posting))) = &join.probes[1] else {
            panic!("fully bound structure uses an actual posting");
        };
        assert_eq!(*posting, [2]);
        assert!(
            join.next(computation, &limits, &mut budget, counters, location())
                .unwrap()
                .is_none()
        );
        let full = selected(&rule, support, None, computation, counters);
        assert_eq!(
            full.iter().map(|row| row[1].clone()).collect::<Vec<_>>(),
            [Value::Number(10), Value::Number(30)]
        );
    });
}

const CHAIN: &str = "p(f(1)).edge(1,2).edge(2,3).edge(3,4).edge(4,5). \
    p(f(Y)):-p(f(X)),edge(X,Y). a;b. :~p(f(X)).[1@1,X]";

struct Run {
    atoms: Vec<Atom>,
    work: u64,
    receipt: Receipt,
}

fn completed(
    source: &str,
    enabled: bool,
    max_work: Option<u64>,
) -> (Result<Run, FormulaFailure>, usize) {
    scoped(enabled, || {
        let preparation = testing::prepare(source);
        let mut counters = Counters::resume(
            preparation.accounting,
            crate::grounding_observer::Work::default(),
        );
        let mut budget = preparation.budget;
        let limits = FormulaLimits {
            max_work: max_work.unwrap_or(preparation.limits.max_work),
            ..preparation.limits
        };
        let location = preparation.location;
        // Objectives and disjunction retain the general outer schedule. Only
        // the eligible rule's variants differ between these two runs.
        assert!(
            crate::formula_support::producers::ProducerPlan::prepare(
                &preparation.program,
                &preparation.catalog,
                &limits,
                &mut counters,
                location,
            )?
            .is_none()
        );
        let observer = Observer::default();
        let profile = Profile::new(Some(&observer));
        counters.observed = profile.work();
        let completed = profile.phase(GroundingPhase::SupportCompletion, Some(location), || {
            crate::formula_support::build(
                preparation.catalog,
                &preparation.program,
                None,
                &limits,
                &mut budget,
                &mut counters,
                location,
            )
        })?;
        let work = counters.accounting.work;
        // Explicit oracle export follows completion and is outside the observed
        // builder allowance; it does not mutate the admitted support owner.
        let snapshot = completed.snapshot(
            &FormulaLimits::default(),
            &mut Counters::default(),
            location,
        )?;
        let mut atoms: Vec<_> = snapshot
            .relations
            .source_atoms()
            .flat_map(|(_, source)| {
                source
                    .iter()
                    .map(|atom| atom.to_atom(ValueLimits::default()).unwrap())
            })
            .collect();
        atoms.sort();
        Ok(Run {
            atoms,
            work,
            receipt: observer.0.get(),
        })
    })
}

#[test]
fn mixed_sources_preserve_complete_support() {
    let (actual, visited) = completed(CHAIN, true, None);
    let (reference, skipped) = completed(CHAIN, false, None);
    let actual = actual.unwrap();
    let reference = reference.unwrap();
    assert!(visited > 1);
    assert_eq!(skipped, 0);
    assert_eq!(actual.atoms, reference.atoms);
    assert_eq!(actual.atoms.len(), 11);
    for number in 1..=5 {
        assert!(actual.atoms.contains(&atom(
            "p",
            vec![value(vec![function("f", 1), ValueNode::Number(number)])]
        )));
    }
    assert!(
        actual.receipt.support_join_work.unwrap() < reference.receipt.support_join_work.unwrap()
    );
    assert!(actual.receipt.join_rows.unwrap() < reference.receipt.join_rows.unwrap());
}

#[test]
fn unchanged_structural_joins_fit_the_saved_work() {
    let actual = completed(CHAIN, true, None).0.unwrap();
    let reference = completed(CHAIN, false, None).0.unwrap();
    assert!(actual.work < reference.work);
    assert_eq!(
        completed(CHAIN, true, Some(actual.work)).0.unwrap().atoms,
        actual.atoms
    );
    for enabled in [true, false] {
        let ceiling = actual.work - u64::from(enabled);
        let Err(error) = completed(CHAIN, enabled, Some(ceiling)).0 else {
            panic!("insufficient work cannot publish completed support");
        };
        assert!(matches!(
            error.cause(),
            FormulaFailure::Limit {
                resource: FormulaResource::Work,
                ..
            }
        ));
    }
}

fn compiled(source: &str, enabled: bool) -> (Result<Compiled, FormulaFailure>, usize) {
    scoped(enabled, || {
        crate::formula_ground::ground(testing::prepare(source), None, None)
    })
}

fn equivalent(source: &str) -> (Compiled, Compiled) {
    let (actual, visits) = compiled(source, true);
    let (reference, skipped) = compiled(source, false);
    let actual = actual.unwrap();
    let reference = reference.unwrap();
    assert!(visits > 0);
    assert_eq!(skipped, 0);
    let atoms = |compiled: &Compiled| {
        compiled
            .atoms
            .atoms()
            .iter()
            .map(|atom| atom.to_atom(ValueLimits::default()).unwrap())
            .collect::<Vec<_>>()
    };
    assert_eq!(atoms(&actual), atoms(&reference));
    assert_eq!(actual.theory.nodes(), reference.theory.nodes());
    assert_eq!(actual.theory.operands(), reference.theory.operands());
    assert_eq!(actual.theory.roots(), reference.theory.roots());
    assert_eq!(actual.origins, reference.origins);
    assert_eq!(actual.objective_origins, reference.objective_origins);
    assert_eq!(
        actual.objective_declarations,
        reference.objective_declarations
    );
    assert!(
        actual
            .objectives
            .templates()
            .iter()
            .eq(reference.objectives.templates().iter())
    );
    assert_eq!(actual.warnings, reference.warnings);
    (actual, reference)
}

#[test]
fn structural_deltas_preserve_formula_semantics() {
    let (actual, reference) =
        equivalent("p(f(1)).edge(1,2).p(f(Y)):-p(f(X)),edge(X,Y).a;b.:~p(f(X)).[1@1,X]");
    assert_eq!(actual.theory.atom_count(), 5);
    let worlds: Vec<_> = (0..1 << actual.theory.atom_count())
        .map(|bits| {
            let atoms = || (0..actual.theory.atom_count()).filter(|atom| bits & (1 << atom) != 0);
            (
                Interpretation::new(&actual.theory, atoms()).unwrap(),
                Interpretation::new(&reference.theory, atoms()).unwrap(),
            )
        })
        .collect();
    let cancellation = Cancellation::default();
    for (candidate, (left, right)) in worlds.iter().enumerate() {
        assert_eq!(
            models(&actual.theory, left, Limits::default(), &cancellation).unwrap(),
            models(&reference.theory, right, Limits::default(), &cancellation).unwrap()
        );
        for (tested, (left_test, right_test)) in worlds.iter().enumerate() {
            assert_eq!(
                models_reduct(
                    &actual.theory,
                    left,
                    left_test,
                    Limits::default(),
                    &cancellation
                )
                .unwrap(),
                models_reduct(
                    &reference.theory,
                    right,
                    right_test,
                    Limits::default(),
                    &cancellation
                )
                .unwrap(),
                "arbitrary M={candidate}, J={tested}"
            );
        }
    }
}

#[test]
fn structural_deltas_preserve_arithmetic_evidence() {
    let (actual, _) = equivalent("p(f(0)).p(f(1)).r(Y):-p(f(X)),Y=1/X.");
    assert_eq!(actual.warnings.len(), 1);
    for source in [
        "p(f(0)).r(Y):-p(f(X)),Y=1/X.",
        "p(f(0)).r(Y):-p(f(X)),Y=1/X,X=0.",
    ] {
        let (actual, visits) = compiled(source, true);
        let (reference, _) = compiled(source, false);
        assert!(visits > 0);
        let actual = actual.unwrap_err();
        let reference = reference.unwrap_err();
        assert!(matches!(actual.cause(), FormulaFailure::Expansion(_)));
        assert_eq!(actual.to_string(), reference.to_string());
    }
}

#[test]
fn unchanged_structural_inputs_have_no_variant() {
    let mut fixture = Fixture::from_atoms(
        [atom("p", vec![nested(1, 1, 0)]), numbered("q", &[0])],
        location(),
    );
    let body = vec![structural(&mut fixture, 1, 0)];
    let rule = rule(&mut fixture, body, 2);
    let limits = FormulaLimits::default();
    fixture.with(location(), |support, _, counters| {
        let predicate = Predicate::new("p", 1).unwrap();
        assert_eq!(support.old_rows(&predicate), 1);
        assert_eq!(support.row_count(&predicate), 1);
        let mut schedule = variants(&rule, support, false, &limits, counters).unwrap();
        assert!(schedule.next(&limits, counters).unwrap().is_none());
    });
}

#[test]
fn structural_variant_scans_observe_cancellation() {
    let mut fixture = Fixture::from_atoms([atom("p", vec![nested(1, 1, 0)])], location());
    let body = vec![
        structural(&mut fixture, 2, 0),
        structural(&mut fixture, 3, 1),
    ];
    let rule = rule(&mut fixture, body, 4);
    let limits = FormulaLimits::default();
    fixture.with(location(), |support, _, counters| {
        let mut variants = variants(&rule, support, false, &limits, counters).unwrap();
        assert!(matches!(
            variants.next(&limits, counters).unwrap(),
            Some(Variant::Delta(0))
        ));
        let cancellation = Cancellation::default();
        cancellation.cancel();
        counters.cancellation = Some(cancellation);
        let error = variants
            .next(&limits, counters)
            .err()
            .expect("charged scan must stop");
        assert_eq!(error.interruption(), Some(Stop::Cancelled));
        counters.cancellation = None;
    });
}

#[test]
fn structural_reference_restores_after_unwind() {
    assert!(structural_enabled());
    let result = std::panic::catch_unwind(|| scoped(false, || panic!("test unwind")));
    assert!(result.is_err());
    assert!(structural_enabled());
}

#[test]
fn structural_reference_stays_on_its_thread() {
    scoped(false, || {
        assert!(!structural_enabled());
        assert!(std::thread::spawn(structural_enabled).join().unwrap());
    });
}
