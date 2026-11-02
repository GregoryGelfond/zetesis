//! Uniform positive weights reduce a threshold to a bound on its atom count.
//!
//! This helper does not authenticate conditions, distinctness or ownership.
//! Its caller supplies those source checks and the operation-admission callback.

/// The threshold itself gives a lower bound; its negation gives an upper bound.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum BoundSide {
    Lower,
    Upper,
}

/// A source shape left to the ordinary narrower, rather than a new refutation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Decline {
    NonPositive,
    ZeroWeight,
    BeyondCardinality,
}

/// The normalized bound or the reason this source does not supply one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum UniformBound {
    Declined(Decline),
    Valid { value: u64 },
}

/// Normalize `weight * count >= bound`, or its negation, without wide division.
///
/// Positive bounds use `ceil(bound / weight) = (bound - 1) / weight + 1`.
/// A ceiling outside `1..=cardinality` is trivial and declines; it never becomes
/// a refutation here. Upper bounds subtract one from that positive ceiling.
/// Callback failures propagate unchanged, before any operation in their chunk.
///
/// Work counts logical 64-bit operations, not instructions or meter overhead.
/// The input-only bound is
/// `4 + 1 + 3 + 4 + 128 * (13 + 4) + 3 + 2 + 2 = 2195`:
/// sign/zero, weight, predecessor, division initialization and bits, successor,
/// cardinality checks, then value and tag. Refusals spend only their prefix,
/// including one result tag. Each bit spends 13, plus 4 only if it subtracts.
pub(super) fn uniform_bound<E>(
    bound: i128,
    weight: u64,
    cardinality: u64,
    side: BoundSide,
    before: &mut impl FnMut(u64) -> Result<(), E>,
) -> Result<UniformBound, E> {
    // This is a bit-preserving register representation, independent of host
    // endianness, not an absolute value or a narrowing numeric conversion.
    let [
        lo0,
        lo1,
        lo2,
        lo3,
        lo4,
        lo5,
        lo6,
        lo7,
        hi0,
        hi1,
        hi2,
        hi3,
        hi4,
        hi5,
        hi6,
        hi7,
    ] = bound.to_le_bytes();
    let mut numerator = [
        u64::from_le_bytes([lo0, lo1, lo2, lo3, lo4, lo5, lo6, lo7]),
        u64::from_le_bytes([hi0, hi1, hi2, hi3, hi4, hi5, hi6, hi7]),
    ];
    before(2)?;
    if numerator[1] >> 63 != 0 {
        before(1)?;
        return Ok(UniformBound::Declined(Decline::NonPositive));
    }
    before(2)?;
    if numerator[0] | numerator[1] == 0 {
        before(1)?;
        return Ok(UniformBound::Declined(Decline::NonPositive));
    }
    before(1)?;
    if weight == 0 {
        before(1)?;
        return Ok(UniformBound::Declined(Decline::ZeroWeight));
    }

    // The positive two-word value has a predecessor even when its low limb is
    // zero. The low subtraction wraps; its borrow is taken from the high limb.
    before(3)?;
    let borrow = u64::from(numerator[0] == 0);
    numerator[0] = numerator[0].wrapping_sub(1);
    numerator[1] -= borrow;
    let mut quotient = divide(numerator, weight, before)?;

    // The predecessor is below 2^127, so this two-word increment cannot overflow.
    before(3)?;
    quotient[0] = quotient[0].wrapping_add(1);
    let carry = u64::from(quotient[0] == 0);
    quotient[1] += carry;
    before(1)?;
    if quotient[1] != 0 {
        before(1)?;
        return Ok(UniformBound::Declined(Decline::BeyondCardinality));
    }
    before(1)?;
    if quotient[0] > cardinality {
        before(1)?;
        return Ok(UniformBound::Declined(Decline::BeyondCardinality));
    }
    before(2)?;
    let value = match side {
        BoundSide::Lower => quotient[0],
        BoundSide::Upper => quotient[0] - 1,
    };
    Ok(UniformBound::Valid { value })
}

/// Restoring division of a two-word numerator by a nonzero word.
///
/// Before each bit, the remainder is below the divisor. Shifting in a bit makes
/// it smaller than twice the divisor, so one subtraction restores the invariant.
/// The quotient is the processed prefix divided by the divisor. The remainder
/// needs 65 bits; neither a wide division nor a hidden multiplication is used.
fn divide<E>(
    numerator: [u64; 2],
    divisor: u64,
    before: &mut impl FnMut(u64) -> Result<(), E>,
) -> Result<[u64; 2], E> {
    before(4)?;
    let mut quotient = [0_u64; 2];
    let mut remainder = [0_u64; 2];
    for word in numerator.iter().rev() {
        for offset in (0..64).rev() {
            before(13)?;
            let bit = (word >> offset) & 1;
            let carry = remainder[0] >> 63;
            remainder[0] = (remainder[0] << 1) | bit;
            remainder[1] = carry;
            quotient[1] = (quotient[1] << 1) | (quotient[0] >> 63);
            quotient[0] <<= 1;
            let wide = remainder[1] != 0;
            let at_least_divisor = remainder[0] >= divisor;
            let take = wide || at_least_divisor;
            if take {
                before(4)?;
                let borrow = u64::from(remainder[0] < divisor);
                remainder[0] = remainder[0].wrapping_sub(divisor);
                remainder[1] -= borrow;
                quotient[0] |= 1;
            }
        }
    }
    Ok(quotient)
}
