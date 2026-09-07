# Native search toward the full compatibility milestone

Status: implemented as the native `zetesis-sat` crate and exposed through
`zetesis --oracle countermodel` (selected automatically when needed). The existing exhaustive candidate and proper-subset
paths remain independent semantic references. SAT uses eager CPU formula
lowering; this does not implement lazy GPU search or full source compatibility.

Use one bounded native SAT kernel for two different queries over the existing
finite Ferraris theory:

1. **Candidate query:** Tseitin-encode the classical theory and find a model M.
2. **Countermodel query:** freeze every formula node's classical truth in M,
   encode that reduct, require J to be a proper subset of M, and search for J.

An inner satisfying assignment supplies a rejection witness, checked again by
the independent formula evaluator. Complete inner UNSAT certifies minimality.
An exhausted budget or cancellation is inconclusive. J is tested against the
frozen reduct; it does not undergo another stable-model query.

Tseitin variables must represent full connective equivalences, with asserted
root truth. This gives each semantic atom interpretation one auxiliary extension.
Only original atoms participate in the subset relation and minimization. The
strict-subset clause requires at least one M-true atom to be false in J; M-false
atoms are fixed false. For the empty M there is no proper subset.

Signed predicate identities occupy distinct semantic atom positions. Source
admission adds coherence roots before the original theory reaches either query;
those roots never provide support. Candidate blocking and proper-subset tests
retain both positions. Query-literal polarity is a separate CNF representation
feature and must not be confused with an ASP predicate's classical sign. The
[Lean identity/coherence laws](../../proofs/Zetesis/StrongNegation.lean) justify
this denotational encoding under injectivity and complete opposite-pair coverage,
not the Rust index or source-registry implementation.

After each completed candidate verdict, block exactly its semantic atom
assignment in the outer query. Do not block an auxiliary assignment instead,
and do not generalize a rejection into a learned semantic clause without a
valid explanation. Exact blocking preserves complete enumeration. Keep the
existing exhaustive oracle for small differential campaigns and preserve the
Horn least-closure specialization where its stronger assumptions hold.

The kernel uses two watched literals, an explicit trail and decision
levels, deterministic occurrence-prioritized branching and chronological backtracking.
Initial unit propagation precedes a bounded complete variable permutation; both
truth branches remain mandatory. Query-local constant/alias compaction occurs
after candidate truth masking, preserving the original reduct theory.
Clause learning follows after coverage, backtracking and propagation are
verified. Every work/decision/allocation ceiling must preserve the distinction
between UNSAT and incomplete search.

The normal-rule bridge additionally supports double-negated producer guards.
For each atom a, require `not not (a -> OR producer_bodies)`. Stable normal
models satisfy this condition by their least-closure derivation. A satisfied
guard's reduct is true under every subset, so the guard prunes outer candidates
without removing inner countermodels. Headless constraints provide no support,
missing producers mean falsum, and self-supported cycles still need minimality.
This specialization is separately tested against both the unguarded formula
translation and the independent lazy CPU oracle.

Required correctness boundaries are CNF extension equivalence; frozen-reduct
encoding; subset/strictness constraints; SAT search coverage; atom-only blocking;
and honest limits. `FerrarisMask.lean` supplies the denotational frozen-mask law,
but does not establish these new implementation obligations.

The [clause-validation laws](../../proofs/Zetesis/ClauseValidation.lean) establish
that an executable scan may stop at the first true literal while still requiring
every clause. They assume fixed, total literal truth and bound only the inspected
literal occurrences. They do not establish complete assignment construction,
index validity, the Rust counters, cancellation behavior, search coverage or
Ferraris reduct encoding. The original reduct remains authoritative after this
classical-query validation optimization.

## First acceptance target

The unchanged original eight-queens variant-01 case now passes complete parity
with clingo: all 92 models through finite variable binding, conditional exact-one
choices, diagonal arithmetic/comparisons and source output metadata. This first
source-to-answer result uses eager CPU formula admission and reduct search.

A separately labeled synthetic S0 regression uses 64 queen atoms, 728
binary attack clauses (224 row, 224 column, 280 diagonal) and 16 row/column
existence clauses. Choice support makes selected queens required in the frozen
reduct, so the inner query should refute countermodels by propagation. That
fixture also produces 92 models but is distinct from the original-source test. Measure it before
claiming a performance improvement; chronological DPLL and general reduct
minimality still have exponential worst cases.

The remaining corpus additionally needs tuple-set aggregates, broader optimization forms,
generated numeric values, exact anonymous-variable safety/projection semantics and observable
model/count/cautious contracts. An efficient SAT kernel enables those features;
it does not establish their source semantics or complete compatibility itself.
