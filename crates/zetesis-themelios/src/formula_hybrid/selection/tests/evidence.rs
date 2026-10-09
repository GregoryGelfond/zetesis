use super::*;
use crate::formula_hybrid::{Candidate, body};
use crate::formula_support::testing::budget;
use crate::formula_support::{Context, PreparedRule};
use zetesis_core::Model;

/// Lend one completed row selected from an all-held production region.
fn with_held_row(
    source: &str,
    check: impl FnOnce(
        &crate::formula_ir::RuleIr,
        &crate::formula_binding::Binding<'_>,
        &crate::formula_support::PositiveRows<'_>,
        Candidate<'_>,
        zetesis_core::AtomLookup<'_, '_>,
        &crate::formula_support::Computation<'_, '_>,
        &FormulaLimits,
    ),
) {
    let owner = owner_of(source);
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let prepared = checker.prepared.as_mut().unwrap();
    let mut counters = Counters::default();
    prepared
        .prepare_selection(owner.core(), &mut counters)
        .unwrap();
    let mut region = Region::all_open(owner.atom_catalog().atoms().len());
    for position in 0..owner.atom_catalog().atoms().len() {
        assert!(region.hold(position));
    }
    let index = prepared.index.unwrap().lookup();
    let selection = Selection {
        predicates: None,
        rows: prepared.rows.as_ref().unwrap(),
        index,
        region: &region,
    };
    assert_eq!(prepared.source.rules.len(), 1);
    let rule = &prepared.source.rules[0];
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
    let row = rows
        .next_row(
            &mut computation,
            &prepared.limits,
            &mut budget(),
            &mut counters,
            rule.location,
        )
        .unwrap()
        .unwrap();
    assert!(row.passes);
    let proof = row
        .positives
        .as_ref()
        .expect("production Selection established held positive rows");
    assert!(proof.covers(&rule.body));
    check(
        rule,
        &row.values,
        proof,
        Candidate::Region(owner.core_theory(), &region),
        index,
        &computation,
        &prepared.limits,
    );
}

#[test]
fn held_row_loans_skip_positive_region_queries() {
    with_held_row(
        include_str!("../../../../tests/fixtures/streamed-consequences/duplicate-patterns.lp"),
        |rule, binding, proof, candidate, index, computation, limits| {
            let mut proved = Counters::default();
            let mut ordinary = Counters::default();
            assert!(
                body(
                    &rule.body,
                    binding,
                    Some(proof),
                    candidate,
                    Some(index),
                    None,
                    Context::new(computation, limits, &mut proved, rule.location),
                )
                .unwrap()
            );
            assert!(
                body(
                    &rule.body,
                    binding,
                    None,
                    candidate,
                    Some(index),
                    None,
                    Context::new(computation, limits, &mut ordinary, rule.location),
                )
                .unwrap()
            );
            assert!(proved.accounting.work < ordinary.accounting.work);
            // Program raising coalesces the repeated authored literal. The raw
            // occurrence test independently checks repeated join occurrences.
            assert_eq!(rule.body.len(), 1);
            assert_eq!(proved.accounting.work, 1);
        },
    );
}

#[test]
fn positive_evidence_retains_negative_truth() {
    for (source, expected) in [
        (
            include_str!("../../../../tests/fixtures/streamed-consequences/negative-first.lp"),
            false,
        ),
        (
            include_str!(
                "../../../../tests/fixtures/streamed-consequences/pivot-double-negative.lp"
            ),
            true,
        ),
    ] {
        with_held_row(
            source,
            |rule, binding, proof, candidate, index, computation, limits| {
                let mut proved = Counters::default();
                let mut ordinary = Counters::default();
                assert_eq!(
                    body(
                        &rule.body,
                        binding,
                        Some(proof),
                        candidate,
                        Some(index),
                        None,
                        Context::new(computation, limits, &mut proved, rule.location),
                    )
                    .unwrap(),
                    expected
                );
                assert_eq!(
                    body(
                        &rule.body,
                        binding,
                        None,
                        candidate,
                        Some(index),
                        None,
                        Context::new(computation, limits, &mut ordinary, rule.location),
                    )
                    .unwrap(),
                    expected
                );
                // Raising may order positives before negatives. Both paths still
                // inspect the negative's truth; only the proved route saves work.
                assert!(proved.accounting.work < ordinary.accounting.work);
            },
        );
    }
}

#[test]
fn foreign_bodies_cannot_consume_row_evidence() {
    with_held_row(
        include_str!("../../../../tests/fixtures/streamed-consequences/one-row-unit.lp"),
        |rule, binding, proof, candidate, index, computation, limits| {
            let foreign: Vec<_> = rule
                .body
                .iter()
                .map(|literal| match literal {
                    crate::formula_ir::LiteralIr::Atom(negation, pattern) => {
                        crate::formula_ir::LiteralIr::Atom(*negation, *pattern)
                    }
                    _ => panic!("ordinary source atoms"),
                })
                .collect();
            assert!(
                !proof.covers(&foreign),
                "equal literals do not authenticate their occurrences"
            );
            let mut unauthenticated = Counters::default();
            let mut ordinary = Counters::default();
            assert!(
                body(
                    &foreign,
                    binding,
                    Some(proof),
                    candidate,
                    Some(index),
                    None,
                    Context::new(computation, limits, &mut unauthenticated, rule.location),
                )
                .unwrap()
            );
            assert!(
                body(
                    &rule.body,
                    binding,
                    None,
                    candidate,
                    Some(index),
                    None,
                    Context::new(computation, limits, &mut ordinary, rule.location),
                )
                .unwrap()
            );
            assert_eq!(unauthenticated.accounting.work, ordinary.accounting.work);
        },
    );
}

#[test]
fn positive_evidence_retains_body_work_refusal() {
    with_held_row(
        include_str!("../../../../tests/fixtures/streamed-consequences/one-row-unit.lp"),
        |rule, binding, proof, candidate, index, computation, limits| {
            let limits = FormulaLimits {
                max_work: 0,
                ..*limits
            };
            assert!(matches!(
                body(
                    &rule.body,
                    binding,
                    Some(proof),
                    candidate,
                    Some(index),
                    None,
                    Context::new(
                        computation,
                        &limits,
                        &mut Counters::default(),
                        rule.location
                    ),
                ),
                Err(FormulaFailure::Limit {
                    resource: FormulaResource::Work,
                    observed: 1,
                    limit: 0,
                    ..
                })
            ));
        },
    );
}

#[test]
fn positive_row_evidence_cannot_replace_model_truth() {
    let owner = owner_of(include_str!(
        "../../../../tests/fixtures/streamed-consequences/one-row-unit.lp"
    ));
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let prepared = checker.prepared.as_mut().unwrap();
    let mut counters = Counters::default();
    prepared
        .prepare_selection(owner.core(), &mut counters)
        .unwrap();
    let mut region = Region::all_open(owner.atom_catalog().atoms().len());
    for position in 0..owner.atom_catalog().atoms().len() {
        assert!(region.hold(position));
    }
    let index = prepared.index.unwrap().lookup();
    let selection = Selection {
        predicates: None,
        rows: prepared.rows.as_ref().unwrap(),
        index,
        region: &region,
    };
    let rule = &prepared.source.rules[0];
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
    let row = rows
        .next_row(
            &mut computation,
            &prepared.limits,
            &mut budget(),
            &mut counters,
            rule.location,
        )
        .unwrap()
        .unwrap();
    assert!(row.passes);
    let proof = row.positives.as_ref().unwrap();
    let empty = Model::new([]).unwrap();
    let mut proved = Counters::default();
    let mut ordinary = Counters::default();
    assert!(
        !body(
            &rule.body,
            &row.values,
            Some(proof),
            Candidate::Model(&empty),
            None,
            None,
            Context::new(&computation, &prepared.limits, &mut proved, rule.location)
        )
        .unwrap()
    );
    assert!(
        !body(
            &rule.body,
            &row.values,
            None,
            Candidate::Model(&empty),
            None,
            None,
            Context::new(&computation, &prepared.limits, &mut ordinary, rule.location)
        )
        .unwrap()
    );
    assert_eq!(proved.accounting.work, ordinary.accounting.work);
}

#[test]
fn unmapped_rows_keep_missing_owner_failures() {
    let owner = owner_of(include_str!(
        "../../../../tests/fixtures/streamed-consequences/one-row-unit.lp"
    ));
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let prepared = checker.prepared.as_mut().unwrap();
    let mut counters = Counters::default();
    let index = zetesis_core::AtomIndex::new_with(&[], || Ok::<_, FormulaFailure>(())).unwrap();
    let positions = RowPositions::prepare(
        &prepared.completed,
        index.lookup(),
        &prepared.limits,
        &mut counters,
        prepared.source.location,
    )
    .unwrap();
    let source_rows = SourceRows::attach(
        &prepared.completed,
        &positions,
        &prepared.limits,
        &mut counters,
        prepared.source.location,
    )
    .unwrap();
    let region = Region::all_open(owner.atom_catalog().atoms().len());
    let selection = Selection {
        predicates: None,
        rows: &source_rows,
        index: index.lookup(),
        region: &region,
    };
    let rule = &prepared.source.rules[0];
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
    let row = rows
        .next_row(
            &mut computation,
            &prepared.limits,
            &mut budget(),
            &mut counters,
            rule.location,
        )
        .unwrap()
        .unwrap();
    assert!(row.passes);
    assert!(row.positives.is_none());
    assert!(matches!(
        body(
            &rule.body,
            &row.values,
            row.positives.as_ref(),
            Candidate::Region(owner.core_theory(), &region),
            Some(index.lookup()),
            None,
            Context::new(&computation, &prepared.limits, &mut counters, rule.location)
        ),
        Err(FormulaFailure::SupportRelation {
            error: Failure::Owner,
            ..
        })
    ));
}
