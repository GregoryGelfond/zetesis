//! Identities of the candidate slates a stream proposes, so that a checked
//! result can name the slate it belongs to without borrowing its candidates.

use std::num::NonZeroU64;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::{Interpretation, Theory};

/// Stream numbers issued in this process; zero is never issued.
static STREAMS: AtomicU64 = AtomicU64::new(1);

/// The identity of one slate of candidates: the stream that proposed it, its
/// place in that stream's proposals, and the stream's theory instance.
///
/// A stream's number is unique in the process, so identities from different
/// streams differ, even over one theory, and so do identities of different
/// slates of one stream. Copies of one identity are equal. The theory instance
/// is kept as its address, which only tells apart instances alive together;
/// equality never rests on it alone, since the stream number already
/// separates every stream.
///
/// An identity binds results to the slate a correct caller checked. It is not
/// evidence about candidates or verdicts, and it does not defend against a
/// caller that fabricates them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlateId {
    stream: NonZeroU64,
    sequence: u64,
    theory: usize,
}

impl SlateId {
    /// The first slate's identity of a new stream over `theory`, with a stream
    /// number no other stream in the process has. Constant time; one atomic
    /// update.
    ///
    /// Returns `None` only when the process has issued every stream number,
    /// 2^64 − 1 of them.
    #[must_use]
    pub fn first(theory: &Theory) -> Option<Self> {
        let stream = STREAMS
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |next| {
                next.checked_add(1)
            })
            .ok()?;
        Some(Self {
            stream: NonZeroU64::new(stream)?,
            sequence: 0,
            theory: theory.instance_key(),
        })
    }

    /// The identity of the stream's next slate. Returns `None` when the
    /// stream has numbered 2^64 slates.
    #[must_use]
    pub const fn successor(self) -> Option<Self> {
        match self.sequence.checked_add(1) {
            Some(sequence) => Some(Self { sequence, ..self }),
            None => None,
        }
    }

    /// The slate's place in its stream's proposals, from zero.
    #[must_use]
    pub const fn sequence(self) -> u64 {
        self.sequence
    }

    /// Whether this identity was issued for a stream over `theory`'s instance.
    /// Only instances alive together are told apart.
    #[must_use]
    pub fn of(self, theory: &Theory) -> bool {
        self.theory == theory.instance_key()
    }
}

/// One slate's identity, theory and candidates, borrowed for one check.
#[derive(Clone, Copy, Debug)]
pub struct SlateRef<'a> {
    id: SlateId,
    theory: &'a Theory,
    candidates: &'a [Interpretation],
}

impl<'a> SlateRef<'a> {
    /// Borrow a slate's candidates under its identity. Constant time: the
    /// candidates are not inspected, and their stream owns that each belongs
    /// to `theory`.
    ///
    /// Returns `None` when `id` was not issued for a stream over `theory`.
    #[must_use]
    pub fn new(id: SlateId, theory: &'a Theory, candidates: &'a [Interpretation]) -> Option<Self> {
        id.of(theory).then_some(Self {
            id,
            theory,
            candidates,
        })
    }

    /// The slate's identity.
    #[must_use]
    pub const fn id(&self) -> SlateId {
        self.id
    }

    /// The theory every candidate belongs to.
    #[must_use]
    pub const fn theory(&self) -> &'a Theory {
        self.theory
    }

    /// The slate's candidates, in proposal order.
    #[must_use]
    pub const fn candidates(&self) -> &'a [Interpretation] {
        self.candidates
    }
}

#[cfg(test)]
mod tests {
    use super::{SlateId, SlateRef};
    use crate::{AdmissionLimits, FormulaParts, Interpretation, Theory};

    fn theory() -> Theory {
        Theory::new(
            1,
            FormulaParts::default(),
            Vec::new(),
            AdmissionLimits::default(),
        )
        .unwrap()
    }

    #[test]
    fn copies_of_one_identity_are_equal() {
        let id = SlateId::first(&theory()).unwrap();
        let copy = id;
        assert_eq!(id, copy);
    }

    #[test]
    fn successive_slates_of_one_stream_differ() {
        let first = SlateId::first(&theory()).unwrap();
        let second = first.successor().unwrap();
        assert_ne!(first, second);
        assert_eq!((first.sequence(), second.sequence()), (0, 1));
    }

    #[test]
    fn first_slates_of_two_streams_over_one_theory_differ() {
        let shared = theory();
        let one = SlateId::first(&shared).unwrap();
        let other = SlateId::first(&shared).unwrap();
        assert_ne!(one, other);
        assert!(one.of(&shared) && other.of(&shared));
    }

    #[test]
    fn identities_of_streams_over_different_instances_differ() {
        let (left, right) = (theory(), theory());
        let one = SlateId::first(&left).unwrap();
        let other = SlateId::first(&right).unwrap();
        assert_ne!(one, other);
        assert!(!one.of(&right));
    }

    #[test]
    fn a_clone_of_the_theory_is_the_same_instance() {
        let owner = theory();
        let id = SlateId::first(&owner).unwrap();
        assert!(id.of(&owner.clone()));
    }

    #[test]
    fn a_slate_view_refuses_an_identity_from_another_theory() {
        let (owner, foreign) = (theory(), theory());
        let candidates = [Interpretation::new(&owner, []).unwrap()];
        let id = SlateId::first(&foreign).unwrap();
        assert!(SlateRef::new(id, &owner, &candidates).is_none());
    }

    #[test]
    fn a_slate_view_keeps_its_identity_theory_and_candidates() {
        let owner = theory();
        let candidates = [
            Interpretation::new(&owner, []).unwrap(),
            Interpretation::new(&owner, [0]).unwrap(),
        ];
        let id = SlateId::first(&owner).unwrap();
        let view = SlateRef::new(id, &owner, &candidates).unwrap();
        assert_eq!(view.id(), id);
        assert!(view.theory().same_instance(&owner));
        assert!(std::ptr::eq(view.candidates(), candidates.as_slice()));
    }
}
