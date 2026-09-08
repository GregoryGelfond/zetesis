//! Optional watch links retain an index without a separate presence word.

use std::num::NonZeroUsize;

use crate::AdmissionError;

/// A node in the fixed two-per-clause intrusive watch registry.
///
/// The stored successor represents a zero-based index; absence remains `None`.
/// The transparent nonzero representation gives `Option<WatchNode>` the layout
/// of one `usize`. Construction checks the sole excluded index, `usize::MAX`.
/// CNF admission already excludes it by bounding twice the clause count.
/// Moving a link copies this identity without allocation or re-encoding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub(super) struct WatchNode(NonZeroUsize);

impl WatchNode {
    pub(super) fn new(index: usize) -> Result<Self, AdmissionError> {
        index
            .checked_add(1)
            .and_then(NonZeroUsize::new)
            .map(Self)
            .ok_or(AdmissionError::Overflow)
    }

    pub(super) const fn index(self) -> usize {
        self.0.get() - 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn watch_identity_round_trips(index in 0..usize::MAX) {
            prop_assert_eq!(WatchNode::new(index).unwrap().index(), index);
        }
    }

    #[test]
    fn last_representable_identity_round_trips() {
        assert_eq!(
            WatchNode::new(usize::MAX - 1).unwrap().index(),
            usize::MAX - 1
        );
    }

    #[test]
    fn unrepresentable_identity_is_refused() {
        assert_eq!(WatchNode::new(usize::MAX), Err(AdmissionError::Overflow));
    }

    #[test]
    fn optional_identity_occupies_one_index_word() {
        assert_eq!(size_of::<Option<WatchNode>>(), size_of::<usize>());
        assert_eq!(align_of::<Option<WatchNode>>(), align_of::<usize>());
    }
}
