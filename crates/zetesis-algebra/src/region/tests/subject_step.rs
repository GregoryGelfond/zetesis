use crate::region::{Narrowing, SubjectStep};

#[test]
fn completed_subject_progress_preserves_its_refutation_and_change() {
    assert_eq!(SubjectStep::Refuted.narrowing(), Narrowing::Refuted);
    assert_eq!(
        SubjectStep::Changed.narrowing(),
        Narrowing::Fixed { changed: true }
    );
    assert_eq!(
        SubjectStep::Unchanged.narrowing(),
        Narrowing::Fixed { changed: false }
    );
}
