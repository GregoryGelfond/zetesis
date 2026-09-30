//! Counts in the nonzero form the workspace's worker and capacity settings take.

use std::num::NonZeroUsize;

/// The nonzero count `count`.
///
/// # Panics
/// Panics if `count` is zero.
#[must_use]
pub fn nonzero(count: usize) -> NonZeroUsize {
    NonZeroUsize::new(count).expect("a nonzero test count")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_nonzero_count_keeps_its_value() {
        assert_eq!(nonzero(3).get(), 3);
    }

    #[test]
    #[should_panic(expected = "a nonzero test count")]
    fn a_zero_count_is_refused() {
        let _ = nonzero(0);
    }
}
