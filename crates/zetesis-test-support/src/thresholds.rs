//! The least resource threshold at which an operation succeeds.

/// An unsigned integer type a resource threshold takes.
pub trait Threshold: Copy {
    /// The threshold as a `u64`.
    fn to_u64(self) -> u64;
    /// The threshold `value`, which the type must hold.
    fn from_u64(value: u64) -> Self;
}

impl Threshold for u64 {
    fn to_u64(self) -> u64 {
        self
    }

    fn from_u64(value: u64) -> Self {
        value
    }
}

impl Threshold for usize {
    fn to_u64(self) -> u64 {
        u64::try_from(self).expect("a threshold within u64")
    }

    fn from_u64(value: u64) -> Self {
        usize::try_from(value).expect("a threshold within usize")
    }
}

/// The least threshold, at most `ceiling`, at which `attempt` succeeds,
/// found by bisection. `attempt` must fail at zero, since the threshold is a
/// resource the operation charges, and succeed at `ceiling` and at every
/// threshold from its least on. Every threshold the search tries is passed to
/// `attempt`, which may check how a failure refuses.
///
/// # Panics
/// Panics if `attempt` succeeds at zero or fails at `ceiling`.
pub fn first_success<T: Threshold>(ceiling: T, mut attempt: impl FnMut(T) -> bool) -> T {
    let mut probe = |value: u64| attempt(T::from_u64(value));
    assert!(!probe(0), "the operation must charge this resource");
    let mut high = ceiling.to_u64();
    assert!(probe(high), "the ceiling must admit the operation");
    let mut low = 0;
    while low + 1 < high {
        let middle = low.midpoint(high);
        if probe(middle) {
            high = middle;
        } else {
            low = middle;
        }
    }
    T::from_u64(high)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_least_succeeding_threshold_is_found() {
        assert_eq!(first_success(1_000_u64, |threshold| threshold >= 37), 37);
        assert_eq!(first_success(64_usize, |threshold| threshold >= 64), 64);
        assert_eq!(first_success(64_usize, |threshold| threshold >= 1), 1);
    }

    #[test]
    fn every_tried_threshold_reaches_the_attempt() {
        let mut tried = Vec::new();
        let least = first_success(16_u64, |threshold| {
            tried.push(threshold);
            threshold >= 5
        });
        assert_eq!(least, 5);
        assert_eq!(tried[..2], [0, 16]);
        assert!(tried.contains(&4) && tried.contains(&5));
    }

    #[test]
    #[should_panic(expected = "must charge this resource")]
    fn success_at_zero_is_refused() {
        let _ = first_success(8_u64, |_| true);
    }

    #[test]
    #[should_panic(expected = "the ceiling must admit the operation")]
    fn failure_at_the_ceiling_is_refused() {
        let _ = first_success(8_u64, |threshold| threshold > 8);
    }
}
