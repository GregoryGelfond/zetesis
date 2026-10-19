//! Producer-free subjects borrow the worklists, not original support ownership.

use super::*;

fn assert_original_preparation(program: &Program, scratch: &NarrowingScratch) {
    assert!(
        scratch
            .founded
            .owner
            .as_ref()
            .is_some_and(|owner| owner.same_instance(&program.owner))
    );
    assert!(scratch.founded.projected.is_some());
}

#[test]
fn candidate_only_subject_retains_original_preparation() {
    let program = Program::new(negative_seed());
    let (mut region, mut known, mut scratch) = program.closed();
    scratch
        .founded
        .prepare_projected(&program.owner, &program.index, &program.producers);
    let before = scratch.founded.passes;
    let bytes = scratch.founded.retained_bytes();
    let foreign = theory(3, vec![Node::atom(2)], vec![]);
    let index = Narrower::new(&foreign);
    let attempt = index.propagate_known_reserved(
        OriginalSubject::new(&foreign, None),
        &mut region,
        &mut index.knowledge(),
        &mut scratch,
        &Cancellation::default(),
        &mut Allowance(u64::MAX),
    );
    assert_eq!(attempt.result.unwrap(), Narrowing::Fixed { changed: false });
    assert_eq!(attempt.statistics.founded_work, 0);
    assert_eq!(scratch.founded.passes, before);
    assert_original_preparation(&program, &scratch);
    assert_eq!(scratch.founded.retained_bytes(), bytes);

    // Force a real original pass, so unchanged inherited knowledge cannot hide
    // loss of preparation or dependence on the foreign subject's worklists.
    invalidate(&mut known);
    let (result, counts) = program.close(&mut region, &mut known, &mut scratch);
    assert_eq!(result, Narrowing::Fixed { changed: false });
    assert!(counts.founded_work > 0);
    assert_eq!(scratch.founded.passes, before + 1);
    assert!(completed(&known));
    assert_original_preparation(&program, &scratch);
}

#[test]
fn frozen_subject_retains_original_preparation() {
    let program = Program::new(negative_seed());
    let (mut original_region, mut original_known, mut scratch) = program.closed();
    scratch
        .founded
        .prepare_projected(&program.owner, &program.index, &program.producers);
    let before = scratch.founded.passes;
    let foreign = theory(
        3,
        vec![Node::atom(0), Node::atom(1), Node::or_pair([0, 1])],
        vec![2],
    );
    let index = Narrower::new(&foreign);
    // All three nodes hold under the candidate {a,b}; use separate reduct
    // knowledge and region while deliberately reusing the worker scratch.
    let attempt = index.narrow_frozen_known_reserved(
        FrozenSubject::new(&foreign, &[true, true, true]),
        &mut Region::all_open(3),
        &mut index.knowledge(),
        &mut scratch,
        &Cancellation::default(),
        &mut Allowance(u64::MAX),
    );
    assert_eq!(attempt.result.unwrap(), Narrowing::Fixed { changed: false });
    assert_eq!(attempt.statistics.founded_work, 0);
    assert_eq!(scratch.founded.passes, before);
    assert_original_preparation(&program, &scratch);

    invalidate(&mut original_known);
    let (result, counts) = program.close(&mut original_region, &mut original_known, &mut scratch);
    assert_eq!(result, Narrowing::Fixed { changed: false });
    assert!(counts.founded_work > 0);
    assert_eq!(scratch.founded.passes, before + 1);
    assert!(completed(&original_known));
    assert_original_preparation(&program, &scratch);
}

#[test]
fn restriction_decisions_invalidate_original_completion() {
    let program = Program::new(negative_seed());
    let (mut region, mut original_known, mut scratch) = program.closed();
    scratch
        .founded
        .prepare_projected(&program.owner, &program.index, &program.producers);
    let before = scratch.founded.passes;
    let restriction = theory(3, vec![Node::atom(2)], vec![0]);
    let index = Narrower::new(&restriction);
    let attempt = index.propagate_known_reserved(
        OriginalSubject::new(&restriction, None),
        &mut region,
        &mut index.knowledge(),
        &mut scratch,
        &Cancellation::default(),
        &mut Allowance(u64::MAX),
    );
    assert_eq!(attempt.result.unwrap(), Narrowing::Fixed { changed: true });
    assert!(region.is_held(2));
    assert!(
        completed(&original_known),
        "the original has not seen the decision yet"
    );
    assert_original_preparation(&program, &scratch);

    // Holding c falsifies the original cycle's only external support. Its
    // immutable preparation survives; its prior region completion must not.
    let (result, _) = program.close(&mut region, &mut original_known, &mut scratch);
    assert_eq!(result, Narrowing::Fixed { changed: true });
    assert!(region.is_cut(0) && region.is_cut(1) && region.is_held(2));
    assert!(scratch.founded.passes > before);
    assert!(completed(&original_known));
    assert_original_preparation(&program, &scratch);
    let expected: BTreeSet<_> = (0..8)
        .filter(|&mask| mask & 4 != 0 && accepted(&program.owner, mask))
        .collect();
    assert_eq!(expected, BTreeSet::from([4]));
}
