//! A region answers a conditional question; only finish may publish the slate.
use super::*;
use crate::RegionalOutcome;
use zetesis_algebra::region::Region;
use zetesis_ferraris::{Node, TightPlanLimits};
use zetesis_theory_support::theories::theory;

fn slate_limit() -> BatchLimits {
    BatchLimits {
        max_candidates: NonZeroUsize::new(2).unwrap(),
        max_pending_bytes: 4096,
    }
}

fn conditional_slate() -> (StableModels, SlateTicket, usize) {
    // Both interpretations satisfy x → x. Its only stable model is {}.
    let owner = theory(1, vec![Node::atom(0), Node::implies(0, 0)], vec![1]);
    let mut search = StableModels::new(&owner, Limits::default(), Cancellation::default()).unwrap();
    // Membership must remain correct for a nonminimal classical proposal even
    // if today's outer candidate subjects can already prune it. Feed the
    // genuine joined-producer admission boundary the two original models.
    let available = search.prepare_pending(slate_limit()).unwrap();
    search.walk_index().unwrap();
    let mut rows = std::mem::take(&mut search.batch.pending);
    rows.push(Interpretation::new(&owner, []).unwrap());
    rows.push(Interpretation::new(&owner, [0]).unwrap());
    let produced = Produced {
        exhausted: true,
        stopped: None,
        original_validation: PhaseMeasurement::default(),
    };
    let admitted =
        search.admit_produced(&produced, available, OriginalCheck::Independent, &mut rows);
    assert_eq!(admitted.stop, None);
    assert!(admitted.exhausted);
    search.batch.pending = rows;
    search.batch.exhausted = admitted.exhausted;
    let SlateProposal::Retry(ticket) = search.propose_slate(slate_limit()).unwrap() else {
        panic!("expected the already admitted pending rows");
    };
    assert_eq!(search.slate(&ticket).unwrap().candidates().len(), 2);
    (search, ticket, 1)
}

fn subset_roots(present: usize) -> Vec<(usize, Region)> {
    (0..2)
        .map(|slot| {
            let mut root = Region::all_open(1);
            if slot != present {
                assert!(root.cut(0));
            }
            (slot, root)
        })
        .collect()
}

fn all_regional() -> Vec<BatchVerdict> {
    vec![BatchVerdict::Residual; 2]
}

fn decisions(present: usize) -> Vec<BatchVerdict> {
    (0..2)
        .map(|slot| {
            if slot == present {
                BatchVerdict::Refuted
            } else {
                BatchVerdict::NoProperSubset
            }
        })
        .collect()
}

fn retry(search: &mut StableModels, id: zetesis_ferraris::SlateId) -> SlateTicket {
    let SlateProposal::Retry(ticket) = search.propose_slate(slate_limit()).unwrap() else {
        panic!("expected the original uncommitted slate");
    };
    assert_eq!(ticket.slate(), id);
    ticket
}

#[test]
fn self_selected_region_can_exhaust_while_the_full_query_has_a_countermodel() {
    let (mut search, ticket, present) = conditional_slate();
    let id = ticket.slate();
    let mut provisional = vec![BatchVerdict::NoProperSubset; 2];
    provisional[present] = BatchVerdict::Residual;
    let mut only_candidate = Region::all_open(1);
    assert!(only_candidate.hold(0));
    let before = search.statistics();
    let mut executor = CompletionExecutor::default();
    let pass = search
        .begin_regional_pass(
            ticket,
            provisional,
            vec![(present, only_candidate)],
            &mut executor,
        )
        .unwrap();
    assert_eq!(
        pass.outcomes().collect::<Vec<_>>(),
        [(present, RegionalOutcome::NoWitnessInSuppliedRegion)]
    );
    drop(pass);
    let abandoned = search.statistics();
    assert_eq!(search.batch_statistics().committed, 0);
    assert_eq!(search.batch_statistics().pending, 2);
    assert_eq!(abandoned.stable_models, before.stable_models);
    assert_eq!(
        abandoned.countermodel_queries,
        before.countermodel_queries + 1
    );
    assert!(abandoned.search.work > before.search.work);
    assert_eq!(search.batch_statistics().completion_calls, 1);
    let again = retry(&mut search, id);
    let pass = search
        .begin_regional_pass(again, all_regional(), subset_roots(present), &mut executor)
        .unwrap();
    let expected: Vec<_> = (0..2)
        .map(|slot| {
            (
                slot,
                if slot == present {
                    RegionalOutcome::ValidatedCountermodel
                } else {
                    RegionalOutcome::NoWitnessInSuppliedRegion
                },
            )
        })
        .collect();
    assert_eq!(pass.outcomes().collect::<Vec<_>>(), expected);
    let stable = pass.finish(decisions(present)).unwrap();
    assert_eq!(stable.len(), 1);
    assert!(stable[0].atoms().next().is_none());
    assert_eq!(
        search.statistics().countermodel_queries,
        before.countermodel_queries + 3
    );
    assert_eq!(search.statistics().countermodels, before.countermodels + 1);
    assert_eq!(search.batch_statistics().completion_calls, 2);
    assert_eq!(search.batch_statistics().committed, 2);
}

#[test]
fn scalar_and_joined_workers_retain_slot_ownership_and_publish_each_charge_once() {
    for workers in [1, 2] {
        let (mut search, ticket, present) = conditional_slate();
        let mut executor = CompletionExecutor::new(NonZeroUsize::new(workers).unwrap()).unwrap();
        let before = search.statistics();
        let pass = search
            .begin_regional_pass(ticket, all_regional(), subset_roots(present), &mut executor)
            .unwrap();
        let receipt = executor.last_statistics().unwrap();
        assert_eq!(
            (receipt.workers, receipt.effective_workers),
            (workers, workers)
        );
        assert_eq!(
            (
                receipt.candidates,
                receipt.residuals,
                receipt.completed,
                receipt.residual_completed
            ),
            (2, 2, 2, 2)
        );
        assert_eq!((receipt.failed, receipt.residual_failed), (0, 0));
        let results: Vec<_> = pass.outcomes().collect();
        assert_eq!((results[0].0, results[1].0), (0, 1));
        assert_eq!(results[present].1, RegionalOutcome::ValidatedCountermodel);
        assert_eq!(
            results[1 - present].1,
            RegionalOutcome::NoWitnessInSuppliedRegion
        );
        let models = pass.finish(decisions(present)).unwrap();
        assert_eq!(models.len(), 1);
        assert!(models[0].atoms().next().is_none());
        let stats = search.statistics();
        assert_eq!(stats.countermodel_queries - before.countermodel_queries, 2);
        assert_eq!(stats.countermodels - before.countermodels, 1);
        assert!(stats.search.work > before.search.work);
        let batch = search.batch_statistics();
        assert_eq!(
            (
                batch.completion_calls,
                batch.committed,
                batch.residuals,
                batch.propagated
            ),
            (1, 2, 2, 0)
        );
        assert_eq!(executor.last_statistics(), Some(receipt));
    }
}

#[test]
fn cpu_certificate_keeps_its_decision_and_supersedes_the_submitted_region() {
    let owner = theory(1, vec![Node::atom(0)], vec![0]);
    let mut search = StableModels::new(&owner, Limits::default(), Cancellation::default()).unwrap();
    assert!(
        search
            .enable_certified_checking(TightPlanLimits::default())
            .unwrap()
    );
    let ticket = new_slate(&mut search, one_candidate());
    assert!(search.slate(&ticket).unwrap().candidates()[0].contains(0));
    let before = search.statistics();
    let mut executor = CompletionExecutor::default();
    let pass = search
        .begin_regional_pass(
            ticket,
            vec![BatchVerdict::Residual],
            vec![(0, Region::all_open(1))],
            &mut executor,
        )
        .unwrap();
    assert_eq!(pass.outcomes().count(), 0);
    let accepted = pass.finish(vec![]).unwrap();
    assert_eq!(accepted.len(), 1);
    assert!(accepted[0].contains(0));
    let after = search.statistics();
    assert_eq!(after.countermodel_queries, before.countermodel_queries);
    assert_eq!(
        after.certified.unwrap().checks,
        before.certified.unwrap().checks + 1
    );
    let receipt = executor.last_statistics().unwrap();
    assert_eq!(
        (
            receipt.candidates,
            receipt.completed,
            receipt.residual_completed,
            receipt.effective_workers
        ),
        (1, 1, 0, 0)
    );
}

#[test]
fn ordinary_verdict_layout_and_whole_root_membership_keep_their_existing_contract() {
    fn copy_value<T: Copy>(v: T) -> T {
        v
    }
    assert_eq!(size_of::<BatchVerdict>(), 1);
    assert_eq!(copy_value(BatchVerdict::Residual), BatchVerdict::Residual);
    let (mut search, ticket, _) = conditional_slate();
    let row = (2 * size_of::<Interpretation>() + size_of::<u64>() + 1 + 1) as u128;
    assert_eq!(search.pending_row_bytes(), row);
    assert_eq!(
        search.pending_batch_bytes().unwrap(),
        row * search.held_rows() as u128 + search.batch.validation.retained_bytes()
    );
    let models = search
        .commit_slate(ticket, all_regional(), &mut CompletionExecutor::default())
        .unwrap();
    assert_eq!(models.len(), 1);
    assert!(models[0].atoms().next().is_none());
}

#[test]
fn complete_region_passes_preserve_the_full_family_and_reduct_work_of_legacy_completion() {
    let theory = choice_beside_a_loop();
    let open = || StableModels::new(&theory, Limits::default(), Cancellation::default()).unwrap();
    let mut legacy = open();
    let mut regional = open();
    let mut full = CompletionExecutor::default();
    let mut scoped = CompletionExecutor::default();
    let mut answers = Vec::new();
    loop {
        let reference = legacy.propose_slate(slate_limit()).unwrap();
        let observed = regional.propose_slate(slate_limit()).unwrap();
        let (expected, pending) = match (reference, observed) {
            (SlateProposal::Exhausted, SlateProposal::Exhausted) => break,
            (SlateProposal::New(a), SlateProposal::New(b)) => (a, b),
            other => panic!("whole and regional proposals differ: {other:?}"),
        };
        let left = legacy.slate(&expected).unwrap().candidates();
        let right = regional.slate(&pending).unwrap().candidates();
        let atoms = |rows: &[Interpretation]| {
            rows.iter()
                .map(|row| row.atoms().collect::<Vec<_>>())
                .collect::<Vec<_>>()
        };
        assert_eq!(atoms(left), atoms(right));
        let roots = right
            .iter()
            .enumerate()
            .map(|(slot, candidate)| {
                let mut root = Region::all_open(theory.atom_count());
                for atom in 0..theory.atom_count() {
                    if !candidate.contains(atom) {
                        assert!(root.cut(atom));
                    }
                }
                (slot, root)
            })
            .collect();
        let tags = vec![BatchVerdict::Residual; right.len()];
        let whole = legacy
            .commit_slate(expected, tags.clone(), &mut full)
            .unwrap();
        let pass = regional
            .begin_regional_pass(pending, tags, roots, &mut scoped)
            .unwrap();
        let decisions = pass
            .outcomes()
            .map(|(_, outcome)| match outcome {
                RegionalOutcome::ValidatedCountermodel => BatchVerdict::Refuted,
                RegionalOutcome::NoWitnessInSuppliedRegion => BatchVerdict::NoProperSubset,
            })
            .collect();
        let committed = pass.finish(decisions).unwrap();
        assert_eq!(atoms(&whole), atoms(&committed));
        assert_eq!(legacy.statistics().search, regional.statistics().search);
        assert_eq!(legacy.statistics().reduct, regional.statistics().reduct);
        assert_eq!(full.last_statistics(), scoped.last_statistics());
        answers.extend(atoms(&committed));
    }
    answers.sort();
    assert_eq!(answers, [vec![0, 1], vec![0, 2], vec![1], vec![2]]);
    assert!(legacy.exhausted() && regional.exhausted());
}

mod refusals;
