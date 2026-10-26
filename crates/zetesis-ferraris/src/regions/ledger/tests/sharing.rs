//! Tiers share rows through the store at walk boundaries: a tier delivers,
//! under its class, the rows that pass the quality filter, and seeding copies
//! the store's rows of a tier's class into the tier by retention.

use super::*;
use crate::LearningStore;
use crate::test_support::Allowance;

fn unlimited() -> Allowance {
    Allowance {
        remaining: u64::MAX,
        failure: Stop::WorkLimit,
    }
}

/// `Program::new` whose owner has `limits`.
fn limited(limits: LearningLimits) -> Program {
    let mut program = Program::new(false);
    program.owner = LearningOwner::theory(&program.theory, limits);
    program
}

fn deliver(program: &Program, scratch: &mut NarrowingScratch, store: &mut LearningStore) {
    let attempt = scratch.deliver_learning(&program.owner, store, &mut unlimited());
    assert_eq!(attempt.result, Ok(()));
}

fn seed(program: &Program, scratch: &mut NarrowingScratch, store: &mut LearningStore) {
    let attempt = scratch.seed_learning(&program.owner, store, &mut unlimited());
    assert_eq!(attempt.result, Ok(()));
}

/// The store after the captured row's delivery.
fn delivered(program: &Program) -> LearningStore {
    let (_, mut scratch) = program.captured();
    let mut store = LearningStore::new();
    deliver(program, &mut scratch, &mut store);
    store
}

#[test]
fn delivery_shares_a_row_within_the_condition_filter() {
    let store = delivered(&Program::new(false));
    assert_eq!(store.len(), 1);
    assert_eq!(store.deliveries(), 1);
}

#[test]
fn delivery_shares_a_row_at_the_condition_limit() {
    let program = limited(LearningLimits {
        share_conditions: 2,
        ..LearningLimits::default()
    });
    assert_eq!(delivered(&program).len(), 1);
}

#[test]
fn delivery_withholds_a_longer_row_without_hits() {
    let program = limited(LearningLimits {
        share_conditions: 1,
        ..LearningLimits::default()
    });
    assert!(delivered(&program).is_empty());
}

#[test]
fn delivery_shares_a_longer_row_once_it_hits() {
    let program = limited(LearningLimits {
        share_conditions: 1,
        share_hits: 1,
        ..LearningLimits::default()
    });
    let (_, mut scratch) = program.captured();
    let mut store = LearningStore::new();
    deliver(&program, &mut scratch, &mut store);
    assert!(store.is_empty());
    let mut sibling = region(&[(0, true), (2, false), (3, true), (4, true)]);
    let (result, counts) =
        program.close(&mut sibling, &mut program.index.knowledge(), &mut scratch);
    assert_eq!(result, Narrowing::Refuted);
    assert_eq!(counts.support_explanations.refuted, 1);
    deliver(&program, &mut scratch, &mut store);
    assert_eq!(store.len(), 1);
}

#[test]
fn a_delivered_row_returns_only_its_new_hits() {
    let program = Program::new(false);
    let (_, mut scratch) = program.captured();
    let mut store = LearningStore::new();
    deliver(&program, &mut scratch, &mut store);
    deliver(&program, &mut scratch, &mut store);
    assert_eq!(store.deliveries(), 1);
    assert_eq!(store.classes[0].rows[0].hits, 0);
    scratch.explanations.hit(0);
    deliver(&program, &mut scratch, &mut store);
    assert_eq!(store.deliveries(), 1);
    assert_eq!(store.len(), 1);
    assert_eq!(store.classes[0].rows[0].hits, 1);
    deliver(&program, &mut scratch, &mut store);
    assert_eq!(store.classes[0].rows[0].hits, 1);
}

#[test]
fn a_seeded_row_starts_at_the_store_hits() {
    let program = Program::new(false);
    let (_, mut source) = program.captured();
    let mut store = LearningStore::new();
    deliver(&program, &mut source, &mut store);
    source.explanations.hit(0);
    deliver(&program, &mut source, &mut store);
    let mut scratch = NarrowingScratch::default();
    seed(&program, &mut scratch, &mut store);
    assert_eq!(scratch.explanations.rows[0].hits, 1);
}

#[test]
fn seeding_lets_a_fresh_tier_reuse_a_delivered_row() {
    let program = Program::new(false);
    let mut store = delivered(&program);
    let mut scratch = NarrowingScratch::default();
    seed(&program, &mut scratch, &mut store);
    assert_eq!(scratch.learned_rows(), 1);
    assert_eq!(store.seeded(), 1);
    let mut sibling = region(&[(0, true), (2, false), (3, true), (4, true)]);
    let (result, counts) =
        program.close(&mut sibling, &mut program.index.knowledge(), &mut scratch);
    assert_eq!(result, Narrowing::Refuted);
    assert_eq!(counts.support_explanations.refuted, 1);
    assert_eq!(counts.support_explanations.attempts, 0);
}

#[test]
fn seeding_twice_adds_no_duplicate() {
    let program = Program::new(false);
    let mut store = delivered(&program);
    let mut scratch = NarrowingScratch::default();
    seed(&program, &mut scratch, &mut store);
    seed(&program, &mut scratch, &mut store);
    assert_eq!(scratch.learned_rows(), 1);
    assert_eq!(store.seeded(), 1);
}

#[test]
fn a_seeded_row_is_not_delivered_back() {
    let program = Program::new(false);
    let mut store = delivered(&program);
    let mut scratch = NarrowingScratch::default();
    seed(&program, &mut scratch, &mut store);
    let attempt = scratch.deliver_learning(&program.owner, &mut store, &mut unlimited());
    assert_eq!(attempt.result, Ok(()));
    // One read examines the seeded row, which goes nowhere.
    assert_eq!(attempt.work, 1);
    assert_eq!(store.deliveries(), 1);
    assert_eq!(store.len(), 1);
}

/// A tier of `program`'s class holding the one row `atom` held -> e absent.
fn tier_with(program: &Program, atoms: &[usize]) -> Ledger {
    let mut tier = owned(&program.theory, *program.owner.limits());
    for &atom in atoms {
        assert!(add(&mut tier, &[(atom, true)], &[(4, false)]));
    }
    tier
}

fn deliver_tier(tier: &mut Ledger, store: &mut LearningStore) {
    tier.deliver(store, &mut Work::new(u64::MAX)).unwrap();
}

fn seed_tier(tier: &mut Ledger, store: &mut LearningStore) -> u64 {
    let mut work = Work::new(u64::MAX);
    tier.seed(store, &mut work).unwrap();
    work.spent
}

#[test]
fn reseeding_reads_only_rows_retained_since() {
    let program = Program::new(false);
    let mut store = LearningStore::new();
    deliver_tier(&mut tier_with(&program, &[0]), &mut store);
    let mut tier = tier_with(&program, &[]);
    seed_tier(&mut tier, &mut store);
    deliver_tier(&mut tier_with(&program, &[1]), &mut store);
    let again = seed_tier(&mut tier, &mut store);
    assert_eq!(tier.len(), 2);
    let fresh = seed_tier(&mut tier_with(&program, &[]), &mut store);
    assert!(
        again < fresh,
        "{again} reads after a complete seeding, {fresh} fresh"
    );
}

#[test]
fn a_seeding_mark_names_its_store() {
    let program = Program::new(false);
    let mut first = LearningStore::new();
    deliver_tier(&mut tier_with(&program, &[0, 1]), &mut first);
    let mut second = LearningStore::new();
    deliver_tier(&mut tier_with(&program, &[2]), &mut second);
    let mut tier = tier_with(&program, &[]);
    seed_tier(&mut tier, &mut first);
    seed_tier(&mut tier, &mut second);
    assert_eq!(tier.len(), 3);
}

#[test]
fn seeding_reads_only_the_tiers_class() {
    let program = Program::new(false);
    let foreign = Program::new(true);
    let mut store = delivered(&program);
    let mut scratch = NarrowingScratch::default();
    seed(&foreign, &mut scratch, &mut store);
    assert_eq!(scratch.learned_rows(), 0);
    assert_eq!(store.seeded(), 0);
}

#[test]
fn a_full_store_evicts_the_row_with_fewest_hits() {
    let program = limited(LearningLimits {
        store_rows: 1,
        ..LearningLimits::default()
    });
    let mut tier = owned(&program.theory, *program.owner.limits());
    assert!(add(&mut tier, &[(0, true)], &[(4, false)]));
    assert!(add(&mut tier, &[(1, true)], &[(4, false)]));
    tier.hit(1);
    let mut store = LearningStore::new();
    tier.deliver(&mut store, &mut Work::new(u64::MAX)).unwrap();
    assert_eq!(store.len(), 1);
    assert_eq!(store.classes[0].evicted(), 1);
    assert_eq!(
        store.classes[0].rows[0]
            .row
            .conditions()
            .collect::<Vec<_>>(),
        [(1, true)]
    );
}

#[test]
fn an_exhausted_sharing_allowance_ends_delivery_not_the_run() {
    let program = limited(LearningLimits {
        share_work: 1,
        ..LearningLimits::default()
    });
    let (_, mut scratch) = program.captured();
    let mut store = LearningStore::new();
    deliver(&program, &mut scratch, &mut store);
    assert!(store.is_empty());
}

#[test]
fn an_interrupted_delivery_publishes_no_partial_row() {
    let program = Program::new(false);
    let (_, mut complete) = program.captured();
    let measured =
        complete.deliver_learning(&program.owner, &mut LearningStore::new(), &mut unlimited());
    assert!(measured.work > 0);
    for failure in [Stop::WorkLimit, Stop::Cancelled, Stop::Deadline] {
        for prefix in 0..measured.work {
            let (_, mut scratch) = program.captured();
            let mut store = LearningStore::new();
            let mut permits = SinglePermits::new(prefix, failure);
            let attempt = scratch.deliver_learning(&program.owner, &mut store, &mut permits);
            assert_eq!(attempt.result, Err(failure));
            assert_eq!(attempt.work, prefix);
            assert!(store.is_empty());
            deliver(&program, &mut scratch, &mut store);
            assert_eq!(store.len(), 1);
        }
    }
}

#[test]
fn an_interrupted_seeding_adds_no_partial_row() {
    let program = Program::new(false);
    let mut store = delivered(&program);
    let measured =
        NarrowingScratch::default().seed_learning(&program.owner, &mut store, &mut unlimited());
    assert!(measured.work > 0);
    for failure in [Stop::WorkLimit, Stop::Cancelled, Stop::Deadline] {
        for prefix in 0..measured.work {
            let mut scratch = NarrowingScratch::default();
            let mut permits = SinglePermits::new(prefix, failure);
            let attempt = scratch.seed_learning(&program.owner, &mut store, &mut permits);
            assert_eq!(attempt.result, Err(failure));
            assert_eq!(attempt.work, prefix);
            assert_eq!(scratch.learned_rows(), 0);
            seed(&program, &mut scratch, &mut store);
            assert_eq!(scratch.learned_rows(), 1);
        }
    }
}
