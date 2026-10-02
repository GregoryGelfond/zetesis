# A counter for proper subsets

[SubsetCounter](../Zetesis/SubsetCounter.lean) gives an executable binary counter
and proves that its completed visits represent exactly the proper subsets of a
finite interpretation. It supplies constructive coverage to the existing
[finite membership algorithm](../Zetesis/FiniteMembership.lean), without assuming
a complete proposal source.

## Positions and carry

A supplied atom list fixes the counter's positions. The first position is the
least significant bit. The atom type need not be finite or ordered; an ascending
list of distinct numeric atoms is one possible instance.

`increment` clears each leading true bit and sets the first false bit. Reaching
the end with a carry returns `none`, explicitly representing overflow. `full`
tests whether every position is true. The walk stops at that state before
visiting or incrementing it.

`rank` interprets the Boolean positions as a natural number. The proofs establish
that a successful carry preserves width and adds exactly one to the rank, while
overflow occurs exactly at the full vector. Equal-width vectors with equal ranks
are identical. Ranks are proof measures; the carry operates directly on the list
and has no machine-word cardinality restriction.

## The population guard

`population` independently counts true positions. `full_iff_population` proves
that a vector is full exactly when its population equals its width. Together
with `population_bound`, this gives `guard_exact`: under the count invariant,
`present < width` is exactly the nonfull guard.

The module also implements `trackedIncrement`, which decrements a supplied
population whenever it clears a true bit and increments it when setting the
first false bit. `tracked_increment_exact` proves that starting with the actual
population produces the ordinary carry together with its actual new population.
A leading true bit contributes one to that count, so its decrement cannot
underflow. `tracked_increment_preserves` packages the resulting count invariant,
unchanged width and next rank for repeated use. Count correctness is established
by the transition, rather than assumed again after every step.

## Completion and coverage

`walk fuel bits` returns either a completed list of visited vectors or `none` for
unfinished work. Fuel counts increments, not scalar operations. A full vector
completes even with zero fuel; a nonfull vector with zero fuel is unfinished.

`walk_complete` maintains an exact interval invariant. The returned ranks begin
at the input rank and end immediately before the full rank. Each carry consumes
one unit of fuel, retains width and advances the interval's first rank by one.
The full-state case has an empty interval.

Starting with `n` false bits therefore completes with `2^n - 1` increments.
`states` extracts that computed result using the proved completion bound.
`states_ranks`, `states_length` and `states_nodup` establish its consecutive order,
exact visit count and absence of repeated Boolean vectors. `states_exact` then
proves that a vector is visited exactly when it has width `n` and is not full.
For `n = 0`, the initial empty vector is already full and there are no visits.

For atoms `[a, b, c]`, the decoded order is
`[], [a], [b], [a,b], [c], [a,c], [b,c]`. The full candidate is excluded.

## From positions to interpretations

`selected` retains precisely the atoms at true positions, in their supplied
order. Its correspondence theorems require the two input lengths to agree.
`selected_is_selection` and `selection_has_bits` connect this representation in
both directions to `FiniteMembership.selections`.

Distinct supplied atoms are required for the semantic soundness of excluding
only the full vector. With `[a,a]`, the nonfull vector `[false,true]` still denotes
the whole interpretation. Under `Nodup`, `selected_missing` constructs an omitted
atom for every nonfull vector, and `selected_proper` establishes proper inclusion.

Conversely, `proper_has_state` uses the existing finite-selection coverage proof
to represent any semantic proper subset, then constructs its Boolean positions.
Properness excludes the full vector, so `states_exact` supplies its actual visit.
`proper_iff_visited` combines both directions. The ambient atom universe remains
unrestricted.

Finally, `countermodel_search_iff` applies the computed Ferraris reduct evaluator
to those visits. The search succeeds exactly when the existing finite membership
search finds a proper-subset reduct model. This proves equality of existential
success, not equality of the first witness or evaluation cost.

## Refinement boundary

The counter, population updates, completion and semantic coverage are proved for
authored Lean algorithms. They are not an extraction of Rust. Connecting them
to a packed implementation still requires exact selected-atom enumeration,
bit-position correspondence, machine-arithmetic bounds and ownership. Resource
limits, cancellation and allocation failures need their own incomplete outcomes;
the finite increment bound is not a solver work budget. None of these results
certifies a complete source grounder or complete answer-set enumeration.
