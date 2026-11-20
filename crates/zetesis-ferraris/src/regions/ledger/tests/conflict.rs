//! Refuted-region cores are a ledger source of the carrier's theory class.

use super::{Program, region};
use crate::{ConflictCore, LearningLimits, LearningOwner, Narrowing};

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
