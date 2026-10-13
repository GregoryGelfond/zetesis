//! Independent verification keeps its cache beside the retained query cache.

use super::*;
use crate::Cancellation;
use crate::test_support::prepare;
use zetesis_ferraris::{
    AdmissionLimits, ExtremaMeasure, ExtremumCutDescriptor, ExtremumCutRule, FormulaParts, Node,
    NumericDomain, NumericRow, ThresholdDescriptor,
};

fn theory(rows: usize, cuts: bool) -> Theory {
    let mut spans = vec![
        NumericRow {
            start: 0,
            length: 0,
            domain: NumericDomain::Weights,
        };
        rows
    ];
    // One ordinal row and many unused weight rows still require dense cache
    // slots. Without live numeric nodes neither domain requests cache storage.
    spans[rows - 1] = NumericRow {
        start: 0,
        length: 1,
        domain: NumericDomain::Ordinal { rank_count: 3 },
    };
    let node = if cuts {
        Node::extremum_cut(0)
    } else {
        Node::falsum()
    };
    let descriptors = if cuts {
        vec![ExtremumCutDescriptor {
            row: rows - 1,
            rule: ExtremumCutRule::Prefix(2),
        }]
    } else {
        vec![]
    };
    let parts = FormulaParts::from_numeric_raw(
        vec![Node::atom(0), node],
        vec![],
        vec![0],
        vec![1],
        spans,
        vec![],
        descriptors,
    )
    .unwrap();
    Theory::new(1, parts, vec![], AdmissionLimits::default()).unwrap()
}

#[test]
fn completion_counts_both_live_ordinal_caches() {
    for rows in [1_usize, 64, 65, 129] {
        assert_numeric_receipt(&theory(rows, true), rows);
    }
}

fn assert_numeric_receipt(theory: &Theory, rows: usize) {
    let owner = prepare(theory);
    let nodes = theory.nodes().len() as u128;
    let cache = rows as u128 * size_of::<ExtremaMeasure>() as u128
        + rows.div_ceil(64) as u128 * size_of::<u64>() as u128;
    let evaluation = EvaluationWorkspace::requested_payload_bytes(theory);
    assert_eq!(evaluation, nodes * size_of::<bool>() as u128 + cache);
    let (variables, clauses) = owner.cnf_shape();
    let previous_transient = transient_counts(1, nodes, variables as u128, nodes);
    assert_eq!(transient(&owner), previous_transient + cache);
    let previous_query = query_bytes(
        1,
        nodes,
        owner.parameter_count() as u128,
        variables as u128,
        clauses as u128,
        nodes,
    );
    let actual = prepared(&owner, 1).unwrap();
    assert_eq!(u128::from(actual.query_bytes), previous_query + 2 * cache);
    let source = requirements(theory, Limits::default(), 1).unwrap();
    let previous_source = counts(
        1,
        u64::try_from(nodes).unwrap(),
        u64::try_from(theory.parts().occurrences()).unwrap(),
        0,
        nodes,
        Limits::default(),
        1,
    )
    .unwrap();
    assert_eq!(
        u128::from(source.query_bytes - previous_source.query_bytes),
        2 * cache,
    );
    assert!(source.query_bytes >= actual.query_bytes);
    assert_eq!(source.shared_bytes, previous_source.shared_bytes);
    assert_eq!(source.result_bytes, previous_source.result_bytes);
}

#[test]
fn unused_rows_without_cuts_preserve_completion_bytes() {
    let theory = theory(129, false);
    let owner = prepare(&theory);
    let nodes = theory.nodes().len() as u128;
    assert_eq!(EvaluationWorkspace::requested_payload_bytes(&theory), nodes);
    let (variables, clauses) = owner.cnf_shape();
    assert_eq!(
        u128::from(prepared(&owner, 1).unwrap().query_bytes),
        query_bytes(
            1,
            nodes,
            owner.parameter_count() as u128,
            variables as u128,
            clauses as u128,
            nodes
        ),
    );
    assert_eq!(
        transient(&owner),
        transient_counts(1, nodes, variables as u128, nodes)
    );
    let mut workspace = EvaluationWorkspace::default();
    workspace
        .reserve(&theory, usize::MAX, &Cancellation::default())
        .unwrap();
    assert!(workspace.retained_bytes() >= size_of::<EvaluationWorkspace>() as u128 + nodes);
}

#[test]
fn actual_cache_growth_is_admitted_before_candidate_work() {
    use super::super::{BatchVerdict, CompletionExecutor, Input, Statistics};
    use crate::search::{Budget, LocalQuota};
    use crate::{SearchMethod, SearchStatistics};
    use std::num::NonZeroUsize;

    let theory = theory(65, true);
    let owner = prepare(&theory);
    let larger = prepare(&self::theory(513, true));
    let candidates = [Interpretation::new(&theory, [0]).unwrap()];
    let verdicts = [BatchVerdict::Residual];
    let limits = Limits::default();
    let required = prepared(&owner, 1).unwrap();
    let ceiling = required.shared_bytes + required.result_bytes + required.query_bytes;
    let mut executor =
        CompletionExecutor::with_scratch_limit(NonZeroUsize::new(1).unwrap(), ceiling).unwrap();
    let cancellation = Cancellation::default();
    let mut budget = Budget {
        quota: LocalQuota,
        cancellation: &cancellation,
        limits: limits.search,
        statistics: SearchStatistics::default(),
    };
    let mut statistics = Statistics::default();
    let mut accepted = Vec::new();
    let mut state = crate::prepared_reduct::State::new(SearchMethod::Clauses);
    state
        .ensure(&theory, limits, &mut budget, &mut statistics)
        .unwrap();
    let before_budget = budget.statistics;
    let before_statistics = statistics;
    let mut actual = 0;
    let result = executor.complete_with(
        Input {
            theory: &theory,
            candidates: &candidates,
            verdicts: &verdicts,
            limits,
            prepared: None,
            query: None,
        },
        &mut budget,
        &mut statistics,
        &mut accepted,
        &mut state,
        |workspace, _, limit, cancellation| {
            workspace.reserve(&larger, limit, cancellation)?;
            actual = workspace.retained_bytes();
            Ok(())
        },
    );
    assert_eq!(result, Err(Incomplete::CompletionScratch));
    let progress = executor.last_statistics().unwrap();
    assert!(
        u128::from(progress.peak_scratch_bytes)
            >= u128::from(required.shared_bytes + required.result_bytes)
                + actual
                + transient(&owner)
    );
    assert!(progress.peak_scratch_bytes > ceiling);
    assert_eq!(progress.requested_scratch_bytes, ceiling);
    assert_eq!(progress.candidates, 0);
    assert_eq!(budget.statistics, before_budget);
    assert_eq!(statistics, before_statistics);
    assert!(accepted.is_empty());
}

#[test]
fn completion_counts_both_live_weighted_caches() {
    for rows in [1_usize, 64, 65, 129] {
        let mut spans = vec![
            NumericRow {
                start: 0,
                length: 0,
                domain: NumericDomain::Ordinal { rank_count: 3 },
            };
            rows
        ];
        spans[rows - 1] = NumericRow {
            start: 0,
            length: 1,
            domain: NumericDomain::Weights,
        };
        let parts = FormulaParts::from_numeric_raw(
            vec![Node::atom(0), Node::threshold(0)],
            vec![],
            vec![0],
            vec![2],
            spans,
            vec![ThresholdDescriptor {
                row: rows - 1,
                bound: 2,
            }],
            vec![],
        )
        .unwrap();
        let owner = Theory::new(1, parts, vec![], AdmissionLimits::default()).unwrap();
        assert_numeric_receipt(&owner, rows);
    }
}
