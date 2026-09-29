//! The least resource threshold at which an attempt succeeds.

/// The least threshold at which `attempt` succeeds, found by doubling and then
/// bisecting; `attempt` must succeed at every threshold above its least.
pub fn first_success(mut attempt: impl FnMut(u64) -> bool) -> u64 {
    let mut high = 1;
    while !attempt(high) {
        high *= 2;
        assert!(high <= 1_048_576, "bounded resource threshold probe");
    }
    let mut low = 0;
    while low + 1 < high {
        let middle = low + (high - low) / 2;
        if attempt(middle) {
            high = middle;
        } else {
            low = middle;
        }
    }
    high
}
