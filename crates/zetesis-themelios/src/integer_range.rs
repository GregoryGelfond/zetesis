//! Inclusive integer ranges after both endpoint evaluations have succeeded.
//!
//! A nonnumeric endpoint or descending bounds contribute no values. Evaluation
//! failures remain with the caller and must not be represented by `None`.

use std::ops::RangeInclusive;

pub(super) fn inclusive(lower: Option<i32>, upper: Option<i32>) -> Option<RangeInclusive<i32>> {
    let (lower, upper) = (lower?, upper?);
    (lower <= upper).then_some(lower..=upper)
}

/// Count without enumeration or i32 endpoint arithmetic, including the entire
/// i32 carrier. Exhausted and descending ranges have no remaining values.
pub(super) fn width(range: &RangeInclusive<i32>) -> u64 {
    if range.is_empty() {
        return 0;
    }
    u64::try_from(i64::from(*range.end()) - i64::from(*range.start()) + 1)
        .expect("a nonempty i32 range has a positive u64 width")
}

#[cfg(test)]
mod tests {
    use super::{inclusive, width};

    #[test]
    fn nonnumeric_endpoints_have_no_integer_range() {
        for endpoints in [(None, None), (Some(1), None), (None, Some(1))] {
            assert_eq!(inclusive(endpoints.0, endpoints.1), None);
        }
    }

    #[test]
    fn descending_endpoints_have_no_integer_range() {
        assert_eq!(inclusive(Some(2), Some(1)), None);
    }

    #[test]
    fn the_entire_i32_carrier_has_exact_width() {
        assert_eq!(width(&(i32::MIN..=i32::MAX)), 1_u64 << 32);
    }

    #[test]
    fn maximum_endpoint_is_emitted_once() {
        let mut range = inclusive(Some(i32::MAX), Some(i32::MAX)).expect("numeric range");
        assert_eq!(width(&range), 1);
        assert_eq!(range.next(), Some(i32::MAX));
        assert_eq!(range.next(), None);
        assert_eq!(width(&range), 0);
    }
}
