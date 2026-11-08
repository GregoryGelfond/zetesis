//! Only the owner's carrier matches and captures; any other original subject
//! of the same theory leaves the tier untouched.

use super::*;

fn plain(
    program: &Program,
    region: &mut Region,
    scratch: &mut NarrowingScratch,
) -> (Narrowing, NarrowingStatistics) {
    let attempt = program.index.narrow_known(
        OriginalSubject::new(&program.theory, Some(&program.producers)),
        region,
        &mut program.index.knowledge(),
        scratch,
        RegionLimits::default(),
        &Cancellation::default(),
    );
    (attempt.result.unwrap(), attempt.statistics)
}

#[test]
fn a_plain_original_subject_captures_no_row() {
    let program = Program::new(false);
    let mut scratch = NarrowingScratch::default();
    // The carrier refutes this region with one capture (`Program::captured`).
    let (result, counts) = plain(
        &program,
        &mut region(&[(0, true), (2, false), (3, true), (4, false)]),
        &mut scratch,
    );
    assert_eq!(result, Narrowing::Refuted);
    assert_eq!(counts.support_explanations.attempts, 0);
    assert_eq!(scratch.learning.explanations.len(), 0);
}

#[test]
fn a_plain_original_subject_matches_no_row() {
    let program = Program::new(false);
    let (_, mut scratch) = program.captured();
    // The carrier cuts a and b here by the captured row alone
    // (`matching_conditions_cut_open_unsupported_atoms`).
    let (_, counts) = plain(
        &program,
        &mut region(&[(2, false), (3, true)]),
        &mut scratch,
    );
    assert_eq!(counts.support_explanations.matched, 0);
    assert_eq!(counts.support_explanations.work, 0);
    assert_eq!(scratch.learning.explanations.len(), 1);
}

#[test]
fn a_carrier_without_producers_still_matches() {
    let program = Program::new(false);
    let (_, mut scratch) = program.captured();
    let mut child = region(&[(2, false), (3, true)]);
    let attempt = program.index.narrow_known(
        OriginalSubject::carrier(&program.owner, None),
        &mut child,
        &mut program.index.knowledge(),
        &mut scratch,
        RegionLimits::default(),
        &Cancellation::default(),
    );
    let result = attempt.result.unwrap();
    let counts = attempt.statistics;
    assert!(matches!(result, Narrowing::Fixed { .. }));
    assert!(child.is_cut(0) && child.is_cut(1));
    assert_eq!(counts.support_explanations.matched, 1);
    assert_eq!(counts.support_explanations.attempts, 0);
}
