# Signed aggregate ranges

`AggregateRanges` supplies the mathematical range argument for parallel signed
aggregate reduction. A finite list represents contribution occurrences, including
equal weights at distinct positions. A selected subcollection is a permutation
of a sublist, so selection cannot increase any weight's multiplicity.

The sum of all negative contributions is a lower bound for every selected sum;
the sum of all positive contributions is an upper bound. Under these separate
bounds, every intermediate result in an arbitrary finite addition tree remains
in the admitted interval. A cancellation example proves why checking only the
final signed total is insufficient.

The module adds seven laws. The complete pinned Lean 4.33.1 build, strict module
checks and axiom audit cover 819 laws across 72 semantic modules. The continuity
record checks that the previous 812 theorem names, source locations, semantic
module bytes and transitive axiom sets remain unchanged.

These laws concern mathematical integers and do not select a machine width.
Applying them to the implementation still requires correspondence for the
contribution carrier, occurrence selection, the actual reduction tree, integer
encoding and checked arithmetic. The audit does not verify Rust, WGSL, device
execution, source entailment, eligibility computation or resource accounting.
The reduction's occurrence-preserving leaf premise is essential; overlapping
subgroups cannot count a shared occurrence twice.
