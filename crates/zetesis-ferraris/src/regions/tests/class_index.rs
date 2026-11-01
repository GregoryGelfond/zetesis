//! Class markers constrain the actual inputs, without owning regional learning.

use std::mem::{align_of, size_of, size_of_val};

use zetesis_algebra::{AnswerSetsSought, Cancellation, Countermodels};

use super::super::{Context, LearningAccess, ReadingClass, Width};
use crate::test_support::theory;
use crate::{
    FrozenSubject, Knowledge, Narrower, NarrowingScratch, Node, OriginalSubject, Region,
    RegionLimits,
};

#[test]
fn class_markers_do_not_enlarge_regional_knowledge() {
    assert_eq!(size_of::<Knowledge<AnswerSetsSought>>(), size_of::<Width>());
    assert_eq!(size_of::<Knowledge<Countermodels>>(), size_of::<Width>());
    assert_eq!(
        align_of::<Knowledge<AnswerSetsSought>>(),
        align_of::<Width>()
    );
    assert_eq!(align_of::<Knowledge<Countermodels>>(), align_of::<Width>());
    assert_eq!(
        size_of::<Context<'_, AnswerSetsSought>>(),
        size_of::<OriginalSubject<'_>>()
    );
    assert_eq!(
        size_of::<Context<'_, Countermodels>>(),
        size_of::<FrozenSubject<'_>>()
    );
}

#[test]
fn frozen_execution_has_no_learning_borrow() {
    let mut scratch = NarrowingScratch::default();
    let mut frozen: () = Countermodels::borrow_learning(&mut scratch.learning);
    assert_eq!(size_of_val(&frozen), 0);
    assert!(frozen.state().is_none());
    assert!(frozen.state_mut().is_none());
    let mut original = AnswerSetsSought::borrow_learning(&mut scratch.learning);
    assert!(original.state().is_some());
    assert!(original.state_mut().is_some());
}

#[test]
fn both_concrete_factories_execute_the_same_unmasked_readings() {
    let theory = theory(1, vec![Node::atom(0)], vec![0]);
    let index = Narrower::new(&theory);
    let mut original = index.knowledge();
    let mut frozen = index.frozen_knowledge();
    assert_eq!(original.retained_bytes(), frozen.retained_bytes());
    let mut original_region = Region::all_open(1);
    let mut frozen_region = original_region.clone();
    let mut scratch = NarrowingScratch::default();
    let cancellation = Cancellation::default();
    let a = index.narrow_known(
        OriginalSubject::new(&theory, None),
        &mut original_region,
        &mut original,
        &mut scratch,
        RegionLimits::default(),
        &cancellation,
    );
    let b = index.narrow_frozen_known(
        FrozenSubject::new(&theory, &[true]),
        &mut frozen_region,
        &mut frozen,
        &mut scratch,
        RegionLimits::default(),
        &cancellation,
    );
    assert_eq!(a.result, b.result);
    assert_eq!(a.statistics, b.statistics);
    assert_eq!(a.preferred, b.preferred);
    assert_eq!(original_region, frozen_region);
    assert_eq!(
        super::compact(&original).masks.slices(),
        super::compact(&frozen).masks.slices()
    );
    let mut copied = index.frozen_knowledge();
    copied.clone_from(&frozen);
    super::counters::same_knowledge(&copied, &frozen);
    assert_eq!(scratch.learned_rows(), 0);
}
