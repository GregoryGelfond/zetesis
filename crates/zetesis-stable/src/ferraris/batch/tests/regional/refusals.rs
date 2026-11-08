//! Malformed, refused and uncommitted regional attempts.
use super::*;

#[test]
fn foreign_ticket_stops_before_completion_and_preserves_the_oldest_slate() {
    let (mut search, own, present) = conditional_slate();
    let (_other, foreign, _) = conditional_slate();
    let mut executor = CompletionExecutor::default();
    let before = search.statistics();
    assert!(matches!(
        search.begin_regional_pass(
            foreign,
            all_regional(),
            subset_roots(present),
            &mut executor
        ),
        Err(BatchError::SlateOrder)
    ));
    assert_eq!(search.statistics(), before);
    assert!(executor.last_statistics().is_none());
    assert_eq!(search.batch_statistics().completion_calls, 0);
    let pass = search
        .begin_regional_pass(own, all_regional(), subset_roots(present), &mut executor)
        .unwrap();
    assert_eq!(pass.finish(decisions(present)).unwrap().len(), 1);
}

#[test]
fn a_scratch_refusal_enters_completion_but_preserves_the_slate_and_work_allowance() {
    let (mut search, ticket, present) = conditional_slate();
    let id = ticket.slate();
    let mut executor =
        CompletionExecutor::with_scratch_limit(NonZeroUsize::new(2).unwrap(), 0).unwrap();
    let before = search.statistics();
    assert!(matches!(
        search.begin_regional_pass(ticket, all_regional(), subset_roots(present), &mut executor),
        Err(BatchError::Limits(Incomplete::CompletionScratch))
    ));
    let receipt = executor.last_statistics().unwrap();
    assert_eq!(
        (receipt.candidates, receipt.completed, receipt.residuals),
        (0, 0, 0)
    );
    assert_eq!(
        search.statistics().countermodel_queries,
        before.countermodel_queries
    );
    assert_eq!(search.batch_statistics().committed, 0);
    assert_eq!(search.batch_statistics().pending, 2);
    assert_eq!(search.batch_statistics().completion_calls, 1);
    assert!(!search.exhausted());
    let next = retry(&mut search, id);
    let mut enough = CompletionExecutor::default();
    let pass = search
        .begin_regional_pass(next, all_regional(), subset_roots(present), &mut enough)
        .unwrap();
    assert_eq!(pass.finish(decisions(present)).unwrap().len(), 1);
    assert_eq!(search.batch_statistics().completion_calls, 2);
    assert_eq!(search.batch_statistics().committed, 2);
}

#[test]
fn wrong_finish_count_does_no_second_query_and_releases_the_exclusive_pass() {
    let (mut search, ticket, present) = conditional_slate();
    let id = ticket.slate();
    let mut executor = CompletionExecutor::default();
    let before = search.statistics();
    let pass = search
        .begin_regional_pass(ticket, all_regional(), subset_roots(present), &mut executor)
        .unwrap();
    assert_eq!(pass.outcomes().count(), 2);
    let receipt = executor.last_statistics();
    assert!(matches!(
        pass.finish(vec![BatchVerdict::Refuted]),
        Err(BatchError::Shape {
            expected: 2,
            actual: 1
        })
    ));
    assert_eq!(executor.last_statistics(), receipt);
    assert_eq!(search.batch_statistics().completion_calls, 1);
    assert_eq!(
        search.statistics().countermodel_queries,
        before.countermodel_queries + 2
    );
    assert_eq!(search.batch_statistics().committed, 0);
    assert_eq!(search.batch_statistics().pending, 2);
    let next = retry(&mut search, id);
    assert_eq!(
        search
            .commit_slate(next, all_regional(), &mut executor)
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn malformed_region_lists_are_refused_before_any_candidate_check() {
    for (provisional, roots, expected, prefix) in [
        (vec![BatchVerdict::Residual], subset_roots(1), 2, 1),
        (all_regional(), vec![(0, Region::all_open(1))], 2, 1),
        (
            all_regional(),
            vec![(1, Region::all_open(1)), (0, Region::all_open(1))],
            2,
            0,
        ),
        (
            all_regional(),
            vec![(0, subset_roots(1).remove(0).1), (0, Region::all_open(1))],
            2,
            1,
        ),
        (
            all_regional(),
            vec![(0, Region::all_open(0)), (1, Region::all_open(1))],
            2,
            0,
        ),
        (
            all_regional(),
            vec![(0, Region::all_open(1)), (1, Region::all_open(1))],
            2,
            0,
        ),
    ] {
        let (mut search, ticket, _) = conditional_slate();
        let id = ticket.slate();
        let before = search.statistics();
        let mut executor = CompletionExecutor::default();
        assert!(
            matches!(search.begin_regional_pass(ticket,provisional,roots,&mut executor),
            Err(BatchError::Shape{expected: e, actual:a}) if (e,a)==(expected,prefix))
        );
        assert_eq!(search.statistics(), before);
        assert!(executor.last_statistics().is_none());
        assert_eq!(search.batch_statistics().committed, 0);
        assert_eq!(search.batch_statistics().completion_calls, 0);
        assert_eq!(search.batch_statistics().pending, 2);
        let same = retry(&mut search, id);
        assert_eq!(search.slate(&same).unwrap().candidates().len(), 2);
    }
}

#[test]
fn finish_refuses_a_global_assertion_contradicting_a_validated_countermodel() {
    let (mut search, ticket, present) = conditional_slate();
    let id = ticket.slate();
    let mut executor = CompletionExecutor::default();
    let pass = search
        .begin_regional_pass(ticket, all_regional(), subset_roots(present), &mut executor)
        .unwrap();
    assert_eq!(
        pass.outcomes().nth(present),
        Some((present, RegionalOutcome::ValidatedCountermodel))
    );
    assert!(matches!(pass.finish(vec![BatchVerdict::NoProperSubset;2]),
        Err(BatchError::Shape{expected:2,actual}) if actual==present));
    assert_eq!(search.batch_statistics().committed, 0);
    assert_eq!(search.batch_statistics().completion_calls, 1);
    let same = retry(&mut search, id);
    assert_eq!(search.slate(&same).unwrap().candidates().len(), 2);
}

#[test]
fn joint_not_model_precedes_regional_search_and_terminates_without_exhaustion() {
    let (mut search, ticket, _) = conditional_slate();
    let mut executor = CompletionExecutor::default();
    let before = search.statistics();
    assert!(matches!(
        search.begin_regional_pass(
            ticket,
            vec![BatchVerdict::NotModel; 2],
            vec![],
            &mut executor
        ),
        Err(BatchError::Search(Incomplete::InvalidWitness))
    ));
    assert!(executor.last_statistics().is_none());
    assert_eq!(
        search.statistics().countermodel_queries,
        before.countermodel_queries
    );
    assert_eq!(search.batch_statistics().committed, 0);
    assert_eq!(search.batch_statistics().completion_calls, 0);
    assert!(!search.exhausted());
    assert!(matches!(
        search.propose_slate(slate_limit()),
        Err(BatchError::Search(Incomplete::ClosedEnumerator))
    ));
}

#[test]
fn exhausted_cumulative_work_returns_a_typed_stop_without_publishing_a_prefix() {
    let (mut search, ticket, present) = conditional_slate();
    let before = search.statistics();
    // Keep the work already performed. The first frozen narrowing cannot
    // acquire another unit, even though the valid roots were admitted.
    search.limits.search.max_work = before.search.work;
    let mut executor = CompletionExecutor::default();
    assert!(matches!(
        search.begin_regional_pass(ticket, all_regional(), subset_roots(present), &mut executor),
        Err(BatchError::Search(Incomplete::WorkLimit))
    ));
    let receipt = executor.last_statistics().unwrap();
    assert_eq!(
        (receipt.candidates, receipt.completed, receipt.failed),
        (1, 0, 1)
    );
    assert_eq!(
        (
            receipt.residuals,
            receipt.residual_completed,
            receipt.residual_failed
        ),
        (1, 0, 1)
    );
    assert_eq!(search.statistics().search.work, before.search.work);
    assert_eq!(search.statistics().stable_models, before.stable_models);
    assert_eq!(search.batch_statistics().committed, 0);
    assert_eq!(search.batch_statistics().completion_calls, 1);
    assert!(!search.exhausted());
    assert!(matches!(
        search.propose_slate(slate_limit()),
        Err(BatchError::Search(Incomplete::ClosedEnumerator))
    ));
}
