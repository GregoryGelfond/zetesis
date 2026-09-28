//! Joined parallel membership shares one solve budget and preserves ordered coverage.

use std::collections::BTreeSet;
use std::convert::Infallible;
use std::num::NonZeroUsize;

use zetesis_ferraris::{AdmissionLimits, Interpretation, Node, Theory};
use zetesis_sat::{
    BatchError, BatchLimits, BatchVerdict, Cancellation, CompletionExecutor, CompletionScratch,
    Incomplete, Limits, PreparedReduct, ReductPreparationLimits, SearchLimits, StableModels,
};

/// Enumerate by the clause forms, the subject of the tests below.
fn by_clauses(
    theory: &zetesis_ferraris::Theory,
    limits: zetesis_sat::Limits,
    cancellation: zetesis_sat::Cancellation,
) -> Result<zetesis_sat::StableModels, zetesis_sat::Incomplete> {
    zetesis_sat::StableModels::with_method(
        theory,
        zetesis_sat::SearchMethod::Clauses,
        limits,
        cancellation,
    )
}

fn theory(atoms: usize, nodes: Vec<Node>, roots: Vec<usize>) -> Theory {
    Theory::new(atoms, nodes, roots, AdmissionLimits::default()).unwrap()
}

fn choices() -> Theory {
    theory(
        3,
        vec![
            Node::False,
            Node::Atom(0),
            Node::Atom(1),
            Node::Atom(2),
            Node::Implies(1, 0),
            Node::Or(1, 4),
            Node::Implies(2, 0),
            Node::Or(2, 6),
            Node::Implies(3, 0),
            Node::Or(3, 8),
        ],
        vec![5, 7, 9],
    )
}

fn executor(workers: usize) -> CompletionExecutor {
    CompletionExecutor::new(NonZeroUsize::new(workers).unwrap()).unwrap()
}

fn prepared_requirements(theory: &Theory, candidates: usize) -> CompletionScratch {
    PreparedReduct::prepare(
        theory,
        ReductPreparationLimits::default(),
        &Cancellation::default(),
    )
    .result
    .unwrap()
    .scratch_requirements(candidates)
    .unwrap()
}

fn retained_query_bytes(theory: &Theory, candidates: usize) -> u64 {
    let required = prepared_requirements(theory, candidates);
    let mut search = by_clauses(theory, Limits::default(), Cancellation::default()).unwrap();
    let mut executor = executor(1);
    search
        .next_batch_with_completion(batch(candidates), &mut executor, residual)
        .unwrap();
    let actual = executor.last_statistics().unwrap();
    assert_eq!(
        actual.requested_scratch_bytes,
        required.shared_bytes + required.result_bytes + required.query_bytes
    );
    assert!(actual.peak_scratch_bytes >= actual.requested_scratch_bytes);
    actual.peak_scratch_bytes - required.result_bytes - required.shared_bytes
}

fn batch(count: usize) -> BatchLimits {
    BatchLimits {
        max_candidates: NonZeroUsize::new(count).unwrap(),
        max_pending_bytes: 1024 * 1024,
    }
}

fn residual(
    _: &Theory,
    candidates: &[Interpretation],
) -> Result<Vec<BatchVerdict>, std::collections::TryReserveError> {
    let mut verdicts = Vec::new();
    verdicts.try_reserve_exact(candidates.len())?;
    verdicts.resize(candidates.len(), BatchVerdict::Residual);
    Ok(verdicts)
}

fn atoms(models: Vec<Interpretation>) -> Vec<Vec<usize>> {
    models
        .into_iter()
        .map(|model| model.atoms().collect())
        .collect()
}

fn collect(
    t: &Theory,
    count: usize,
    executor: &mut CompletionExecutor,
) -> (Vec<Vec<usize>>, zetesis_sat::Statistics) {
    let mut search = by_clauses(t, Limits::default(), Cancellation::default()).unwrap();
    let mut result = Vec::new();
    while !search.exhausted() {
        result.extend(atoms(
            search
                .next_batch_with_completion(batch(count), executor, residual)
                .unwrap(),
        ));
        assert_eq!(search.batch_statistics().pending, 0);
    }
    assert_eq!(
        search.statistics().candidates,
        search.batch_statistics().committed
    );
    (result, search.statistics())
}

// Worker chunks can retain different moved watches and perform different cold
// indexing, undo, propagation and branching work. Compare the semantic receipts
// across routes; exact operational counters require the same chunk schedule.
fn assert_same_family_accounting(
    actual: &(Vec<Vec<usize>>, zetesis_sat::Statistics),
    reference: &(Vec<Vec<usize>>, zetesis_sat::Statistics),
) {
    assert_eq!(actual.0, reference.0);
    let (actual, reference) = (&actual.1, &reference.1);
    assert_eq!(actual.projections, reference.projections);
    assert_eq!(
        (
            actual.candidate_queries,
            actual.candidate_restrictions,
            actual.candidates,
            actual.countermodel_queries,
            actual.countermodels,
            actual.stable_models
        ),
        (
            reference.candidate_queries,
            reference.candidate_restrictions,
            reference.candidates,
            reference.countermodel_queries,
            reference.countermodels,
            reference.stable_models
        ),
    );
    assert_eq!(actual.support, reference.support);
    assert_eq!(actual.certified, reference.certified);
    assert_eq!(actual.reduct.preparation, reference.reduct.preparation);
    assert_eq!(actual.reduct.original_work, reference.reduct.original_work);
    assert_eq!(
        actual.reduct.parameter_work,
        reference.reduct.parameter_work
    );
    // These fixtures admit the same shape, so retained capacity must agree too.
    assert_eq!(
        actual.reduct.peak_workspace_bytes,
        reference.reduct.peak_workspace_bytes
    );
}

#[test]
fn parallel_completion_matches_reference_and_scalar_order_across_reused_theories_and_batches() {
    let mut parallel = executor(3);
    assert_eq!(parallel.workers(), 3);
    assert_eq!(parallel.last_statistics(), None);
    let mut scalar = CompletionExecutor::default();
    assert_eq!(scalar.workers(), 1);
    for left in 0..3 {
        for right in 0..3 {
            for connective in [
                Node::And(left, right),
                Node::Or(left, right),
                Node::Implies(left, right),
            ] {
                for roots in [vec![], vec![3], vec![4], vec![3, 4]] {
                    let t = theory(
                        2,
                        vec![
                            Node::Atom(0),
                            Node::Atom(1),
                            Node::False,
                            connective,
                            Node::Implies(3, 2),
                        ],
                        roots,
                    );
                    let expected: BTreeSet<Vec<_>> = (0..4)
                        .filter_map(|mask| {
                            let candidate = Interpretation::new(
                                &t,
                                (0..2).filter(|atom| mask & (1 << atom) != 0),
                            )
                            .unwrap();
                            zetesis_ferraris::check(
                                &t,
                                &candidate,
                                zetesis_ferraris::Limits::default(),
                                &Cancellation::default(),
                            )
                            .unwrap()
                            .accepted()
                            .then(|| candidate.atoms().collect())
                        })
                        .collect();
                    let actual = collect(&t, 3, &mut parallel);
                    assert_same_family_accounting(&actual, &collect(&t, 3, &mut scalar));
                    assert_eq!(actual.0.into_iter().collect::<BTreeSet<_>>(), expected);
                }
            }
        }
    }
    for t in [
        choices(),
        theory(0, vec![], vec![]),
        theory(0, vec![Node::False], vec![0]),
    ] {
        for count in [1, 3, 8, 32] {
            assert_same_family_accounting(
                &collect(&t, count, &mut parallel),
                &collect(&t, count, &mut scalar),
            );
        }
    }
}

fn partial(limits: Limits, executor: &mut CompletionExecutor) -> StableModels {
    let mut search = by_clauses(&choices(), limits, Cancellation::default()).unwrap();
    let result = search.next_batch_with_completion(batch(3), executor, |_, candidates| {
        assert_eq!(candidates.len(), 3);
        Err::<Vec<BatchVerdict>, _>("retain proposals before completion")
    });
    assert!(matches!(result, Err(BatchError::Checker(_))));
    search
}

#[test]
fn shared_work_ceiling_counts_preparation_before_residual_workers() {
    // Measure the exact same four-worker, three-candidate cold prestate that
    // each limited replay uses. A one-worker chunk has a different warm cost.
    let mut parallel = executor(4);
    let mut complete = partial(Limits::default(), &mut parallel);
    let proposed = complete.statistics().search;
    assert_eq!(
        complete
            .next_batch_with_completion(batch(3), &mut parallel, residual)
            .unwrap()
            .len(),
        3
    );
    let finished = complete.statistics().search;
    let cold = complete.statistics().reduct.preparation.unwrap();
    assert!(cold.work > 0);
    assert!(finished.work > proposed.work);
    let progress = parallel.last_statistics().unwrap();
    assert_eq!((progress.workers, progress.effective_workers), (4, 3));
    for repeat in 0..8 {
        for extra in [
            0,
            1,
            cold.work - 1,
            cold.work,
            (finished.work - proposed.work) / 2,
            finished.work - proposed.work - 1,
            finished.work - proposed.work,
        ] {
            let ceiling = proposed.work + extra;
            let limits = Limits {
                search: SearchLimits {
                    max_work: ceiling,
                    ..SearchLimits::default()
                },
                ..Limits::default()
            };
            let mut search = partial(limits, &mut parallel);
            let result = search.next_batch_with_completion(batch(3), &mut parallel, residual);
            assert_eq!(
                search.statistics().search.work,
                ceiling,
                "repeat {repeat}, extra {extra}"
            );
            if ceiling == finished.work {
                assert_eq!(result.unwrap().len(), 3);
                assert_eq!(search.statistics().search, finished);
                assert_eq!(search.batch_statistics().pending, 0);
            } else {
                assert!(matches!(
                    result,
                    Err(BatchError::Search(Incomplete::WorkLimit))
                ));
                assert_eq!(search.batch_statistics().pending, 3);
                assert_eq!(search.batch_statistics().committed, 0);
                assert_eq!(search.statistics().stable_models, 0);
                assert!(!search.exhausted());
                let progress = parallel.last_statistics().unwrap();
                let preparation = search.statistics().reduct.preparation.unwrap();
                if extra < cold.work {
                    assert_eq!(preparation.work, extra);
                    assert_eq!(preparation.retained_bytes, 0);
                    assert_eq!(
                        (
                            progress.effective_workers,
                            progress.candidates,
                            progress.completed,
                            progress.failed
                        ),
                        (0, 0, 0, 0)
                    );
                } else {
                    assert_eq!(preparation, cold);
                    assert_eq!(progress.candidates, 3);
                    assert_eq!(progress.completed + progress.failed, 3);
                    assert!(progress.failed > 0);
                }
            }
        }
    }
}

#[test]
fn shared_decision_ceiling_includes_proposals_and_residuals() {
    let mut scalar = executor(1);
    let mut parallel = executor(4);
    // Independent unconstrained atoms require proper-subset branching. Find a
    // measured batch with reduct decisions before testing its shared decision cap.
    let t = theory(5, vec![], vec![]);
    let mut measured = by_clauses(&t, Limits::default(), Cancellation::default()).unwrap();
    let _ = measured.next_batch(batch(16), |_, _| Err::<Vec<BatchVerdict>, _>("measure"));
    let proposed_decisions = measured.statistics().search.decisions;
    measured
        .next_batch_with_completion(batch(16), &mut scalar, residual)
        .unwrap();
    let total = measured.statistics().search.decisions;
    assert!(total > proposed_decisions);
    for ceiling in [proposed_decisions, total - 1, total] {
        let limits = Limits {
            search: SearchLimits {
                max_decisions: ceiling,
                ..SearchLimits::default()
            },
            ..Limits::default()
        };
        let mut search = by_clauses(&t, limits, Cancellation::default()).unwrap();
        let result = search.next_batch_with_completion(batch(16), &mut parallel, residual);
        assert_eq!(search.statistics().search.decisions, ceiling);
        if ceiling == total {
            assert_eq!(atoms(result.unwrap()), vec![Vec::<usize>::new()]);
        } else {
            assert!(matches!(
                result,
                Err(BatchError::Search(Incomplete::DecisionLimit))
            ));
            assert_eq!(search.batch_statistics().pending, 16);
            assert_eq!(search.statistics().stable_models, 0);
        }
    }
}

#[test]
fn checker_retry_restriction_and_failed_certificate_preserve_the_owned_batch() {
    let mut pool = executor(3);
    let mut search = partial(Limits::default(), &mut pool);
    assert_eq!(pool.last_statistics(), None);
    let original = search.theory().clone();
    let restriction = theory(3, vec![Node::False], vec![0]);
    search.restrict_candidates(&restriction).unwrap();
    let result = search.next_batch_with_completion(batch(3), &mut pool, |theory, candidates| {
        assert!(theory.same_instance(&original));
        for candidate in candidates {
            assert!(
                zetesis_ferraris::check(
                    theory,
                    candidate,
                    zetesis_ferraris::Limits::default(),
                    &Cancellation::default()
                )
                .unwrap()
                .accepted()
            );
        }
        Ok::<_, Infallible>(vec![
            BatchVerdict::NoProperSubset,
            BatchVerdict::NotModel,
            BatchVerdict::Residual,
        ])
    });
    assert!(matches!(
        result,
        Err(BatchError::Search(Incomplete::InvalidWitness))
    ));
    assert_eq!(search.batch_statistics().pending, 3);
    assert_eq!(search.batch_statistics().committed, 0);
    assert_eq!(search.statistics().stable_models, 0);
    let progress = pool.last_statistics().unwrap();
    assert_eq!(progress.completed, 2);
    assert_eq!(progress.failed, 1);
    assert!(!search.exhausted());
    assert!(matches!(
        search.next_batch_with_completion(batch(3), &mut pool, residual),
        Err(BatchError::Search(Incomplete::ClosedEnumerator))
    ));

    let mut retry = partial(Limits::default(), &mut pool);
    retry.restrict_candidates(&restriction).unwrap();
    assert_eq!(
        retry
            .next_batch_with_completion(batch(3), &mut pool, residual)
            .unwrap()
            .len(),
        3
    );
    assert!(
        retry
            .next_batch_with_completion(batch(3), &mut pool, residual)
            .unwrap()
            .is_empty()
    );
    assert!(retry.exhausted());
}

#[test]
fn cancellation_before_workspace_reservation_keeps_pending_candidates() {
    let cancellation = Cancellation::default();
    let mut search = by_clauses(&choices(), Limits::default(), cancellation.clone()).unwrap();
    let mut pool = executor(4);
    let result = search.next_batch_with_completion(batch(3), &mut pool, |theory, candidates| {
        cancellation.cancel();
        residual(theory, candidates)
    });
    assert!(matches!(
        result,
        Err(BatchError::Search(Incomplete::Cancelled))
    ));
    assert_eq!(search.batch_statistics().pending, 3);
    assert_eq!(search.statistics().stable_models, 0);
    assert_eq!(pool.last_statistics().unwrap().candidates, 0);
    assert_eq!(pool.last_statistics().unwrap().failed, 0);
    assert_eq!(pool.last_statistics().unwrap().completed, 0);
    // The same pool is reusable after the caller's cancelled solve.
    assert_eq!(collect(&choices(), 3, &mut pool).0.len(), 8);
}

#[test]
fn opt_in_worker_sums_do_not_replace_scalar_search_wall_intervals() {
    for workers in [1, 3] {
        let mut pool = executor(workers);
        let mut search =
            by_clauses(&choices(), Limits::default(), Cancellation::default()).unwrap();
        search.enable_phase_timing();
        search
            .next_batch_with_completion(batch(3), &mut pool, residual)
            .unwrap();
        let progress = pool.last_statistics().unwrap();
        assert!(progress.elapsed.is_some());
        assert_eq!(progress.residuals, 3);
        let timing = search.statistics().phase_timings.unwrap();
        if workers == 1 {
            assert_eq!(progress.worker_original_validation, None);
            assert_eq!(progress.worker_reduct, None);
            assert_eq!(timing.original_validation.calls, 6);
            assert_eq!(timing.reduct.calls, 3);
        } else {
            assert_eq!(progress.worker_original_validation.unwrap().calls, 3);
            assert_eq!(progress.worker_reduct.unwrap().calls, 3);
            assert_eq!(timing.original_validation.calls, 3);
            assert_eq!(timing.reduct.calls, 0);
        }
        collect(&choices(), 3, &mut pool);
        assert_eq!(pool.last_statistics().unwrap().elapsed, None);
        assert_eq!(pool.last_statistics().unwrap().worker_reduct, None);
    }
}

#[test]
fn scratch_admission_precedes_results_and_is_retryable_without_new_proposals() {
    let t = choices();
    for workers in [1, 2, 4] {
        let mut search = by_clauses(&t, Limits::default(), Cancellation::default()).unwrap();
        let mut refused =
            CompletionExecutor::with_scratch_limit(NonZeroUsize::new(workers).unwrap(), 0).unwrap();
        assert_eq!(refused.scratch_limit(), 0);
        let first = search.next_batch_with_completion(batch(3), &mut refused, residual);
        assert!(matches!(
            first,
            Err(BatchError::Limits(Incomplete::CompletionScratch))
        ));
        assert_eq!(search.batch_statistics().pending, 3);
        assert_eq!(search.batch_statistics().committed, 0);
        assert_eq!(refused.last_statistics().unwrap().candidates, 0);
        assert_eq!(refused.last_statistics().unwrap().peak_scratch_bytes, 0);
        let before = search.statistics();
        let required = prepared_requirements(&t, 3);
        let mut results_only = CompletionExecutor::with_scratch_limit(
            NonZeroUsize::new(workers).unwrap(),
            required.result_bytes,
        )
        .unwrap();
        assert!(matches!(
            search.next_batch_with_completion(batch(3), &mut results_only, residual),
            Err(BatchError::Limits(Incomplete::CompletionScratch))
        ));
        let prepared = search.statistics();
        let cold = prepared.reduct.preparation.unwrap();
        assert_eq!(prepared.search.work, before.search.work + cold.work);
        assert_eq!(prepared.countermodel_queries, before.countermodel_queries);
        assert_eq!(prepared.candidate_queries, before.candidate_queries);
        let mut admitted = CompletionExecutor::with_scratch_limit(
            NonZeroUsize::new(workers).unwrap(),
            required.shared_bytes + required.result_bytes + retained_query_bytes(&t, 3),
        )
        .unwrap();
        let found = search
            .next_batch_with_completion(batch(3), &mut admitted, residual)
            .unwrap();
        assert_eq!(found.len(), 3);
        assert_eq!(
            search.statistics().candidate_queries,
            before.candidate_queries
        );
        let progress = admitted.last_statistics().unwrap();
        assert_eq!((progress.workers, progress.effective_workers), (workers, 1));
        assert_eq!(
            progress.peak_scratch_bytes,
            required.shared_bytes + required.result_bytes + retained_query_bytes(&t, 3)
        );
        assert_eq!(
            (progress.residual_completed, progress.residual_failed),
            (3, 0)
        );
        assert_eq!(search.batch_statistics().pending, 0);
    }
}

#[test]
fn prepared_owner_survives_capacity_refusal_and_retry() {
    let theory = choices();
    let requested = prepared_requirements(&theory, 3);
    let actual = requested.shared_bytes + requested.result_bytes + retained_query_bytes(&theory, 3);
    let mut search = by_clauses(&theory, Limits::default(), Cancellation::default()).unwrap();
    let _ = search.next_batch(batch(3), |_, _| {
        Err::<Vec<BatchVerdict>, _>("retain proposals")
    });
    let before = search.statistics();
    let mut refused =
        CompletionExecutor::with_scratch_limit(NonZeroUsize::new(1).unwrap(), actual - 1).unwrap();
    assert!(matches!(
        search.next_batch_with_completion(batch(3), &mut refused, residual),
        Err(BatchError::Limits(Incomplete::CompletionScratch))
    ));
    let progress = refused.last_statistics().unwrap();
    assert_eq!(
        (progress.candidates, progress.completed, progress.failed),
        (0, 0, 0)
    );
    let prepared = search.statistics();
    let cold = prepared.reduct.preparation.unwrap();
    assert_eq!(prepared.search.work, before.search.work + cold.work);
    assert_eq!(prepared.countermodel_queries, 0);
    assert_eq!(search.batch_statistics().pending, 3);
    let mut exact =
        CompletionExecutor::with_scratch_limit(NonZeroUsize::new(1).unwrap(), actual).unwrap();
    assert_eq!(
        search
            .next_batch_with_completion(batch(3), &mut exact, residual)
            .unwrap()
            .len(),
        3
    );
    assert_eq!(exact.last_statistics().unwrap().peak_scratch_bytes, actual);
    assert_eq!(
        search.statistics().reduct.preparation,
        Some(cold),
        "retry must not reconstruct the owner or reset cold work"
    );
    assert_eq!(
        search.statistics().candidate_queries,
        before.candidate_queries
    );
    assert_eq!(search.batch_statistics().pending, 0);
}

#[test]
fn fixed_scratch_envelopes_cap_concurrency_and_release_between_irregular_calls() {
    let t = choices();
    for workers in [1, 2, 4] {
        for admitted in [1, 2, 4] {
            let requirements = prepared_requirements(&t, 3);
            let ceiling = requirements.shared_bytes
                + requirements.result_bytes
                + retained_query_bytes(&t, 3) * admitted;
            let mut pool = CompletionExecutor::with_scratch_limit(
                NonZeroUsize::new(workers).unwrap(),
                ceiling,
            )
            .unwrap();
            let reference = collect(&t, 3, &mut executor(1));
            let mut previous = None;
            for _ in 0..2 {
                let actual = collect(&t, 3, &mut pool);
                assert_same_family_accounting(&actual, &reference);
                if let Some(previous) = &previous {
                    // Same worker/byte envelope and batch sequence: retain the
                    // exact work, decisions, propagations and conflict receipt.
                    assert_eq!(&actual, previous);
                }
                previous = Some(actual);
                let progress = pool.last_statistics().unwrap();
                assert!(progress.peak_scratch_bytes <= ceiling);
                assert!(
                    progress.effective_workers <= workers.min(usize::try_from(admitted).unwrap())
                );
            }
        }
    }
}

#[test]
fn certificate_only_completion_needs_result_storage_but_no_query_workspace() {
    let t = choices();
    let requirements = prepared_requirements(&t, 3);
    for workers in [1, 2, 4] {
        let mut search = by_clauses(&t, Limits::default(), Cancellation::default()).unwrap();
        let mut pool = CompletionExecutor::with_scratch_limit(
            NonZeroUsize::new(workers).unwrap(),
            requirements.result_bytes,
        )
        .unwrap();
        let models = search
            .next_batch_with_completion(batch(3), &mut pool, |_, candidates| {
                Ok::<_, Infallible>(vec![BatchVerdict::NoProperSubset; candidates.len()])
            })
            .unwrap();
        assert_eq!(models.len(), 3);
        let statistics = pool.last_statistics().unwrap();
        assert_eq!((statistics.effective_workers, statistics.residuals), (0, 0));
        assert_eq!(statistics.peak_scratch_bytes, requirements.result_bytes);
        assert_eq!(statistics.completed, 3);
        assert_eq!(search.statistics().countermodel_queries, 0);
    }
}

#[test]
fn one_workspace_parallel_failure_joins_all_residual_slots_and_accounts_each() {
    let t = choices();
    let required = prepared_requirements(&t, 3);
    let mut probe = by_clauses(&t, Limits::default(), Cancellation::default()).unwrap();
    let _ = probe.next_batch(batch(3), |_, _| {
        Err::<Vec<BatchVerdict>, _>("retain proposals")
    });
    let mut limits = Limits::default();
    limits.search.max_work = probe.statistics().search.work
        + PreparedReduct::prepare(
            &t,
            ReductPreparationLimits::default(),
            &Cancellation::default(),
        )
        .result
        .unwrap()
        .statistics()
        .work;
    let mut search = by_clauses(&t, limits, Cancellation::default()).unwrap();
    let mut pool = CompletionExecutor::with_scratch_limit(
        NonZeroUsize::new(4).unwrap(),
        required.shared_bytes + required.result_bytes + retained_query_bytes(&t, 3),
    )
    .unwrap();
    let result = search.next_batch_with_completion(batch(3), &mut pool, residual);
    assert!(matches!(
        result,
        Err(BatchError::Search(Incomplete::WorkLimit))
    ));
    let progress = pool.last_statistics().unwrap();
    assert_eq!(
        (
            progress.effective_workers,
            progress.candidates,
            progress.failed
        ),
        (1, 3, 3)
    );
    assert_eq!(
        (progress.residual_completed, progress.residual_failed),
        (0, 3)
    );
    assert_eq!(
        (
            search.batch_statistics().pending,
            search.batch_statistics().committed
        ),
        (3, 0)
    );
    assert_eq!(collect(&t, 3, &mut pool), collect(&t, 3, &mut executor(1)));
}

#[test]
fn independent_query_capacity_is_admitted_before_worker_entry() {
    // Many independent atom slots make the worker arrays larger than cold
    // compilation. Derive the actual cold ceiling from the real builder.
    let input = theory(64, vec![], vec![]);
    let prepared = PreparedReduct::prepare(
        &input,
        ReductPreparationLimits::default(),
        &Cancellation::default(),
    )
    .result
    .unwrap();
    let limit = u64::try_from(prepared.statistics().peak_bytes).unwrap();
    let limits = Limits {
        max_reduct_bytes: limit,
        ..Limits::default()
    };
    let mut search = by_clauses(&input, limits, Cancellation::default()).unwrap();
    let mut pool = executor(3);
    let result = search.next_batch_with_completion(batch(1), &mut pool, residual);
    assert!(
        matches!(result, Err(BatchError::Search(Incomplete::ReductStorage { required, limit: actual })) if required > actual && actual == u128::from(limit))
    );
    assert_eq!(
        search.statistics().reduct.preparation,
        Some(prepared.statistics())
    );
    let progress = pool.last_statistics().unwrap();
    assert_eq!(
        (progress.candidates, progress.completed, progress.failed),
        (0, 0, 0)
    );
    assert_eq!(search.statistics().countermodel_queries, 0);
    assert_eq!(search.batch_statistics().pending, 1);
    assert_eq!(search.batch_statistics().committed, 0);
    assert!(!search.exhausted());
}
