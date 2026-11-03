//! Numeric boundary contracts of structured-value admission.

use super::{ValueError, ValueResource, ceiling};

#[test]
fn value_limits_keep_full_width_observations() {
    for resource in [
        ValueResource::Nodes,
        ValueResource::Depth,
        ValueResource::Bytes,
    ] {
        assert_eq!(ceiling(resource, usize::MAX as u128, usize::MAX), Ok(()));
        assert_eq!(
            ceiling(resource, u128::MAX, 0),
            Err(ValueError::Limit {
                resource,
                observed: u128::MAX,
                limit: 0,
            }),
        );
    }
}
