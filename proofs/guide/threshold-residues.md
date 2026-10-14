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

`interval_intersection_refines` explains when a divisor scan can stop without
refuting. If the final divisor divides the divisor of the scanned prefix, write
`partial = final * factor`. Every point `B + partial*k` is the same point as
`B + final*(factor*k)`. Thus an interval intersection already found for the
partial progression survives further gcd refinement, with the base and interval
held fixed. The law even holds without positivity; its intended gcd application
uses positive divisors. This permits skipping the rest of that refutation check,
not accepting a subset sum or a candidate. Establishing the scan's divisibility
invariant and its Rust early-exit correspondence remains separate work.

`wide_interval_intersects` gives a sufficient intersection condition without a
coefficient scan. If a positive coefficient `w` fits inside the width `U-L`, then
`[L,U)` contains a point `B+w*k`, provided `B≤L`. Divide `L-B` by `w`: an exact
quotient reaches `L`; otherwise the next quotient reaches a point less than `w`
beyond `L`. `open_coefficient_intersects` carries that witness to every divisor
of `w`. An undecided coefficient therefore proves that the residual-divisor test
cannot refute this interval. It does not prove that a selected sum exists.

The Rust consumer applies this witness to the first undecided grouped coefficient
in its existing scan, after mandatory events have updated the numeric sums. The
term visit is charged once; decided conditions supply no residual witness. The
check runs once for each coalesced dirty row, rather than inspecting a witness at
every notification. Later decisions and tighter requirements reconsider the row
through the existing propagation callbacks. These event and representation
obligations remain separate from the arithmetic laws.

The Rust check uses coefficients grouped by identical condition. Its divisor can
be stronger than the divisor of the ungrouped weights: two occurrences with
weights 2 and 3 under one condition contribute either 0 or 5. The theorem applies
to the supplied row; preservation under runtime grouping is a separate premise.

The Rust implementation coalesces arithmetic notifications separately from bound
transport and runs them after node, bound-transfer, witness and support propagation
settles. Each dirty row is then checked once; the check creates no further events.
An earlier arithmetic refutation remains valid after more decisions or tighter
bounds, since these only restrict its possible completions. This scheduling
argument is separate from the proved arithmetic statements above.

The mathematical arithmetic is unbounded. The Rust implementation must establish
grouped-row correspondence, its greatest-common-divisor computation, the interval
test, checked machine arithmetic and complete processing of pending truth events
before reading the sum. Queue ordering and termination, cancellation and memory
accounting remain implementation obligations. This is an original-candidate
restriction; it neither changes the frozen reduct nor establishes answer-set
membership.
