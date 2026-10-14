use super::*;
use crate::FormulaLimits;
use crate::formula_ir::{HeadIr, LiteralIr, Operation, RuleIr};
use crate::formula_support::testing::Fixture;
use crate::formula_support::{Computation, Counters, Support};
use crate::test_support::location;
use themelios_program::program::DefaultNegation;
use zetesis_core::{Atom, AtomPattern, Predicate, Term, Value, ValueLimits};

fn fixture(relation: Relation) -> (Fixture, RuleIr) {
    let mut fixture = Fixture::from_atoms(
        [0, 1, 2].map(|value| {
            Atom::new(Predicate::new("p", 1).unwrap(), vec![Value::Number(value)]).unwrap()
        }),
        location(),
    );
    let mut pattern = |name, variables: &[usize]| {
        fixture.pattern(
            &AtomPattern::new(
                Predicate::new(name, variables.len()).unwrap(),
                variables.iter().copied().map(Term::Variable).collect(),
            )
            .unwrap(),
            location(),
        )
    };
    let left = pattern("p", &[0]);
    let right = pattern("p", &[1]);
    let head = pattern("h", &[0, 1]);
    let comparison = fixture.comparison(
        vec![Operation::Variable(0)],
        relation,
        vec![Operation::Variable(1)],
        location(),
    );
    let rule = RuleIr {
        head: HeadIr::Normal(Some(head)),
        body: vec![
            LiteralIr::Atom(DefaultNegation::None, left),
            LiteralIr::Atom(DefaultNegation::None, right),
            LiteralIr::Compare(comparison),
        ],
        variables: 2,
        body_variables: 2,
        bindings: None,
        location: location(),
        origins: vec![location()],
    };
    (fixture, rule)
}

fn selected<'source>(
    rule: &RuleIr,
    support: &Support<'source>,
    computation: &mut Computation<'_, 'source>,
    counters: &mut Counters,
    pivot: usize,
    evidence: bool,
) -> Vec<(Vec<Value>, bool)> {
    let limits = FormulaLimits::default();
    let mut budget = testing::budget();
    let mut join = Join::variant_rule(
        rule,
        delta::Variant::Delta(pivot),
        support,
        None,
        &mut budget,
        Context::new(computation, &limits, counters, location()),
    )
    .unwrap();
    join.select_numeric_probes(rule, &limits, counters).unwrap();
    assert!(
        join.expression_keys.is_some(),
        "flat numeric traversal is explicitly enabled"
    );
    if evidence {
        join.evidence();
    }
    let mut rows = Vec::new();
    while let Some(row) = join
        .next_row(computation, &limits, &mut budget, counters, location())
        .unwrap()
    {
        rows.push((
            (0..2)
                .map(|slot| {
                    row.values
                        .read(slot, computation.read(), location())
                        .unwrap()
                        .to_value(ValueLimits::default())
                        .unwrap()
                })
                .collect(),
            row.passes,
        ));
    }
    rows
}

#[test]
fn both_numeric_routes_preserve_the_exact_first_new_partition() {
    for relation in [Relation::Eq, Relation::Gt] {
        let (mut fixture, rule) = fixture(relation);
        fixture.with(location(), |support, computation, counters| {
            let predicate = Predicate::new("p", 1).unwrap();
            assert_eq!(support.old_rows(&predicate), 2);
            assert_eq!(support.row_count(&predicate), 3);
            let observer = crate::test_support::Observer::default();
            let profile = crate::grounding_observer::Profile::new(Some(&observer));
            counters.observed = profile.work();
            let actual =
                profile
                    .phase(GroundingPhase::SupportCompletion, None, || {
                        Ok::<_, std::convert::Infallible>([0, 1].map(|pivot| {
                            selected(&rule, support, computation, counters, pivot, false)
                        }))
                    })
                    .unwrap();
            let expected = if relation == Relation::Eq {
                vec![vec![2, 2]]
            } else {
                vec![vec![2, 0], vec![2, 1]]
            };
            assert_eq!(
                actual[0],
                expected
                    .iter()
                    .map(|row| (testing::numbers(row), true))
                    .collect::<Vec<_>>()
            );
            assert!(
                actual[1].is_empty(),
                "the later pivot cannot revisit an earlier new occurrence"
            );
            let work = observer.0.get();
            assert_eq!(
                work.numeric_index_builds,
                Some(1),
                "one full snapshot certificate serves both pivots"
            );
            if relation == Relation::Eq {
                assert_eq!(work.expression_key_probes, Some(2));
                assert_eq!(work.expression_key_rows, Some(1));
            } else {
                assert_eq!(work.range_probes, Some(2));
                assert_eq!(work.range_probe_rows, Some(2));
            }
            let reference = controlled(true, || {
                [0, 1].map(|pivot| selected(&rule, support, computation, counters, pivot, false))
            });
            assert_eq!(
                actual, reference,
                "source order and successful substitutions agree exactly"
            );
        });
    }
}

#[test]
fn evidence_clears_numeric_selection_and_keeps_defined_false_rows() {
    let (mut fixture, rule) = fixture(Relation::Eq);
    fixture.with(location(), |support, computation, counters| {
        let observer = crate::test_support::Observer::default();
        let profile = crate::grounding_observer::Profile::new(Some(&observer));
        counters.observed = profile.work();
        let rows = profile
            .phase(GroundingPhase::SupportCompletion, None, || {
                Ok::<_, std::convert::Infallible>(selected(
                    &rule,
                    support,
                    computation,
                    counters,
                    0,
                    true,
                ))
            })
            .unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows.iter().filter(|(_, passes)| *passes).count(), 1);
        assert_eq!(observer.0.get().expression_key_probes, Some(0));
        assert_eq!(observer.0.get().range_probes, Some(0));
        assert_eq!(observer.0.get().numeric_index_builds, Some(0));
    });
}

#[test]
fn recursive_head_production_uses_both_queries_without_changing_catalog_order() {
    for (source, equality) in [
        (
            "p(0). step(0,1). step(1,2). p(Y) :- p(X),step(A,Y),X+0=A+0.",
            true,
        ),
        ("p(0). d(1..3). p(Y) :- p(X),d(Y),Y<X+2.", false),
    ] {
        let before = controlled(true, || ground(testing::prepare(source), None, None)).unwrap();
        let receipts = Receipts::default();
        let after = ground(testing::prepare(source), Some(&receipts), None).unwrap();
        assert_same_program(&before, &after);
        let work = receipts.other.get();
        assert!(
            work.numeric_index_builds.unwrap() > 0,
            "round index built: {source}"
        );
        assert!(
            if equality {
                work.expression_key_probes.unwrap() > 0
            } else {
                work.range_probes.unwrap() > 0
            },
            "support-round route activated: {source}"
        );
    }
}

#[test]
fn expression_selection_in_support_keeps_scalar_failure_exclusion() {
    for selected_value in [0, 1] {
        let source = format!(
            "{{p(0;1)}}. {{q({selected_value})}}. h(X,Y) :- q(Y),p(X),X=Y+0,(X+2147483647)/1>0."
        );
        let before = controlled(true, || ground(testing::prepare(&source), None, None));
        let receipts = Receipts::default();
        let after = ground(testing::prepare(&source), Some(&receipts), None);
        if selected_value == 0 {
            assert_same_program(&before.unwrap(), &after.unwrap());
        } else {
            match (before.unwrap_err(), after.unwrap_err()) {
                (
                    FormulaFailure::Expansion(crate::ExpansionFailure::Evaluation {
                        error: before,
                        location: before_site,
                    }),
                    FormulaFailure::Expansion(crate::ExpansionFailure::Evaluation {
                        error: after,
                        location: after_site,
                    }),
                ) => {
                    assert_eq!(before, themelios_program::term::EvalError::Overflow);
                    assert_eq!(before, after);
                    assert_eq!(before_site, after_site);
                }
                other => panic!("same retained scalar failure expected: {other:?}"),
            }
        }
        assert!(
            receipts.other.get().expression_key_probes.unwrap() > 0,
            "the support traversal, before family evidence, uses the equality query"
        );
    }
}
