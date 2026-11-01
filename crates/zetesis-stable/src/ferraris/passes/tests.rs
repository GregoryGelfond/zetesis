use crate::ferraris::{Limits, StableModels};
use crate::{Cancellation, Incomplete};
use zetesis_ferraris::Node;
use zetesis_theory_support::theories::theory;

/// Three free atoms: every subset is a stable model of the choice theory.
fn choices() -> zetesis_ferraris::Theory {
    theory(
        3,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::atom(2),
            Node::falsum(),
            Node::implies(0, 3),
            Node::or_pair([0, 4]),
            Node::implies(1, 3),
            Node::or_pair([1, 6]),
            Node::implies(2, 3),
            Node::or_pair([2, 8]),
        ],
        vec![5, 7, 9],
    )
}

fn exhausted(theory: &zetesis_ferraris::Theory) -> StableModels {
    let mut stream = StableModels::new(theory, Limits::default(), Cancellation::default()).unwrap();
    for model in stream.by_ref() {
        model.unwrap();
    }
    assert!(stream.exhausted());
    stream.stop().unwrap();
    stream
}

#[test]
fn a_following_stream_reports_the_earlier_receipts_before_its_own() {
    let original = choices();
    let earlier = exhausted(&original);
    let mut later = exhausted(&original);
    let alone = later.statistics();
    later.follow(&earlier).unwrap();
    let both = later.statistics();
    let first = earlier.statistics();
    assert_eq!(first.stable_models, 8);
    assert_eq!(
        both.stable_models,
        first.stable_models + alone.stable_models
    );
    assert_eq!(both.candidates, first.candidates + alone.candidates);
    assert_eq!(both.search.work, first.search.work + alone.search.work);
    assert_eq!(
        both.regions.unwrap().counts.regions,
        first.regions.unwrap().counts.regions + alone.regions.unwrap().counts.regions
    );
}

#[test]
fn a_following_stream_reports_the_earlier_batch_receipts() {
    let original = choices();
    let earlier = exhausted(&original);
    let mut later = exhausted(&original);
    let alone = later.batch_statistics();
    later.follow(&earlier).unwrap();
    let both = later.batch_statistics();
    let first = earlier.batch_statistics();
    assert_eq!(
        both.original_validation_work,
        first.original_validation_work + alone.original_validation_work
    );
    assert_eq!(both.pending, alone.pending);
}

#[test]
fn a_stream_cannot_follow_one_over_another_theory_instance() {
    let earlier = exhausted(&choices());
    let mut later =
        StableModels::new(&choices(), Limits::default(), Cancellation::default()).unwrap();
    assert_eq!(later.follow(&earlier), Err(Incomplete::WrongTheory));
    assert_eq!(later.statistics().stable_models, 0);
}

#[test]
fn a_later_pass_is_left_the_cumulative_limits_the_earlier_did_not_spend() {
    let limits = Limits {
        max_candidates: 10,
        ..Limits::default()
    };
    let spent = exhausted(&choices()).statistics();
    let remaining = limits.remaining_after(&spent);
    assert_eq!(remaining.max_candidates, 10 - spent.candidates);
    assert_eq!(
        remaining.search.max_work,
        limits.search.max_work - spent.search.work
    );
    assert_eq!(
        remaining.search.max_decisions,
        limits.search.max_decisions - spent.search.decisions
    );
    // Per-call and capacity ceilings are each stream's own.
    assert_eq!(
        remaining.max_verification_work,
        limits.max_verification_work
    );
    assert_eq!(remaining.max_reduct_bytes, limits.max_reduct_bytes);
}

#[test]
fn passes_within_remaining_limits_stop_at_the_run_limit() {
    // Two exhaustive passes over eight stable models need sixteen candidates;
    // a run limited to twelve stops in the second pass.
    let original = choices();
    let limits = Limits {
        max_candidates: 12,
        ..Limits::default()
    };
    let mut earlier = StableModels::new(&original, limits, Cancellation::default()).unwrap();
    for model in earlier.by_ref() {
        model.unwrap();
    }
    earlier.stop().unwrap();
    assert_eq!(earlier.statistics().candidates, 8);
    let mut later = StableModels::new(
        &original,
        limits.remaining_after(&earlier.statistics()),
        Cancellation::default(),
    )
    .unwrap();
    later.follow(&earlier).unwrap();
    let mut found = 0;
    let stop = loop {
        match later.next() {
            Some(Ok(_)) => found += 1,
            Some(Err(stop)) => break Some(stop),
            None => break None,
        }
    };
    assert_eq!(stop, Some(Incomplete::CandidateLimit));
    assert_eq!(found, 4);
    assert!(!later.exhausted());
    assert_eq!(later.statistics().candidates, 12);
}

#[test]
fn saturated_limits_leave_nothing_to_a_later_pass() {
    let spent = exhausted(&choices()).statistics();
    let limits = Limits {
        max_candidates: 1,
        ..Limits::default()
    };
    assert_eq!(limits.remaining_after(&spent).max_candidates, 0);
}

#[test]
fn following_passes_preserve_each_runtime_arithmetic_receipt() {
    use crate::{CardinalityStatistics, ParityStatistics, RegionSearchStatistics};

    let earlier = RegionSearchStatistics {
        counts: crate::RegionCounts {
            arithmetic: crate::ferraris::regions::arithmetic_counts::ArithmeticCounts {
                parity_passes: 2,
                parity_forced: 3,
                parity_refuted: 1,
                cardinality_passes: 5,
                cardinality_refuted: 2,
            },
            ..crate::RegionCounts::default()
        },
        parity: ParityStatistics {
            region_passes: 2,
            forced: 3,
            refuted: 1,
            ..ParityStatistics::default()
        },
        cardinality: CardinalityStatistics {
            region_passes: 5,
            refuted: 2,
            ..CardinalityStatistics::default()
        },
        ..RegionSearchStatistics::default()
    };
    let later = RegionSearchStatistics {
        counts: crate::RegionCounts {
            arithmetic: crate::ferraris::regions::arithmetic_counts::ArithmeticCounts {
                parity_passes: 7,
                parity_forced: 11,
                parity_refuted: 3,
                cardinality_passes: 13,
                cardinality_refuted: 5,
            },
            ..crate::RegionCounts::default()
        },
        parity: ParityStatistics {
            region_passes: 7,
            forced: 11,
            refuted: 3,
            ..ParityStatistics::default()
        },
        cardinality: CardinalityStatistics {
            region_passes: 13,
            refuted: 5,
            ..CardinalityStatistics::default()
        },
        ..RegionSearchStatistics::default()
    };
    let both = super::regions(&earlier, later);
    assert_eq!(both.parity.region_passes, 9);
    assert_eq!(both.parity.forced, 14);
    assert_eq!(both.parity.refuted, 4);
    assert_eq!(both.cardinality.region_passes, 18);
    assert_eq!(both.cardinality.refuted, 7);
    assert_eq!(both.parity.preparation_work, 0);
    assert_eq!(both.cardinality.preparation_work, 0);
}

#[test]
fn multipass_projection_agrees_with_the_combined_private_receipt() {
    use crate::ferraris::regions::arithmetic_counts::ArithmeticCounts;
    let earlier = crate::RegionSearchStatistics {
        counts: crate::RegionCounts {
            arithmetic: ArithmeticCounts {
                parity_passes: 2,
                parity_forced: 3,
                parity_refuted: 1,
                cardinality_passes: 5,
                cardinality_refuted: 2,
            },
            ..crate::RegionCounts::default()
        },
        ..crate::RegionSearchStatistics::default()
    }
    .with_runtime_arithmetic();
    let both = super::regions(&earlier, earlier);
    let projected = both.with_runtime_arithmetic();
    assert_eq!(projected.parity, both.parity);
    assert_eq!(projected.cardinality, both.cardinality);
    assert_eq!(both.counts.arithmetic.parity_passes, 4);
    assert_eq!(both.counts.arithmetic.parity_forced, 6);
    assert_eq!(both.counts.arithmetic.parity_refuted, 2);
    assert_eq!(both.counts.arithmetic.cardinality_passes, 10);
    assert_eq!(both.counts.arithmetic.cardinality_refuted, 4);
}
