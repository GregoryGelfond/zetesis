# Streaming membership through a frozen reduct

[`CounterSearch.lean`](../Zetesis/CounterSearch.lean) connects the actual
positional counter to checked formula evaluation. It computes the original
truth mask once, checks original satisfaction, then tests proper subsets against
that same mask. The namespace is `Zetesis.Refinement.CounterSearch`.

## One candidate at a time

`search` queries the current counter state and returns immediately when it finds
a countermodel. A false query advances the counter. Reaching the full state ends
the search without testing that state, because it is not a proper subset.
The computation retains no list of earlier visits. Its recursive Lean form is
not a constant-stack or allocation claim about a compiled implementation.

`search_exact` relates this computation to the already proved finite counter
walk. For a completed walk and successful queries, streaming produces the same
existential verdict as searching the complete visit list. The generic lemma
states its query premise explicitly; the membership composition discharges it
with `query_exact`, using the checked evaluator's admitted-index theorem.

`query` reads a supplied frozen mask. `check` produces that mask by evaluating
the original candidate exactly once. The formula table must have admitted child
indices, and every asserted root must name a table entry. Those structural
premises establish successful reads; they do not assume correct evaluation.

## Completion and limited work

The search allowance counts subset queries. If it reaches zero before a witness
or full-state completion, the result is `none`. A failed indexed query also
returns `none`. This model distinguishes both from completed truth, but does not
give them separate error labels. It does not model a Rust work budget,
cancellation, allocation failure or the precedence of those effects.

For a candidate with `n` distinct atoms, the proved counter bound is `2^n - 1`.
`check_exact` establishes completion at that bound and equality with the finite
membership decision. `check_iff_answer_set` composes that result with the Ferraris
definition. No complete-enumeration or evaluator-agreement oracle is assumed.

`search_completed_extension` and `check_completed_extension` prove that extra
fuel cannot alter a completed verdict. `completed_check_exact` compares an
arbitrary completed run with the proved complete run at their common extended
allowance. Thus every completed verdict is correct, including an early rejection
after original failure or discovery of a countermodel. `accepted_is_answer_set`
states positive soundness without requiring the caller to supply the full bound.
Nothing follows about membership from an unfinished result.

## Relation to packed storage and Rust

The [packed subset proof](packed-subsets.md) derives the corresponding complete
visit sequence from 64-bit set/clear operations, and derives distinct selected
coordinates from a finite scan. This module instead isolates streaming,
short-circuiting, frozen-mask reuse and checked formula reads, using the shared
positional counter. Both connect to `SubsetCounter` and the same ASP semantics.
The [packed streaming proof](packed-counter-search.md) composes those writes
with this control and derives exact completed membership. Connecting the
composed algorithm to extracted Rust operations remains an implementation
obligation.

The corresponding Rust behavior is the exhaustive checker in
`crates/zetesis-ferraris/src/oracle.rs::check`, not every optimized solving route.
Its original satisfaction check, retained mask, subset carry and early
countermodel return motivate these boundaries. The authored Lean algorithm does
not establish Rust ownership, vector behavior, machine bounds, exact resource
charges, grounding, parallel scheduling or shader correctness.
