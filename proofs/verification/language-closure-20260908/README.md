# Count-head activity and objective transport

The clean pinned Lean 4.33.1 build, strict checks of all 74 semantic modules and
complete axiom audit passed. The library now contains 834 theorems: all 819 prior
declarations plus nine `CountHeadActivity` laws and six `ObjectiveTransport` laws.
`commands.json` records the actual invocations and outcomes. The continuity
record checks unchanged prior module bytes, declaration locations and transitive
axiom sets; prior records remain retained rather than regenerated.

`CountHeadActivity` distinguishes head permission from complete-tuple activity
without assuming a tuple/atom bijection. Its original and frozen laws compose
with `HeadMeasures` in unchanged theory contexts. `ObjectiveTransport` separates
completed numeric presence carriers from model-relative contribution activation,
preserving normalized keys and cost vectors under explicit correspondences.

These are semantic library results. They do not verify the Rust recognizers,
source joins, clingo priority-layout correspondence, allocation/accounting or
GPU execution. The objective source regression covers 243 interpretation pairs
with J contained in M; the count-head source regressions separately cover 1,142
original/frozen pairs without that subset restriction. Neither test count is a
Lean theorem count or a compiler-refinement claim.
