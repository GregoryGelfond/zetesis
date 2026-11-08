//! Pure value reductions and the verdict of a possibly incomplete enumeration.
//!
//! Reductions combine values; they do not decide membership, retain witnesses,
//! allocate storage or poll control. The caller authenticates accepted members,
//! completed coverage and the successful effects preceding absorption. Ordered
//! instances additionally require the documented pure order on their values.

use std::cmp::Ordering;
use std::marker::PhantomData;

pub mod sets;

mod sealed {
    pub trait Sealed {}
}

/// A built-in commutative value reduction with an optional absorbing value.
///
/// `combine` is associative and commutative, and `unit` is its identity. If
/// `is_absorbing(value)`, combining that value with any continuation preserves
/// it. These are the value laws of `Zetesis.AxisFolds.Reduction`; they do not
/// commute allocations, control observations or resource-failure traces.
///
/// Implementations are sealed and statically dispatched. No caller-defined
/// membership or narrowing policy is accepted. The built-in operations perform
/// no allocation or control observation. An ordered instance additionally
/// requires a pure, allocation-free total order consistent with value equality;
/// its value laws hold under that contract, which Rust's `Ord` bound alone does
/// not enforce. See [`Minimum`].
pub trait Reduction: sealed::Sealed + Sized {
    /// The value combined independently of witnesses or completion evidence.
    type Value;

    /// Identity for value combination, distinct from an empty-family verdict.
    fn unit(&self) -> Self::Value;

    /// Combine two values using this reduction's commutative operation.
    fn combine(&self, left: Self::Value, right: Self::Value) -> Self::Value;

    /// Whether no further value can change this result.
    fn is_absorbing(&self, value: &Self::Value) -> bool;
}

/// Boolean conjunction: identity true, absorbing value false.
#[derive(Clone, Copy, Debug)]
pub struct All;

/// Boolean disjunction: identity false, absorbing value true.
#[derive(Clone, Copy, Debug)]
pub struct Any;

/// Checked addition of nonnegative counts, with identity `Some(0)`.
///
/// `Some(count)` is exact; `None` records a sum exceeding `u64::MAX`. Overflow
/// propagates through subsequent combinations. Callers map it to their existing
/// typed failure before publishing a count; it never establishes an answer.
/// No value reports early absorption: exact counts require complete coverage.
#[derive(Clone, Copy, Debug)]
pub struct CheckedSum;

/// Minimum of optional ordered values; `None` is the empty minimum's identity.
///
/// `V::cmp` must be a pure, allocation-free total order consistent with equality.
/// These are semantic requirements, not properties enforced by the trait bound.
/// Copy excludes owned destructors from value selection. Equal values retain the
/// left representative, but the reduction laws concern equality of values, not
/// representative identity. No value establishes early optimality.
#[derive(Clone, Copy, Debug)]
pub struct Minimum<V>(PhantomData<fn() -> V>);

impl<V> Minimum<V> {
    /// Construct the statically dispatched minimum operation without storage.
    #[must_use]
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

impl<V> Default for Minimum<V> {
    fn default() -> Self {
        Self::new()
    }
}

/// One minimum value together with its already-computed incoming comparison.
/// The comparison describes this step, not a commutative accumulated receipt.
#[derive(Clone, Copy, Debug)]
pub struct MinimumStep<V> {
    /// Incoming value if strictly smaller; otherwise the previous representative.
    pub value: V,
    /// Incoming versus previous value. With no previous value this is `Less`.
    pub incoming: Ordering,
}

impl<V: Copy + Ord> Minimum<V> {
    /// Combine one incoming value with the current minimum.
    ///
    /// No comparison is made when there is no current value. Otherwise `cmp`
    /// runs exactly once; the returned ordering retains tie information without
    /// a second comparison. Neither witnesses nor fallible effects are handled.
    #[must_use]
    #[inline]
    pub fn step(&self, current: Option<V>, incoming: V) -> MinimumStep<V> {
        let Some(current) = current else {
            return MinimumStep {
                value: incoming,
                incoming: Ordering::Less,
            };
        };
        let order = incoming.cmp(&current);
        MinimumStep {
            value: if order == Ordering::Less {
                incoming
            } else {
                current
            },
            incoming: order,
        }
    }
}

impl<V: Copy + Ord> sealed::Sealed for Minimum<V> {}

impl<V: Copy + Ord> Reduction for Minimum<V> {
    type Value = Option<V>;

    #[inline]
    fn unit(&self) -> Self::Value {
        None
    }

    #[inline]
    fn combine(&self, left: Self::Value, right: Self::Value) -> Self::Value {
        match right {
            Some(incoming) => Some(self.step(left, incoming).value),
            None => left,
        }
    }

    #[inline]
    fn is_absorbing(&self, _value: &Self::Value) -> bool {
        false
    }
}

impl sealed::Sealed for All {}
impl sealed::Sealed for Any {}
impl sealed::Sealed for CheckedSum {}

impl Reduction for All {
    type Value = bool;

    #[inline]
    fn unit(&self) -> bool {
        true
    }

    #[inline]
    fn combine(&self, left: bool, right: bool) -> bool {
        left && right
    }

    #[inline]
    fn is_absorbing(&self, value: &bool) -> bool {
        !*value
    }
}

impl Reduction for Any {
    type Value = bool;

    #[inline]
    fn unit(&self) -> bool {
        false
    }

    #[inline]
    fn combine(&self, left: bool, right: bool) -> bool {
        left || right
    }

    #[inline]
    fn is_absorbing(&self, value: &bool) -> bool {
        *value
    }
}

impl Reduction for CheckedSum {
    type Value = Option<u64>;

    #[inline]
    fn unit(&self) -> Self::Value {
        Some(0)
    }

    #[inline]
    fn combine(&self, left: Self::Value, right: Self::Value) -> Self::Value {
        match (left, right) {
            (Some(left), Some(right)) => left.checked_add(right),
            _ => None,
        }
    }

    #[inline]
    fn is_absorbing(&self, _value: &Self::Value) -> bool {
        false
    }
}

impl<L: Reduction, R: Reduction> sealed::Sealed for (L, R) {}

/// Products combine both coordinates; both must absorb to absorb the product.
/// No coordinate's input is skipped by this value operation.
impl<L: Reduction, R: Reduction> Reduction for (L, R) {
    type Value = (L::Value, R::Value);

    #[inline]
    fn unit(&self) -> Self::Value {
        (self.0.unit(), self.1.unit())
    }

    #[inline]
    fn combine(&self, left: Self::Value, right: Self::Value) -> Self::Value {
        (
            self.0.combine(left.0, right.0),
            self.1.combine(left.1, right.1),
        )
    }

    #[inline]
    fn is_absorbing(&self, value: &Self::Value) -> bool {
        self.0.is_absorbing(&value.0) && self.1.is_absorbing(&value.1)
    }
}

/// The result of a reduction over an authenticated family of accepted members.
///
/// This carrier does not itself prove membership, coverage or witness validity.
/// An absorbed result requires a nonempty accepted prefix and successful effects
/// for its deciding step. It survives later stopping. Every other value requires
/// complete coverage; empty coverage is distinct from the reduction's unit.
/// Witness and stop payloads move unchanged and impose no allocation policy.
#[derive(Debug)]
pub enum Verdict<V, W, E> {
    /// A nonempty accepted prefix fixed the value for every continuation.
    Absorbed {
        /// Absorbing value of the reduction.
        value: V,
        /// Caller-selected evidence for the deciding accepted prefix.
        witnesses: W,
    },
    /// The fold of a completed, nonempty family without earlier absorption.
    Complete(V),
    /// Complete coverage established that the family has no members.
    Empty,
    /// Neither accepted absorption nor complete coverage was established.
    Undecided(E),
}

impl<V, W, E> Verdict<V, W, E> {
    /// Classify completed coverage that did not already establish absorption.
    ///
    /// The caller supplies `None` exactly when no member was accepted; `Some`
    /// contains the fold of a nonempty family, even when it equals the unit.
    /// Calling this requires authenticated completion, not merely an empty
    /// queue after a stop. No value or witness is synthesized, cloned or read.
    #[must_use]
    pub fn completed(value: Option<V>) -> Self {
        match value {
            Some(value) => Self::Complete(value),
            None => Self::Empty,
        }
    }
}

#[cfg(test)]
mod tests;
