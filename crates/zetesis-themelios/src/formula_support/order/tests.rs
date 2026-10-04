use crate::ProgramSite;
use themelios_base::source::SourceId;
use themelios_base::span::{ByteOffset, Span};
use themelios_program::program::{DefaultNegation, Relation};
use zetesis_core::{AtomPattern, Predicate, Sign, Term};

use super::arrange;
use crate::ExpansionLimits;
use crate::expansion::Budget;
use crate::formula_ir::{Expression, LiteralIr, Operation};
use crate::formula_support::testing::Fixture;
use crate::formula_support::{PatternOccurrence, PositivePattern};

fn location() -> ProgramSite {
    ProgramSite::source(themelios_base::span::Location {
        source: SourceId::new(37),
        span: Span::empty(ByteOffset::new(12)),
    })
}

fn atom(fixture: &mut Fixture, name: &str, variables: &[usize]) -> LiteralIr {
    let pattern = AtomPattern::new(
        Predicate::with_sign(name, variables.len(), Sign::Positive).unwrap(),
        variables
            .iter()
            .map(|&variable| Term::Variable(variable))
            .collect(),
    )
    .unwrap();
    LiteralIr::Atom(DefaultNegation::None, fixture.pattern(&pattern, location()))
}

fn less(left: usize, right: usize) -> LiteralIr {
    let variable = |variable| Expression {
        nodes: vec![Operation::Variable(variable)],
    };
    LiteralIr::Compare(variable(left), Relation::Lt, variable(right))
}

/// The predicate names of `literals` in the order `arrange` chooses, with
/// `sizes` giving each positive occurrence's relation size in source order.
fn order(
    fixture: &mut Fixture,
    literals: &[LiteralIr],
    sizes: &[usize],
    variables: usize,
) -> Vec<String> {
    fixture.with(location(), |_, computation, counters| {
        let mut occurrences: Vec<_> = literals
            .iter()
            .enumerate()
            .filter_map(|(source, literal)| match literal {
                LiteralIr::Atom(_, pattern) => Some(PatternOccurrence {
                    pattern: PositivePattern::Flat(
                        computation
                            .static_pattern(
                                *pattern,
                                &crate::FormulaLimits::default(),
                                counters,
                                location(),
                            )
                            .unwrap(),
                    ),
                    source,
                }),
                _ => None,
            })
            .collect();
        let positions: Vec<usize> = occurrences.iter().map(|o| o.source).collect();
        let mut bound = vec![false; variables];
        let mut budget = Budget::new(ExpansionLimits::default(), usize::MAX);
        arrange(
            &mut occurrences,
            literals,
            &mut bound,
            |occurrence| {
                sizes[positions
                    .iter()
                    .position(|&p| p == occurrence.source)
                    .unwrap()]
            },
            &mut budget,
            location(),
        )
        .unwrap();
        occurrences
            .iter()
            .map(|occurrence| occurrence.atom().predicate().name().to_owned())
            .collect()
    })
}

#[test]
fn a_comparison_is_decided_before_an_unrelated_occurrence_of_the_same_size() {
    // d(W), d(X), d(Y) in canonical order with X < Y: d(X) and d(Y) each
    // bind a variable the comparison waits on and d(W) none, so d(X) comes
    // first by canonical order among them, d(Y) then decides the comparison
    // and d(W) joins the surviving rows.
    let mut fixture = Fixture::default();
    let literals = [
        atom(&mut fixture, "dw", &[0]),
        atom(&mut fixture, "dx", &[1]),
        atom(&mut fixture, "dy", &[2]),
        less(1, 2),
    ];
    assert_eq!(
        order(&mut fixture, &literals, &[40, 40, 40], 3),
        ["dx", "dy", "dw"]
    );
}

#[test]
fn a_smaller_relation_precedes_a_comparison_decider() {
    let mut fixture = Fixture::default();
    let literals = [
        atom(&mut fixture, "big", &[0]),
        atom(&mut fixture, "small", &[1]),
        less(0, 0),
    ];
    assert_eq!(
        order(&mut fixture, &literals, &[100, 10], 2),
        ["small", "big"]
    );
}

#[test]
fn a_bound_occurrence_is_a_test_and_precedes_every_generator() {
    // a(X) binds X; c(X) is then a test although its relation is the largest.
    let mut fixture = Fixture::default();
    let literals = [
        atom(&mut fixture, "a", &[0]),
        atom(&mut fixture, "b", &[0, 1]),
        atom(&mut fixture, "c", &[0]),
    ];
    assert_eq!(
        order(&mut fixture, &literals, &[10, 500, 1000], 2),
        ["a", "c", "b"]
    );
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config { cases: 256, rng_seed: proptest::test_runner::RngSeed::Fixed(20_260_917), ..Default::default() })]
    /// Without comparisons or shared variables the order is the stable sort
    /// by relation size.
    #[test]
    fn without_comparisons_disjoint_occurrences_sort_stably_by_size(sizes in proptest::collection::vec(0_usize..4, 0..8)) {
        let mut fixture = Fixture::default();
        let literals: Vec<_> = sizes
            .iter()
            .enumerate()
            .map(|(index, _)| atom(&mut fixture, &format!("p{index}"), &[index]))
            .collect();
        let mut expected: Vec<(usize, usize)> = sizes.iter().copied().enumerate().map(|(index, size)| (size, index)).collect();
        expected.sort_unstable();
        let expected: Vec<String> = expected.iter().map(|(_, index)| format!("p{index}")).collect();
        proptest::prop_assert_eq!(order(&mut fixture, &literals, &sizes, sizes.len()), expected);
    }
}

#[test]
fn refused_preparation_capacity_is_not_reported_as_allocated() {
    let mut events = Vec::new();
    let mut admit = |capacity| {
        let super::Capacity::Requested(bytes) = capacity else {
            panic!("refused preflight cannot allocate");
        };
        events.push(bytes);
        Err(crate::FormulaFailure::UnsafeVariable {
            variable: 17,
            location: location(),
        })
    };
    let mut space = super::Workspace {
        bytes: 0,
        location: Some(location()),
        admit: Some(&mut admit),
    };
    assert!(matches!(
        space.reserve::<usize>(4),
        Err(crate::FormulaFailure::UnsafeVariable { variable: 17, .. })
    ));
    assert_eq!(space.bytes, 0);
    assert_eq!(events, [4 * size_of::<usize>() as u128]);
}

#[test]
fn preparation_observes_actual_cumulative_capacity() {
    let mut events = Vec::new();
    let mut admit = |capacity| {
        events.push(match capacity {
            super::Capacity::Requested(bytes) => (false, bytes),
            super::Capacity::Allocated(bytes) => (true, bytes),
        });
        Ok(())
    };
    let mut space = super::Workspace {
        bytes: 0,
        location: Some(location()),
        admit: Some(&mut admit),
    };
    let first = space.reserve::<usize>(3).unwrap();
    let first_bytes = first.capacity() as u128 * size_of::<usize>() as u128;
    let second = space.reserve::<u8>(5).unwrap();
    let total = first_bytes + second.capacity() as u128;
    assert_eq!(space.bytes, total);
    assert_eq!(
        events,
        [
            (false, 3 * size_of::<usize>() as u128),
            (true, first_bytes),
            (false, first_bytes + 5),
            (true, total),
        ]
    );
}

#[test]
fn arrangement_resolves_each_offered_count_once() {
    let mut fixture = Fixture::default();
    let literals = [
        atom(&mut fixture, "large", &[0]),
        atom(&mut fixture, "small", &[1]),
        atom(&mut fixture, "middle", &[2]),
    ];
    fixture.with(location(), |_, computation, counters| {
        let mut occurrences: Vec<_> = literals
            .iter()
            .enumerate()
            .map(|(source, literal)| {
                let LiteralIr::Atom(_, pattern) = literal else {
                    unreachable!()
                };
                PatternOccurrence {
                    source,
                    pattern: PositivePattern::Flat(
                        computation
                            .static_pattern(
                                *pattern,
                                &crate::FormulaLimits::default(),
                                counters,
                                location(),
                            )
                            .unwrap(),
                    ),
                }
            })
            .collect();
        let calls = std::cell::Cell::new(0);
        let mut budget = Budget::new(ExpansionLimits::default(), usize::MAX);
        arrange(
            &mut occurrences,
            &literals,
            &mut [false; 3],
            |occurrence| {
                calls.set(calls.get() + 1);
                [30, 10, 20][occurrence.source]
            },
            &mut budget,
            location(),
        )
        .unwrap();
        assert_eq!(calls.get(), 3);
        assert_eq!(
            occurrences
                .iter()
                .map(|item| item.source)
                .collect::<Vec<_>>(),
            [1, 2, 0]
        );
    });
}

#[test]
fn comparison_free_arrangement_refuses_every_short_work_prefix() {
    let mut fixture = Fixture::default();
    let literals: Vec<_> = (0..4)
        .map(|slot| atom(&mut fixture, "p", &[slot]))
        .collect();
    fixture.with(location(), |_, computation, counters| {
        let original: Vec<_> = literals.iter().enumerate().map(|(source, literal)| {
            let LiteralIr::Atom(_, pattern) = literal else { unreachable!() };
            PatternOccurrence {
                source,
                pattern: PositivePattern::Flat(computation.static_pattern(
                    *pattern, &crate::FormulaLimits::default(), counters, location(),
                ).unwrap()),
            }
        }).collect();
        let mut complete = Budget::new(ExpansionLimits::default(), usize::MAX);
        arrange(&mut original.clone(), &literals, &mut [false; 4], |_| 1, &mut complete, location()).unwrap();
        let required = complete.usage().term_work;
        assert!(required > original.len() * original.len());
        for maximum in 0..required {
            let mut budget = Budget::new(ExpansionLimits { max_term_work: maximum, ..ExpansionLimits::default() }, usize::MAX);
            let mut occurrences = original.clone();
            assert!(matches!(
                arrange(&mut occurrences, &literals, &mut [false; 4], |_| 1, &mut budget, location()),
                Err(crate::FormulaFailure::Expansion(crate::ExpansionFailure::Limit {
                    resource: crate::ExpansionResource::TermWork, limit, observed, ..
                })) if limit == maximum as u128 && observed > limit
            ));
            assert!(budget.usage().term_work <= maximum);
            // A refused in-place order may have completed earlier swaps; it
            // still contains each original occurrence exactly once.
            let mut sources: Vec<_> = occurrences.iter().map(|item| item.source).collect();
            sources.sort_unstable();
            assert_eq!(sources, [0, 1, 2, 3]);
        }
    });
}

#[test]
fn offered_counts_use_checked_predicate_text_comparison() {
    let prefix = "x".repeat(64);
    let atoms = ["a", "b", "c"].map(|suffix| {
        zetesis_core::Atom::new(
            Predicate::new(format!("p{prefix}{suffix}"), 0).unwrap(),
            vec![],
        )
        .unwrap()
    });
    let query = Predicate::new(format!("p{prefix}b"), 0).unwrap();
    let mut fixture = Fixture::from_atoms(atoms, location());
    fixture.with(location(), |support, _, _| {
        let mut required = 0;
        let expected = support
            .row_counts_with((&query).into(), || {
                required += 1;
                Ok::<_, usize>(())
            })
            .unwrap();
        assert_eq!(expected.1, 1);
        assert!(required > prefix.len());
        for maximum in 0..required {
            let mut calls = 0;
            let result = support.row_counts_with((&query).into(), || {
                if calls == maximum {
                    return Err(maximum);
                }
                calls += 1;
                Ok(())
            });
            assert_eq!(result, Err(maximum));
        }
        assert_eq!(
            support
                .row_counts_with((&query).into(), || Ok::<_, usize>(()))
                .unwrap(),
            expected
        );
    });
}

#[test]
fn comparison_readiness_refuses_unpaid_variable_scans() {
    let literals = [less(0, 1)];
    let mut complete = Budget::new(ExpansionLimits::default(), usize::MAX);
    super::Decisions::checked(
        &literals,
        &[],
        &[true, true],
        &mut complete,
        location(),
        &mut |_| Ok(()),
    )
    .unwrap();
    let required = complete.usage().term_work;
    assert!(required > 0);
    for maximum in 0..required {
        let mut budget = Budget::new(
            ExpansionLimits {
                max_term_work: maximum,
                ..ExpansionLimits::default()
            },
            usize::MAX,
        );
        assert!(matches!(
            super::Decisions::checked(&literals, &[], &[true, true], &mut budget, location(), &mut |_| Ok(())),
            Err(crate::FormulaFailure::Expansion(crate::ExpansionFailure::Limit {
                resource: crate::ExpansionResource::TermWork, limit, observed, ..
            })) if limit == maximum as u128 && observed > limit
        ));
    }
}
