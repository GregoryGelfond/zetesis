# Finite ranks for extrema

[`RankedExtrema`](../Zetesis/RankedExtrema.lean) describes when `#min` and
`#max` can select integer ranks instead of comparing source values repeatedly.
The result remains an ASP extremum; the rank is an evaluation representation.

A finite row has contribution values and eligibility formulas. Its carrier also
contains the empty result: `#sup` for a minimum or `#inf` for a maximum. Rank
preparation must establish two facts:

- Source order is weakly preserved. Equal source values have equal ranks, and
  an ordered pair cannot have ranks in reverse order.
- Each represented guard has correct strict and inclusive cuts on those ranks,
  including the empty endpoint.

Ranks may merge distinct source values lying in the same interval between guard
bounds. This does not permit recovering the actual selected source value from
the rank alone. A new guard must have its own agreement proved; it may require
a finer partition.

The proof follows the evaluation pipeline:

1. `extreme_map` lifts agreement of a pairwise choice to agreement of finite
   selection. A selected tail value remains inside the carrier.
2. `compatible_choice` and `ranked_extreme` show that weak order preservation
   suffices for integer min/max. If two source values collapse to one rank,
   either tie choice returns that same rank.
3. `cut_agreement` derives all six comparisons from the two guard cuts.
   `guard_agreement` combines this with selection, for every tuple mask.
4. `original` and `frozen` apply the established `AggregateReduct` laws. Frozen
   truth requires the guard on both original and recursively frozen eligibility.
   The second interpretation is arbitrary, not necessarily a subset of the first.

Tuple positions remain positions of complete aggregate keys. Repeated first
values or shared eligibility formulas do not identify complete tuples. The proof
does not change aggregate coalescing, source permissions or endpoint policy.

These are mathematical evaluation laws. They do not verify a typed comparator,
sorting or partition construction, mutable storage, partial propagation, resource
accounting, Rust arithmetic, device transport or WGSL. Those refinements must
establish the stated carrier and agreement obligations.
