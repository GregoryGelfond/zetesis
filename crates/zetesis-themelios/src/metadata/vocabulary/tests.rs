//! Peak publication and component failure remain separate boundaries.

use super::*;

#[test]
fn refused_metadata_observation_keeps_its_peak() {
    let mut authority = Authority::new(MetadataStorageLimits::default(), 0).unwrap();
    let retained = authority.bytes();
    let limit = usize::try_from(retained).unwrap();
    authority.limits.max_bytes = limit;
    assert_eq!(authority.observe(0), Ok(()));
    assert_eq!(
        authority.observe(1),
        Err(MetadataStorageError::from(StorageError::Storage {
            required: retained + 1,
            limit,
        }))
    );
    assert_eq!(authority.peak, retained + 1);
    assert_eq!(authority.bytes(), retained);
    // A previous peak above this ceiling must not reject a fitting new total.
    assert_eq!(authority.observe(0), Ok(()));
    assert_eq!(authority.peak, retained + 1);
}

#[test]
fn component_failure_precedes_outer_capacity_refusal() {
    let mut authority = Authority::new(MetadataStorageLimits::default(), 0).unwrap();
    let before = authority.bytes();
    authority.limits.max_bytes = usize::try_from(before).unwrap();
    let error = authority
        .append(0, |components, read, _available| {
            // Deliberately construct competing private-boundary failures using
            // real component growth; no allocator-slack assumption is needed.
            components
                .append_terms_with(read, [TemplateTerm::Variable(0)], usize::MAX, READY)
                .unwrap();
            Err::<(), _>(TemplateCatalogFailure::Incomplete)
        })
        .unwrap_err();
    assert_eq!(
        error,
        MetadataStorageError::Components(TemplateCatalogFailure::Incomplete)
    );
    assert!(authority.bytes() > before);
    assert!(authority.peak >= authority.bytes());
    assert!(authority.peak > authority.limits.max_bytes as u128);
}
