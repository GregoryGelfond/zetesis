# Divisibility of a remaining aggregate sum

[ThresholdResidues](../Zetesis/ThresholdResidues.lean) gives a necessary condition
for a partially decided weighted sum. Let `B` be the contribution already known
true. If `d` divides every undecided coefficient, every compatible complete sum
has the form `B + d*k` for a natural number `k`. When none of those values lies
in the required interval, the candidate region has no satisfying completion.

For example, require a sum of 17 from one weight-1 condition and several weight-6
and weight-10 conditions. If the weight-1 condition becomes false, every remaining
sum is even. Ordinary lower and upper bounds may still enclose 17; divisibility
rules it out. This does not resolve every unreachable sum: weights 6, 10, 15 and
15 have common divisor 1, yet cannot sum to 17.

The proof has three steps:

1. `sum_decomposition` separates the known contribution from the selected
   undecided contribution. Compatibility fixes the truth of known conditions;
   every occurrence of an undecided condition remains in the row.
2. `sum_residue` applies exact weight division to that undecided row.
   `residual_divisible` states the resulting divisibility of the remainder.
3. `interval_refuted` excludes a complete reading when the arithmetic progression
   misses the interval. `formula_interval_refuted` translates this result into
   original satisfaction of the lower threshold and failure of the upper one.

The conditions need not be independent. The progression may contain values that
no interpretation realizes; it only has to contain every realizable sum. A
nonempty intersection therefore supplies no model or answer-set certificate.

The Rust check uses coefficients grouped by identical condition. Its divisor can
be stronger than the divisor of the ungrouped weights: two occurrences with
weights 2 and 3 under one condition contribute either 0 or 5. The theorem applies
to the supplied row; preservation under runtime grouping is a separate premise.

The mathematical arithmetic is unbounded. The Rust implementation must establish
grouped-row correspondence, its greatest-common-divisor computation, the interval
test, checked machine arithmetic and complete processing of pending truth events
before reading the sum. Queue termination, cancellation and memory accounting
remain implementation obligations. This is an original-candidate restriction;
it neither changes the frozen reduct nor establishes answer-set membership.
