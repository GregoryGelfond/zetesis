use super::*;
use crate::formula_ir::{HeadIr, Prepared};
use crate::formula_support::testing::budget;
use crate::formula_support::{
    CompletedCatalog, CompletedQueries, Join, PreparedRule, build, testing,
};
use crate::grounding_observer::Profile;
use crate::test_support::Observer;
use crate::{FormulaLimits, FormulaResource, GroundingWork as Receipt};
use zetesis_cpu::Cancellation;

fn fixture(source: &str) -> (CompletedCatalog, Prepared) {
    let preparation = testing::prepare(source);
    let program = preparation.program;
    let catalog = build(
        preparation.catalog,
        &program,
        None,
        &FormulaLimits::default(),
        &mut budget(),
        &mut Counters::default(),
        preparation.location,
    )
    .unwrap();
    (catalog, program)
}

fn constraint(program: &Prepared) -> &crate::formula_ir::RuleIr {
    program
        .rules
        .iter()
        .find(|rule| matches!(rule.head, HeadIr::Normal(None)))
        .unwrap()
}

struct Run {
    bindings: Vec<(bool, Vec<String>)>,
    receipt: Receipt,
    work: u64,
}

fn run(
    prepared: &PreparedRule<'_>,
    queries: &CompletedQueries<'_>,
    counters: &mut Counters,
) -> Run {
    let limits = FormulaLimits::default();
    let location = prepared.rule.location;
    let observer = Observer::default();
    let profile = Profile::new(Some(&observer));
    let previous = std::mem::replace(&mut counters.observed, profile.work());
    let before = counters.accounting.work;
    let mut computation = queries.computation(location).unwrap();
    let mut rows = prepared
        .rows(
            queries,
            None,
            &computation,
            &limits,
            &mut budget(),
            counters,
        )
        .unwrap();
    let bindings = profile
        .phase(
            crate::grounding_observer::GroundingPhase::RuleInstantiation,
            Some(location),
            || {
                let mut bindings = Vec::new();
                while let Some(row) = rows
                    .next_row(&mut computation, &limits, &mut budget(), counters, location)
                    .unwrap()
                {
                    // Immutable support selection grants no candidate truth.
                    assert!(row.positives.is_none());
                    let values = (0..row.values.len())
                        .map(|slot| {
                            row.values
                                .read(slot, computation.read(), location)
                                .unwrap()
                                .to_string()
                        })
                        .collect();
                    bindings.push((row.passes, values));
                }
                Ok::<_, FormulaFailure>(bindings)
            },
        )
        .unwrap();
    drop(rows);
    counters.observed = previous;
    Run {
        bindings,
        receipt: observer.0.get(),
        work: counters.accounting.work - before,
    }
}

fn compare(source: &str) -> (Run, Run) {
    let (catalog, program) = fixture(source);
    let rule = constraint(&program);
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let mut completed = catalog
        .snapshot(&limits, &mut counters, rule.location)
        .unwrap();
    let mut prepared =
        PreparedRule::new(rule, &mut completed, &limits, &mut budget(), &mut counters).unwrap();
    assert!(
        prepared.pattern_rows.is_some(),
        "fixture exercises a proper structural selection"
    );
    let retained = counters.accounting.workspace.bytes();
    let queries = completed
        .queries(
            crate::JoinStrategy::Indexed,
            &limits,
            &counters,
            rule.location,
        )
        .unwrap();
    let selected = run(&prepared, &queries, &mut counters);
    assert_eq!(counters.accounting.workspace.bytes(), retained);
    let pattern_rows = prepared.pattern_rows.take();
    let reference = run(&prepared, &queries, &mut counters);
    prepared.pattern_rows = pattern_rows;
    // Dropping and restarting cursors must preserve the complete ordered binding sequence.
    assert_eq!(
        run(&prepared, &queries, &mut counters).bindings,
        selected.bindings
    );
    assert_eq!(selected.bindings, reference.bindings);
    (selected, reference)
}

#[test]
fn partial_structures_reduce_offered_rows() {
    let (selected, reference) = compare(include_str!(
        "../../../../tests/fixtures/pattern-rows/partial-structures.lp"
    ));
    assert_eq!(selected.bindings.len(), 2);
    assert!(selected.receipt.join_rows < reference.receipt.join_rows);
    assert!(selected.work < reference.work);
}

#[test]
fn pattern_rows_preserve_correlated_aliases() {
    for source in [
        include_str!("../../../../tests/fixtures/pattern-rows/repeated-variable.lp"),
        include_str!("../../../../tests/fixtures/pattern-rows/nested-aliases.lp"),
        include_str!("../../../../tests/fixtures/pattern-rows/anonymous-arguments.lp"),
        include_str!("../../../../tests/fixtures/pattern-rows/mixed-terms.lp"),
        include_str!("../../../../tests/fixtures/pattern-rows/repeated-occurrences.lp"),
    ] {
        let (selected, reference) = compare(source);
        assert!(!selected.bindings.is_empty());
        assert!(selected.receipt.join_rows < reference.receipt.join_rows);
    }
}

#[test]
fn pattern_rows_include_optional_and_derived_producers() {
    let (selected, _) = compare(include_str!(
        "../../../../tests/fixtures/pattern-rows/optional-derived-producers.lp"
    ));
    assert_eq!(selected.bindings.len(), 2);
}

#[test]
fn dynamic_bindings_still_reach_the_matcher() {
    let (selected, _) = compare(include_str!(
        "../../../../tests/fixtures/pattern-rows/dynamic-bindings.lp"
    ));
    assert_eq!(
        selected
            .bindings
            .iter()
            .filter(|(passes, _)| *passes)
            .count(),
        2
    );
}

#[test]
fn narrower_whole_argument_postings_remain_preferred() {
    let (selected, reference) = compare(include_str!(
        "../../../../tests/fixtures/pattern-rows/whole-argument-postings.lp"
    ));
    assert_eq!(selected.bindings.len(), 1);
    assert_eq!(selected.receipt.join_rows, reference.receipt.join_rows);
}

#[test]
fn complete_pattern_selection_keeps_no_row_copy() {
    let (catalog, program) = fixture(include_str!(
        "../../../../tests/fixtures/pattern-rows/full-selection.lp"
    ));
    let rule = constraint(&program);
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let mut completed = catalog
        .snapshot(&limits, &mut counters, rule.location)
        .unwrap();
    let prepared =
        PreparedRule::new(rule, &mut completed, &limits, &mut budget(), &mut counters).unwrap();
    assert!(prepared.pattern_rows.is_none());
    assert_eq!(counters.accounting.workspace.bytes(), 0);
}

#[test]
fn empty_pattern_rows_authenticate_source_occurrences() {
    let source = include_str!("../../../../tests/fixtures/pattern-rows/empty-selection.lp");
    let (catalog, program) = fixture(source);
    let (foreign, foreign_program) = fixture(source);
    let rule = constraint(&program);
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let mut completed = catalog
        .snapshot(&limits, &mut counters, rule.location)
        .unwrap();
    let prepared =
        PreparedRule::new(rule, &mut completed, &limits, &mut budget(), &mut counters).unwrap();
    let pattern_rows = prepared.pattern_rows.as_ref().unwrap();
    let entry = pattern_rows
        .entries
        .iter()
        .position(Option::is_some)
        .unwrap();
    assert!(
        pattern_rows.entries[entry]
            .as_ref()
            .unwrap()
            .positions
            .is_empty()
    );
    let foreign = foreign
        .snapshot(&limits, &mut counters, rule.location)
        .unwrap();
    let occurrence = prepared.plan.patterns[entry];
    let rows = foreign
        .relations
        .find_with(
            occurrence.atom().predicate(),
            &limits,
            &mut counters,
            rule.location,
        )
        .unwrap();
    assert!(matches!(
        pattern_rows.posting(entry, rows, &limits, &mut counters, rule.location),
        Err(FormulaFailure::SupportRelation {
            error: zetesis_core::relation::Failure::Owner,
            ..
        })
    ));
    let queries = completed
        .queries(
            crate::JoinStrategy::Indexed,
            &limits,
            &counters,
            rule.location,
        )
        .unwrap();
    let computation = queries.computation(rule.location).unwrap();
    assert!(matches!(
        Join::filtered_rule(
            constraint(&foreign_program),
            queries.support(),
            None,
            Some(&prepared),
            &mut budget(),
            Context::new(&computation, &limits, &mut counters, rule.location),
        ),
        Err(FormulaFailure::SupportRelation {
            error: zetesis_core::relation::Failure::Owner,
            ..
        })
    ));
}

#[test]
fn refused_pattern_preparation_releases_partial_storage() {
    let (catalog, program) = fixture(include_str!(
        "../../../../tests/fixtures/pattern-rows/refused-preparation.lp"
    ));
    let rule = constraint(&program);
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let mut completed = catalog
        .snapshot(&limits, &mut counters, rule.location)
        .unwrap();
    let prepared =
        PreparedRule::new(rule, &mut completed, &limits, &mut budget(), &mut counters).unwrap();
    let retained = counters.accounting.workspace.bytes();
    let queries = completed
        .queries(
            crate::JoinStrategy::Indexed,
            &limits,
            &counters,
            rule.location,
        )
        .unwrap();
    let computation = queries.computation(rule.location).unwrap();
    let before = counters.accounting.work;
    drop(
        PatternRows::prepare(
            &prepared.plan,
            &completed.relations,
            rule.variables,
            &mut budget(),
            Context::new(&computation, &limits, &mut counters, rule.location),
        )
        .unwrap(),
    );
    let required = counters.accounting.work - before;
    for remaining in [0, 1, required / 2, required - 1] {
        let limited = FormulaLimits {
            max_work: counters.accounting.work + remaining,
            ..limits
        };
        assert!(matches!(
            PatternRows::prepare(
                &prepared.plan,
                &completed.relations,
                rule.variables,
                &mut budget(),
                Context::new(&computation, &limited, &mut counters, rule.location),
            ),
            Err(FormulaFailure::Limit {
                resource: FormulaResource::Work,
                ..
            })
        ));
        assert_eq!(counters.accounting.workspace.bytes(), retained);
    }
    let limited = FormulaLimits {
        max_support_bytes: completed.relations.bytes
            + size_of::<super::super::super::Support<'_>>()
            + retained,
        ..limits
    };
    assert!(matches!(
        PatternRows::prepare(
            &prepared.plan,
            &completed.relations,
            rule.variables,
            &mut budget(),
            Context::new(&computation, &limited, &mut counters, rule.location),
        ),
        Err(FormulaFailure::Limit {
            resource: FormulaResource::SupportBytes,
            ..
        })
    ));
    assert_eq!(counters.accounting.workspace.bytes(), retained);
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let mut interrupted = Counters::with_cancellation(counters, Some(&cancellation));
    assert!(matches!(
        PatternRows::prepare(
            &prepared.plan,
            &completed.relations,
            rule.variables,
            &mut budget(),
            Context::new(&computation, &limits, &mut interrupted, rule.location),
        ),
        Err(FormulaFailure::Interrupted { .. })
    ));
    assert_eq!(interrupted.accounting.workspace.bytes(), retained);
}
