# Dividing threshold weights by a common divisor

[ThresholdDivision](../Zetesis/ThresholdDivision.lean) preserves a nonnegative
threshold formula when every weight is divided by the same positive common
divisor and its bound is rounded upward. The conditions may be arbitrary
formulas. Both original satisfaction and frozen-reduct satisfaction are
preserved, for arbitrary interpretations M and J.

The row retains every ordered pair of a natural weight and a condition ID.
Let `d > 0` divide each weight. `divide` maps a weight `w` to `w / d` and leaves
its condition ID in place. `roundedBound` maps bound zero to zero and a positive
bound `B` to `1 + (B - 1) / d`. The divisor need not be greatest. Zero weights
are divisible by every divisor, as `divides_all` records, and repeated condition
IDs keep all their contributions.

For example, weights `[6, 10, 6]` and bound `11` may become `[3, 5, 3]` and
bound `6` using divisor `2`. Selecting the first and third positions reaches
both thresholds: the sums are `12` and `6`. Every other selection agrees too.
Divisibility of the total alone is insufficient: dividing `[1, 1]` by `2`
would erase each contribution and change a threshold at bound `1`.

The argument has three steps:

1. `masked_sum_divide` inducts over the row. For every mask, the original
   selected sum is `d` times the divided selected sum. Each occurrence is
   retained, including repeated conditions and zero weights.
2. `rounded_bound_le_iff` proves the arithmetic comparison law for a positive
   divisor. `masked_threshold_divide` combines it with the sum equality to
   preserve the threshold for every mask.
3. `original` uses the mask of condition truth. `frozen` uses the established
   [ThresholdReduct](../Zetesis/ThresholdReduct.lean) law's two gates: the original
   threshold and the threshold on recursively frozen conditions. The arithmetic
   equivalence preserves each gate without assuming that J is contained in M
   or that M satisfies the original formula.

`equivalent` packages these two results in
`ChoiceIntervals.Equivalent`. Its existing connective and substitution laws
therefore carry the rewrite into a formula context, including implication.

An aggregate comparison can contain more than one primitive threshold. Write
`G(k)` for the threshold at bound `k`. Equality uses `G(k)` together with the
negation of `G(k+1)`; disequality retains the implication from `G(k)` to
`G(k+1)`. Each cut must be rounded separately. With divisor `2` and `k = 3`,
both cuts become `2`: equality is impossible and the implication is true.
Rounding `k` first and then adding one would describe a different comparison.
Greater-than and less-than-or-equal similarly use the successor cut before
division. Signed nonpositive bounds retain their constant handling before the
positive-bound calculation.

The arithmetic is over unbounded natural numbers. Choosing the divisor,
validating the raw input, handling signed bounds, simplifying boundary guards,
checked machine arithmetic, shared-row storage, cancellation and resource
accounting remain implementation obligations. The law does not justify
complementing negative weights in an original aggregate. It does not claim
identical propagation work or search order.

The [implementation correspondence](../../docs/book/lean/correspondence.md#nonnegative-aggregate-thresholds)
records the native extraction gap. This theorem does not update the retained
Boolean generation or establish a refinement of the threshold-aware Rust code.
