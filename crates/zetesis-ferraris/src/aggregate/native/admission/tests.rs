use super::*;
use crate::test_support::theory;

fn admission<'w, 'c>(
    owner: &Theory,
    work: &'w mut Work<'c>,
) -> Admission<'w, 'c, VocabularyBuilder> {
    Admission::new(
        owner,
        Function::Count,
        VocabularyBuilder::new(usize::MAX).unwrap(),
        0,
        0,
        AdmissionLimits {
            max_bytes: u64::MAX,
            max_storage_bytes: u64::MAX,
            ..AdmissionLimits::default()
        },
        work,
    )
    .unwrap()
}

#[test]
fn refused_actual_storage_retains_current_and_peak() {
    let owner = theory(0, vec![], vec![]);
    let cancellation = Cancellation::default();
    let mut work = Work::new(u64::MAX, &cancellation);
    let mut admission = admission(&owner, &mut work);
    let current = narrow(admission.current()).unwrap();
    admission.limits.max_storage_bytes = current;
    let before = admission.work.statistics();
    let next_peak = before.storage_peak_bytes + 1;
    assert_eq!(
        admission.observe(u128::from(next_peak - current)),
        Err(ErrorKind::Limit(Resource::StorageBytes))
    );
    assert_eq!(admission.work.storage_bytes, current);
    assert_eq!(admission.work.storage_peak_bytes, next_peak);
    assert_eq!(admission.work.meter.charged(), before.work);
    assert_eq!(admission.work.resident_bytes, before.resident_bytes);
    assert_eq!(admission.work.peak_bytes, before.peak_bytes);
}

#[test]
fn unrepresentable_actual_peak_keeps_current_before_refusal() {
    let owner = theory(0, vec![], vec![]);
    let cancellation = Cancellation::default();
    let mut work = Work::new(u64::MAX, &cancellation);
    let mut admission = admission(&owner, &mut work);
    let current = admission.current();
    admission.work.storage_bytes = 0;
    admission.work.storage_peak_bytes = 7;
    admission.limits.max_storage_bytes = 0;
    let overlap = u128::from(u64::MAX) + 1 - current;
    assert_eq!(admission.observe(overlap), Err(ErrorKind::Overflow));
    assert_eq!(admission.work.storage_bytes, narrow(current).unwrap());
    assert_eq!(admission.work.storage_peak_bytes, 7);
}

#[test]
fn logical_byte_refusal_retains_both_logical_assignments() {
    let owner = theory(0, vec![], vec![]);
    let cancellation = Cancellation::default();
    let mut work = Work::new(u64::MAX, &cancellation);
    let mut admission = admission(&owner, &mut work);
    let before = admission.work.statistics();
    admission.limits.max_bytes = 0;
    assert_eq!(admission.logical(1), Err(ErrorKind::Limit(Resource::Bytes)));
    assert_eq!(admission.work.resident_bytes, before.resident_bytes + 1);
    assert_eq!(
        admission.work.peak_bytes,
        admission.work.resident_bytes + admission.logical_scratch
    );
    assert_eq!(admission.work.storage_bytes, before.storage_bytes);
    assert_eq!(admission.work.storage_peak_bytes, before.storage_peak_bytes);
    assert_eq!(admission.work.meter.charged(), before.work);
}

#[test]
fn refused_storage_request_does_not_record_an_actual_peak() {
    let cancellation = Cancellation::default();
    let mut work = Work::new(1, &cancellation);
    work.storage_peak_bytes = 7;
    let limits = AdmissionLimits {
        max_storage_bytes: 9,
        ..AdmissionLimits::default()
    };
    assert_eq!(
        reserve::<u8>(1, 9, limits, &mut work),
        Err(ErrorKind::Limit(Resource::StorageBytes))
    );
    assert_eq!(work.meter.charged(), 1);
    assert_eq!(work.storage_bytes, 0);
    assert_eq!(work.storage_peak_bytes, 7);
}
