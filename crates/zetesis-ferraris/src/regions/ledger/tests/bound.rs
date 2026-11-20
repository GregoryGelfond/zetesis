//! Objective-bound cores are a ledger source of the bound-restricted class.

use super::{Program, region};
use crate::test_support::Allowance;
use crate::{ConflictCore, LearningLimits, LearningOwner, LearningStore, Narrowing};
use zetesis_algebra::Meter;
use zetesis_algebra::Stop;

fn program(limits: LearningLimits) -> Program {
    let mut program = Program::new(false);
    program.owner = LearningOwner::bound(&program.theory, limits);
    program
}

fn founding_disabled() -> LearningLimits {
    LearningLimits {
        founded_attempts: 0,
        conflict_core: ConflictCore::Full,
        ..LearningLimits::default()
    }
}

fn dead_end() -> [(usize, bool); 4] {
    [(0, true), (2, false), (3, true), (4, false)]
}

fn unlimited() -> Allowance {
    Allowance {
        remaining: u64::MAX,
        failure: Stop::WorkLimit,
    }
}

#[test]
fn a_refuted_bound_carrier_certifies_a_bound_row() {
    let program = program(founding_disabled());
    let mut scratch = crate::NarrowingScratch::default();
    let (result, counts) = program.close(
        &mut region(&dead_end()),
        &mut program.index.knowledge(),
        &mut scratch,
    );
    assert_eq!(result, Narrowing::Refuted);
    assert_eq!(counts.support_explanations.certified, 0);
    assert_eq!(counts.conflict_explanations.certified, 0);
    assert_eq!(counts.bound_explanations.attempts, 1);
    assert_eq!(counts.bound_explanations.certified, 1);
    assert_eq!(scratch.learning.explanations.len(), 1);
    assert_eq!(scratch.bound_rows(), 0);
}

#[test]
fn a_fresh_sibling_reuses_the_bound_row() {
    let program = program(founding_disabled());
    let mut scratch = crate::NarrowingScratch::default();
    let (result, counts) = program.close(
        &mut region(&dead_end()),
        &mut program.index.knowledge(),
        &mut scratch,
    );
    assert_eq!(result, Narrowing::Refuted);
    assert_eq!(counts.bound_explanations.certified, 1);
    let (again, counts) = program.close(
        &mut region(&dead_end()),
        &mut program.index.knowledge(),
        &mut scratch,
    );
    assert_eq!(again, Narrowing::Refuted);
    assert_eq!(counts.support_explanations.refuted, 1);
    assert_eq!(counts.support_explanations.matched, 1);
    assert_eq!(counts.bound_explanations.attempts, 0);
    assert_eq!(scratch.learning.explanations.len(), 1);
}

#[test]
fn the_bound_attempt_cap_declines_further_rows() {
    let program = program(LearningLimits {
        founded_attempts: 0,
        bound_attempts: 0,
        ..LearningLimits::default()
    });
    let mut scratch = crate::NarrowingScratch::default();
    let (result, counts) = program.close(
        &mut region(&dead_end()),
        &mut program.index.knowledge(),
        &mut scratch,
    );
    assert_eq!(result, Narrowing::Refuted);
    assert_eq!(counts.bound_explanations.attempts, 0);
    assert_eq!(counts.bound_explanations.certified, 0);
    assert_eq!(scratch.learning.explanations.len(), 0);
}

#[test]
fn a_theory_carrier_does_not_apply_a_bound_row() {
    let bound = program(founding_disabled());
    let mut scratch = crate::NarrowingScratch::default();
    let (result, counts) = bound.close(
        &mut region(&dead_end()),
        &mut bound.index.knowledge(),
        &mut scratch,
    );
    assert_eq!(result, Narrowing::Refuted);
    assert_eq!(counts.bound_explanations.certified, 1);

    let mut store = LearningStore::new();
    let attempt = crate::test_support::stop_learning(&scratch.deliver_learning(
        &bound.owner,
        &mut store,
        &mut Meter::reserved(&mut unlimited()),
    ));
    assert_eq!(attempt.result, Ok(()));
    assert_eq!(store.len(), 1);
    assert_eq!(store.source_rows(), (0, 0, 1));

    let mut theory = Program::new(false);
    theory.owner = LearningOwner::theory(&theory.theory, founding_disabled());
    let mut seeded = crate::NarrowingScratch::default();
    let seeded_attempt = crate::test_support::stop_learning(&seeded.seed_learning(
        &theory.owner,
        &mut store,
        &mut Meter::reserved(&mut unlimited()),
    ));
    assert_eq!(seeded_attempt.result, Ok(()));
    assert_eq!(seeded.learned_rows(), 0);

    let (again, counts) = theory.close(
        &mut region(&dead_end()),
        &mut theory.index.knowledge(),
        &mut seeded,
    );
    assert_eq!(again, Narrowing::Refuted);
    assert_eq!(counts.support_explanations.matched, 0);
    assert_eq!(counts.conflict_explanations.certified, 1);
    assert_eq!(counts.bound_explanations.certified, 0);
    assert_eq!(seeded.learned_rows(), 1);
}

#[test]
fn dropping_the_bound_class_keeps_theory_rows() {
    let bound = program(founding_disabled());
    let mut bound_scratch = crate::NarrowingScratch::default();
    let (result, counts) = bound.close(
        &mut region(&dead_end()),
        &mut bound.index.knowledge(),
        &mut bound_scratch,
    );
    assert_eq!(result, Narrowing::Refuted);
    assert_eq!(counts.bound_explanations.certified, 1);

    let mut theory = Program::new(false);
    theory.owner = LearningOwner::theory(&theory.theory, founding_disabled());
    let mut theory_scratch = crate::NarrowingScratch::default();
    let (result, counts) = theory.close(
        &mut region(&dead_end()),
        &mut theory.index.knowledge(),
        &mut theory_scratch,
    );
    assert_eq!(result, Narrowing::Refuted);
    assert_eq!(counts.conflict_explanations.certified, 1);

    let mut store = LearningStore::new();
    let attempt = crate::test_support::stop_learning(&bound_scratch.deliver_learning(
        &bound.owner,
        &mut store,
        &mut Meter::reserved(&mut unlimited()),
    ));
    assert_eq!(attempt.result, Ok(()));
    let attempt = crate::test_support::stop_learning(&theory_scratch.deliver_learning(
        &theory.owner,
        &mut store,
        &mut Meter::reserved(&mut unlimited()),
    ));
    assert_eq!(attempt.result, Ok(()));
    assert_eq!(store.source_rows(), (0, 1, 1));

    store.drop_bound_class();
    assert_eq!(store.source_rows(), (0, 1, 0));
    assert_eq!(store.len(), 1);

    let mut seeded = crate::NarrowingScratch::default();
    let seeded_attempt = crate::test_support::stop_learning(&seeded.seed_learning(
        &bound.owner,
        &mut store,
        &mut Meter::reserved(&mut unlimited()),
    ));
    assert_eq!(seeded_attempt.result, Ok(()));
    assert_eq!(seeded.learned_rows(), 0);
}
