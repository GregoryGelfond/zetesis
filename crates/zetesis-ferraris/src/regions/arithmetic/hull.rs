//! Bounded affine proposals over a local truth table, before source checking.
//!
//! The forbidden table uses two words per 128 patterns. A local coefficient
//! mask is a scalar over at most eight variables, not another packed-set owner.
//! These equations describe the affine hull of the allowed patterns; an
//! independent source check must establish every row before it can be retained.

use zetesis_algebra::words::Words;

/// One local GF(2) equation: the selected variables sum to `constant`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct LocalEquation {
    pub(super) coefficients: u8,
    pub(super) constant: bool,
}

/// A proposal whose live equations occupy the initialized prefix.
#[derive(Debug)]
pub(super) struct Hull {
    equations: [LocalEquation; 8],
    count: usize,
    /// Whether the allowed patterns themselves form the represented affine set.
    pub(super) exact: bool,
}

impl Hull {
    /// Borrow the completed equations; unused storage is not a proposed row.
    pub(super) fn equations(&self) -> &[LocalEquation] {
        &self.equations[..self.count]
    }
}

/// Least-bit echelon scratch and the observations needed for exactness.
struct Span {
    pivots: [u8; 8],
    rank: usize,
    allowed_count: usize,
    anchor: Option<u8>,
}

/// Fixed elimination scratch, excluding the caller's retained Hull result.
pub(super) const SCRATCH_BYTES: usize = std::mem::size_of::<Span>();

/// Propose the affine hull of the patterns whose forbidden bits are clear.
///
/// Least-set-bit elimination builds an echelon basis of differences from the
/// first allowed pattern. Descending substitution then builds its orthogonal
/// complement, with constants given by that anchor. Each pivot has no bits
/// below its index; every elimination advances the least remaining set bit.
/// Once the difference span has rank `k`, it is the whole local space; no
/// later pattern can add a pivot. The remaining patterns contribute only to
/// the allowed count, preserving exactness. There are at most `2^k` patterns
/// and `k` visits per elimination/substitution.
/// An empty allowed set emits `0 = 1`, including when `k = 0`.
///
/// All scratch and output have fixed size; no allocation or owner row mutation
/// occurs. The returned value transfers this one initialized result. Every
/// nonzero callback charge precedes the exact bounded operation chunk it names.
/// Binary word operations include operand reads and the destination write;
/// standalone initialization, comparisons and retained-field writes count.
/// Address calculation, finite-loop control, dimension/precondition checks,
/// lossless Boolean/integer representation and ABI transport are not extra
/// logical operations. Borrowing the result and embedding its rows are caller
/// work, outside this proposal.
///
/// The span trace costs `11 + 5q + A + 2B + 2[A>0] + 3J + T + D + 3r + 1`:
/// eight pivot initializations and three scalar initializations; `q` forbidden
/// tests and `A` allowed-count updates; `B <= A` anchor/difference visits before
/// full rank; first-anchor publication; `J` bit tests; `T` pivot inspections;
/// `D` occupied-pivot XORs; `r` new pivots and rank tests; final anchor inspection.
/// The fixed result initialization adds 18 field writes. An empty result adds
/// four writes. Otherwise free-position inspection costs `k`; each of the
/// `e = k-r` equations costs `8 + k + 6r`, and exactness costs three operations.
/// The six occupied-pivot operations include the explicit parity-to-Boolean
/// comparison, as do the four constant operations.
///
/// A new pivot does not also XOR an occupied pivot, so `D <= T-r`. Together
/// with `T <= J <= B*k`, this pays for the added rank tests within the existing
/// input bound: with `q = 2^k` and `Span(k) = 3k + 6 + q*(5k+8)`, it remains
/// `Span(k) + (8-k) + 18 + max(4, 3+k+k*(7k+8))`, at most 12,859 for `k <= 8`.
/// The differences from variable-length reference scratch/output are explicit:
/// `8-k` pivot writes, 18 initialized output fields, and at most `k*(k+1)`
/// parity-to-Boolean comparisons. This bound is not charged instead of the
/// executed trace and changes no caller allowance or preparation ceiling.
///
/// # Errors
/// Returns the first callback refusal unchanged. The refused chunk and all
/// later chunks are unexecuted; no partial proposal is returned.
///
/// # Panics
/// Panics before any callback if `support_len > 8` or `forbidden` does not have
/// exactly `2 * ceil(2^support_len / 128)` words. Padding must be zero; it is
/// outside the pattern traversal and is not inspected here.
pub(super) fn propose<E>(
    support_len: usize,
    forbidden: &[u64],
    before: &mut impl FnMut(u64) -> Result<(), E>,
) -> Result<Hull, E> {
    assert!(
        support_len <= 8,
        "local support has at most eight variables"
    );
    let patterns = 1usize << support_len;
    assert_eq!(
        forbidden.len(),
        2 * patterns.div_ceil(128),
        "padded table length"
    );
    let Span {
        pivots,
        rank,
        allowed_count,
        anchor,
    } = span(support_len, patterns, forbidden, before)?;
    before(18)?; // eight coefficient/constant pairs, count and exactness
    let mut hull = Hull {
        equations: [LocalEquation {
            coefficients: 0,
            constant: false,
        }; 8],
        count: 0,
        exact: false,
    };
    before(1)?;
    let Some(anchor) = anchor else {
        before(4)?;
        hull.equations[0] = LocalEquation {
            coefficients: 0,
            constant: true,
        };
        hull.count = 1;
        hull.exact = true;
        return Ok(hull);
    };
    for free in 0..support_len {
        before(1)?;
        if pivots[free] != 0 {
            continue;
        }
        before(1)?;
        let mut coefficients = 1u8 << free;
        for position in (0..support_len).rev() {
            before(1)?;
            if pivots[position] != 0 {
                before(4)?; // AND, popcount, parity AND, zero comparison
                let bit = (pivots[position] & coefficients).count_ones() & 1 != 0;
                before(2)?; // shift and XOR, including a zero contribution
                coefficients ^= u8::from(bit) << position;
            }
        }
        before(4)?;
        let constant = (coefficients & anchor).count_ones() & 1 != 0;
        before(3)?;
        hull.equations[hull.count] = LocalEquation {
            coefficients,
            constant,
        };
        hull.count += 1;
    }
    before(3)?; // shift, equality comparison, flag write
    hull.exact = allowed_count == 1usize << rank;
    Ok(hull)
}

/// Build the difference span without copying scratch into a second owner.
fn span<E>(
    support_len: usize,
    patterns: usize,
    forbidden: &[u64],
    before: &mut impl FnMut(u64) -> Result<(), E>,
) -> Result<Span, E> {
    before(11)?;
    let mut span = Span {
        pivots: [0u8; 8],
        rank: 0,
        allowed_count: 0,
        anchor: None,
    };
    let mut remaining = (0..=u8::MAX).take(patterns);
    'basis: for pattern in remaining.by_ref() {
        if is_forbidden(pattern, forbidden, before)? {
            continue;
        }
        before(1)?;
        span.allowed_count += 1;
        before(1)?;
        let first = if let Some(first) = span.anchor {
            first
        } else {
            before(2)?; // anchor value and presence
            span.anchor = Some(pattern);
            pattern
        };
        before(1)?;
        let mut difference = pattern ^ first;
        for (position, pivot) in span.pivots[..support_len].iter_mut().enumerate() {
            before(3)?; // shift, AND, zero comparison
            if (difference >> position) & 1 == 0 {
                continue;
            }
            before(1)?;
            if *pivot == 0 {
                before(2)?;
                *pivot = difference;
                span.rank += 1;
                before(1)?;
                if span.rank == support_len {
                    break 'basis;
                }
                break;
            }
            before(1)?;
            difference ^= *pivot;
        }
    }
    // Full rank settles the span, but not whether every pattern was allowed.
    // Finish that count over the same cursor; no later elimination is needed.
    for pattern in remaining {
        if is_forbidden(pattern, forbidden, before)? {
            continue;
        }
        before(1)?;
        span.allowed_count += 1;
    }
    Ok(span)
}

/// One pattern test shared by basis construction and the exactness count.
fn is_forbidden<E>(
    pattern: u8,
    forbidden: &[u64],
    before: &mut impl FnMut(u64) -> Result<(), E>,
) -> Result<bool, E> {
    before(5)?; // quotient, remainder, shift, AND, zero comparison
    let index = usize::from(pattern);
    Ok(Words::<u64, _>::new(forbidden).contains(index))
}
