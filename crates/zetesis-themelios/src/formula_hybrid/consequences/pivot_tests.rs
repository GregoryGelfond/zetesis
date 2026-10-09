use super::*;
use crate::formula_ir::RuleIr;
use crate::formula_support::testing::budget;
use crate::formula_support::{Computation, PreparedRule};
use crate::{AdmissionOptions, ConstraintCheckLimits, ExpansionLimits, FormulaLimits};

fn candidate(source: &str, other_held: Option<bool>) -> (crate::HybridFormula, Region) {
    let owner = crate::prepare_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap()
    .ground_hybrid()
    .unwrap();
    let mut region = Region::all_open(owner.atom_catalog().atoms().len());
    for (position, atom) in owner.atom_catalog().atoms().iter().enumerate() {
        if atom.predicate().name() != "p" {
            match other_held {
                Some(true) => assert!(region.hold(position)),
                Some(false) => assert!(region.cut(position)),
                None => {}
            }
        }
    }
    (owner, region)
}

fn original_pivot(
    rule: &RuleIr,
    prepared: &PreparedConstraints<'_>,
    counters: &mut Counters,
) -> usize {
    rule.body
        .iter()
        .position(|literal| {
            literal_atom(literal).is_some_and(|(negation, pattern)| {
                negation == DefaultNegation::None
                    && pattern
                        .get(
                            prepared.completed.components().unwrap(),
                            &prepared.limits,
                            counters,
                            rule.location,
                        )
                        .unwrap()
                        .predicate()
                        .name()
                        == "p"
            })
        })
        .unwrap()
}

/// Exercise the real immutable core mapping and original-occurrence selector.
fn with_pivot_rows(
    source: &str,
    other_held: Option<bool>,
    mut inspect: impl FnMut(
        &RuleIr,
        &Binding<'_>,
        &PositiveRows<'_>,
        &Region,
        AtomLookup<'_, '_>,
        &Computation<'_, '_>,
        &FormulaLimits,
    ),
) -> usize {
    let (owner, region) = candidate(source, other_held);
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let prepared = checker.prepared.as_mut().unwrap();
    let mut counters = Counters::default();
    prepared
        .prepare_selection(owner.core(), &mut counters)
        .unwrap();
    assert_eq!(prepared.source.rules.len(), 1);
    let rule = &prepared.source.rules[0];
    let index = prepared.index.unwrap().lookup();
    let pivot = original_pivot(rule, prepared, &mut counters);
    let selection = ConsequenceSelection {
        selection: crate::formula_hybrid::selection::Selection {
            predicates: None,
            rows: prepared.rows.as_ref().unwrap(),
            index,
            region: &region,
        },
        mode: ConsequenceMode::Pivot(pivot),
    };
    let plan = PreparedRule::new(
        rule,
        &mut prepared.completed,
        &prepared.limits,
        &mut budget(),
        &mut counters,
    )
    .unwrap();
    let queries = prepared
        .completed
        .queries(
            crate::JoinStrategy::Indexed,
            &prepared.limits,
            &counters,
            rule.location,
        )
        .unwrap();
    let mut computation = queries.computation(rule.location).unwrap();
    let mut rows = plan
        .rows(
            &queries,
            Some(&selection),
            &computation,
            &prepared.limits,
            &mut budget(),
            &mut counters,
        )
        .unwrap();
    let mut count = 0;
    while let Some(row) = rows
        .next_row(
            &mut computation,
            &prepared.limits,
            &mut budget(),
            &mut counters,
            rule.location,
        )
        .unwrap()
    {
        assert!(row.passes);
        let proof = row.positives.as_ref().expect("matched mapped open pivot");
        assert!(!proof.covers(&rule.body));
        assert_eq!(proof.open_in(&rule.body).unwrap().0, pivot);
        inspect(
            rule,
            &row.values,
            proof,
            &region,
            index,
            &computation,
            &prepared.limits,
        );
        count += 1;
    }
    assert!(
        count > 0,
        "the production selector yielded a completed pivot row"
    );
    count
}

#[test]
fn mapped_pivot_rows_avoid_catalog_queries() {
    let count = with_pivot_rows(
        include_str!("../../../tests/fixtures/streamed-consequences/mapped-pivot.lp"),
        Some(true),
        |rule, binding, proof, region, index, computation, limits| {
            let (_, atom) = proof.open_in(&rule.body).unwrap();
            let mut proved = Counters::default();
            let mut ordinary = Counters::default();
            let expected = ConstraintConsequence::Cut {
                atom,
                site: rule.location,
            };
            assert_eq!(
                body(
                    &rule.body,
                    binding,
                    Some(proof),
                    region,
                    index,
                    None,
                    Context::new(computation, limits, &mut proved, rule.location)
                )
                .unwrap(),
                expected,
            );
            assert_eq!(
                body(
                    &rule.body,
                    binding,
                    None,
                    region,
                    index,
                    None,
                    Context::new(computation, limits, &mut ordinary, rule.location)
                )
                .unwrap(),
                expected,
            );
            assert_eq!(
                proved.accounting.work, 2,
                "only the two literal visits remain"
            );
            assert!(ordinary.accounting.work > proved.accounting.work);
        },
    );
    assert_eq!(count, 2);
}

#[test]
fn pivot_evidence_preserves_negative_truth() {
    for (source, held, cut) in [
        (
            include_str!("../../../tests/fixtures/streamed-consequences/pivot-negative.lp"),
            Some(false),
            true,
        ),
        (
            include_str!("../../../tests/fixtures/streamed-consequences/pivot-negative.lp"),
            Some(true),
            false,
        ),
        (
            include_str!("../../../tests/fixtures/streamed-consequences/pivot-negative.lp"),
            None,
            false,
        ),
        (
            include_str!("../../../tests/fixtures/streamed-consequences/pivot-double-negative.lp"),
            Some(true),
            true,
        ),
        (
            include_str!("../../../tests/fixtures/streamed-consequences/pivot-double-negative.lp"),
            Some(false),
            false,
        ),
        (
            include_str!("../../../tests/fixtures/streamed-consequences/pivot-double-negative.lp"),
            None,
            false,
        ),
    ] {
        with_pivot_rows(
            source,
            held,
            |rule, binding, proof, region, index, computation, limits| {
                let expected = if cut {
                    ConstraintConsequence::Cut {
                        atom: proof.open_in(&rule.body).unwrap().1,
                        site: rule.location,
                    }
                } else {
                    ConstraintConsequence::NoConsequence
                };
                for evidence in [Some(proof), None] {
                    assert_eq!(
                        body(
                            &rule.body,
                            binding,
                            evidence,
                            region,
                            index,
                            None,
                            Context::new(
                                computation,
                                limits,
                                &mut Counters::default(),
                                rule.location
                            )
                        )
                        .unwrap(),
                        expected,
                    );
                }
            },
        );
    }
}

#[test]
fn copied_bodies_cannot_consume_pivot_evidence() {
    with_pivot_rows(
        include_str!("../../../tests/fixtures/streamed-consequences/one-row-unit.lp"),
        None,
        |rule, binding, proof, region, index, computation, limits| {
            let copied: Vec<_> = rule
                .body
                .iter()
                .map(|literal| match literal {
                    LiteralIr::Atom(sign, atom) => LiteralIr::Atom(*sign, *atom),
                    _ => panic!("this fixture has one flat atom"),
                })
                .collect();
            assert!(proof.open_in(&copied).is_none());
            let mut offered = Counters::default();
            let mut ordinary = Counters::default();
            let left = body(
                &copied,
                binding,
                Some(proof),
                region,
                index,
                None,
                Context::new(computation, limits, &mut offered, rule.location),
            )
            .unwrap();
            let right = body(
                &copied,
                binding,
                None,
                region,
                index,
                None,
                Context::new(computation, limits, &mut ordinary, rule.location),
            )
            .unwrap();
            assert_eq!(left, right);
            assert_eq!(offered.accounting.work, ordinary.accounting.work);
        },
    );
}

#[test]
fn pivot_evidence_retains_work_refusal() {
    with_pivot_rows(
        include_str!("../../../tests/fixtures/streamed-consequences/one-row-unit.lp"),
        None,
        |rule, binding, proof, region, index, computation, limits| {
            let limits = FormulaLimits {
                max_work: 0,
                ..*limits
            };
            assert!(matches!(
                body(
                    &rule.body,
                    binding,
                    Some(proof),
                    region,
                    index,
                    None,
                    Context::new(
                        computation,
                        &limits,
                        &mut Counters::default(),
                        rule.location
                    )
                ),
                Err(FormulaFailure::Limit {
                    resource: crate::FormulaResource::Work,
                    observed: 1,
                    limit: 0,
                    ..
                })
            ));
        },
    );
}

#[test]
fn pivot_evidence_retains_cancellation() {
    with_pivot_rows(
        include_str!("../../../tests/fixtures/streamed-consequences/one-row-unit.lp"),
        None,
        |rule, binding, proof, region, index, computation, limits| {
            let cancellation = Cancellation::default();
            cancellation.cancel();
            let mut counters = Counters::default().with_cancellation(Some(&cancellation));
            assert!(matches!(
                body(
                    &rule.body,
                    binding,
                    Some(proof),
                    region,
                    index,
                    None,
                    Context::new(computation, limits, &mut counters, rule.location)
                ),
                Err(FormulaFailure::Interrupted {
                    reason: zetesis_cpu::Stop::Cancelled,
                    ..
                })
            ));
        },
    );
}
