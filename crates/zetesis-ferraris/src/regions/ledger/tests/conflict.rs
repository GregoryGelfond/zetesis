//! Refuted-region cores are a ledger source of the carrier's theory class.

use super::{Program, region};
use crate::{ConflictCore, LearningLimits, LearningOwner, Narrowing};

use crate::regions::ledger::{ConflictInput, Ledger};
use crate::test_support::Allowance;
use zetesis_algebra::{Failure, Meter, Stop};

fn program(limits: LearningLimits) -> Program {
    let mut program = Program::new(false);
    program.owner = LearningOwner::theory(&program.theory, limits);
    program
}

fn founding_disabled(core: ConflictCore) -> LearningLimits {
    LearningLimits {
        founded_attempts: 0,
        conflict_core: core,
        ..LearningLimits::default()
    }
}

/// The same decisions the founded collector uses; without that source, the
/// conflict source records the region's decided literals.
fn dead_end() -> [(usize, bool); 4] {
    [(0, true), (2, false), (3, true), (4, false)]
}

#[test]
fn a_refuted_carrier_region_certifies_a_conflict_row() {
    let program = program(founding_disabled(ConflictCore::Full));
    let mut scratch = crate::NarrowingScratch::default();
    let (result, counts) = program.close(
        &mut region(&dead_end()),
        &mut program.index.knowledge(),
        &mut scratch,
    );
    assert_eq!(result, Narrowing::Refuted);
    assert_eq!(counts.support_explanations.certified, 0);
    assert_eq!(counts.conflict_explanations.attempts, 1);
    assert_eq!(counts.conflict_explanations.certified, 1);
    assert_eq!(scratch.learning.explanations.len(), 1);
    let row = &scratch.learning.explanations.rows[0].row;
    let conditions: Vec<_> = row.conditions().collect();
    let outputs: Vec<_> = row.outputs().collect();
    // One decided literal is the negated output; the rest are conditions.
    assert_eq!(conditions.len() + outputs.len(), dead_end().len());
    assert_eq!(outputs.len(), 1);
}

#[test]
fn a_fresh_sibling_reuses_the_conflict_row() {
    let program = program(founding_disabled(ConflictCore::Full));
    let mut scratch = crate::NarrowingScratch::default();
    let (result, counts) = program.close(
        &mut region(&dead_end()),
        &mut program.index.knowledge(),
        &mut scratch,
    );
    assert_eq!(result, Narrowing::Refuted);
    assert_eq!(counts.conflict_explanations.certified, 1);
    let (again, counts) = program.close(
        &mut region(&dead_end()),
        &mut program.index.knowledge(),
        &mut scratch,
    );
    assert_eq!(again, Narrowing::Refuted);
    assert_eq!(counts.support_explanations.refuted, 1);
    assert_eq!(counts.support_explanations.matched, 1);
    assert_eq!(counts.conflict_explanations.attempts, 0);
    assert_eq!(scratch.learning.explanations.len(), 1);
}

#[test]
fn minimization_drops_a_literal_the_theory_does_not_need() {
    let full = program(founding_disabled(ConflictCore::Full));
    let mut scratch = crate::NarrowingScratch::default();
    let (_, full_counts) = full.close(
        &mut region(&dead_end()),
        &mut full.index.knowledge(),
        &mut scratch,
    );
    assert_eq!(full_counts.conflict_explanations.certified, 1);
    let full_width = scratch.learning.explanations.rows[0]
        .row
        .conditions()
        .count()
        + 1;

    let minimized = program(founding_disabled(ConflictCore::Minimized));
    let mut scratch = crate::NarrowingScratch::default();
    let (_, counts) = minimized.close(
        &mut region(&dead_end()),
        &mut minimized.index.knowledge(),
        &mut scratch,
    );
    assert_eq!(counts.conflict_explanations.certified, 1);
    let narrow_width = scratch.learning.explanations.rows[0]
        .row
        .conditions()
        .count()
        + 1;
    assert!(
        narrow_width < full_width,
        "minimized core {narrow_width} should be tighter than the full decided set {full_width}"
    );
    assert!(counts.conflict_explanations.shrunk > 0);
}

#[test]
fn the_conflict_attempt_cap_declines_further_rows() {
    let program = program(LearningLimits {
        founded_attempts: 0,
        conflict_attempts: 0,
        ..LearningLimits::default()
    });
    let mut scratch = crate::NarrowingScratch::default();
    let (result, counts) = program.close(
        &mut region(&dead_end()),
        &mut program.index.knowledge(),
        &mut scratch,
    );
    assert_eq!(result, Narrowing::Refuted);
    assert_eq!(counts.conflict_explanations.attempts, 0);
    assert_eq!(counts.conflict_explanations.certified, 0);
    assert_eq!(scratch.learning.explanations.len(), 0);
}

#[test]
fn full_core_width_refusal_allocates_no_literal_storage() {
    let mut ledger = Ledger::default();
    let mut region = crate::Region::all_open(65);
    for atom in 0..18 {
        assert!(region.hold(atom));
    }
    let mut counts = crate::ConflictLearningStatistics::default();
    let mut quota = Allowance {
        remaining: u64::MAX,
        failure: Stop::WorkLimit,
    };
    let mut work = Meter::reserved(&mut quota);
    ledger
        .capture_conflict(ConflictInput::Region(&region), &mut work, &mut counts)
        .unwrap();
    assert_eq!(counts.attempts, 1);
    assert_eq!(counts.declined, 1);
    assert_eq!(counts.certified, 0);
    assert_eq!(work.charged(), 1);
    assert_eq!(
        ledger.conflict_remaining,
        LearningLimits::DEFAULT.conflict_work - 1
    );
    assert_eq!(ledger.peak_bytes(), 0);
    assert_eq!(ledger.len(), 0);
}

#[test]
fn full_core_discovery_obeys_its_source_allowance() {
    let mut ledger = Ledger {
        conflict_remaining: 1,
        ..Ledger::default()
    };
    let mut region = crate::Region::all_open(65);
    assert!(region.hold(64));
    let mut counts = crate::ConflictLearningStatistics::default();
    let mut quota = Allowance {
        remaining: u64::MAX,
        failure: Stop::WorkLimit,
    };
    let mut work = Meter::reserved(&mut quota);
    ledger
        .capture_conflict(ConflictInput::Region(&region), &mut work, &mut counts)
        .unwrap();
    assert_eq!(counts.attempts, 1);
    assert_eq!(counts.declined, 1);
    assert_eq!(ledger.conflict_remaining, 0);
    assert_eq!(work.charged(), 1);
    assert_eq!(ledger.peak_bytes(), 0);
    assert_eq!(ledger.len(), 0);
}

#[test]
fn full_core_copy_refusal_retains_its_allocation_peak() {
    let mut ledger = Ledger::default();
    let mut region = crate::Region::all_open(65);
    assert!(region.hold(0));
    assert!(region.cut(64));
    let mut counts = crate::ConflictLearningStatistics::default();
    let mut quota = Allowance {
        remaining: 3,
        failure: Stop::WorkLimit,
    };
    let mut work = Meter::reserved(&mut quota);
    let result = ledger.capture_conflict(ConflictInput::Region(&region), &mut work, &mut counts);
    assert_eq!(result.map_err(Failure::into_stop), Err(Stop::WorkLimit));
    assert_eq!(counts.attempts, 1);
    assert_eq!(counts.certified, 0);
    assert_eq!(work.charged(), 3);
    assert_eq!(
        ledger.conflict_remaining,
        LearningLimits::DEFAULT.conflict_work - 3
    );
    assert!(ledger.peak_bytes() >= 2 * size_of::<(usize, bool)>() as u128);
    assert_eq!(ledger.len(), 0);
}

#[test]
fn full_core_optional_copy_decline_retains_its_allocation_peak() {
    let mut ledger = Ledger {
        conflict_remaining: 3,
        ..Ledger::default()
    };
    let mut region = crate::Region::all_open(65);
    assert!(region.hold(0));
    assert!(region.cut(64));
    let mut counts = crate::ConflictLearningStatistics::default();
    let mut quota = Allowance {
        remaining: u64::MAX,
        failure: Stop::WorkLimit,
    };
    let mut work = Meter::reserved(&mut quota);
    ledger
        .capture_conflict(ConflictInput::Region(&region), &mut work, &mut counts)
        .unwrap();
    assert_eq!(counts.attempts, 1);
    assert_eq!(counts.declined, 1);
    assert_eq!(ledger.conflict_remaining, 0);
    assert_eq!(work.charged(), 3);
    assert!(ledger.peak_bytes() >= 2 * size_of::<(usize, bool)>() as u128);
    assert_eq!(ledger.len(), 0);
}

#[test]
fn packed_full_core_keeps_signed_literal_identity() {
    let mut ledger = Ledger::default();
    let mut region = crate::Region::all_open(130);
    for (atom, value) in [(0, true), (63, false), (64, true), (129, false)] {
        assert!(if value {
            region.hold(atom)
        } else {
            region.cut(atom)
        });
    }
    let mut counts = crate::ConflictLearningStatistics::default();
    let mut quota = Allowance {
        remaining: u64::MAX,
        failure: Stop::WorkLimit,
    };
    let mut work = Meter::reserved(&mut quota);
    ledger
        .capture_conflict(ConflictInput::Region(&region), &mut work, &mut counts)
        .unwrap();
    assert_eq!(counts.certified, 1);
    assert_eq!(
        ledger.rows[0].row.conditions().collect::<Vec<_>>(),
        [(0, true), (63, false), (64, true)]
    );
    assert_eq!(
        ledger.rows[0].row.outputs().collect::<Vec<_>>(),
        [(129, true)]
    );
}
