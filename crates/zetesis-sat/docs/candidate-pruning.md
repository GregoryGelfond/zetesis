# Candidate restrictions, probing and projection indexing

These operations act on the classical candidate query. The stored Ferraris
theory and its reduct remain the acceptance subject. No domain identifier or
corpus metadata selects their behavior.

The [retained cursor](candidate-cursor.md) supplies traversal within one fixed
candidate region. The [library guide](../README.md) describes its public owners
and outcomes.

## Transactional candidate constraints

`StableModels::restrict_candidates` accepts a constraint-only `Theory` whose
semantic atom indices must mean the same atoms as the original theory.
Full gate equivalences append the restriction to the candidate CNF.
Earlier restrictions and exact semantic blocks remain, while the outer cursor
restarts against the new fixed prefix.

An encoding failure restores the previous variables, clauses, submitted shape
counts and live cursor. Work charged before failure remains charged.
Closed iterators, carrier-count mismatches and pending blocking failures return
explicit errors. Equal carrier sizes alone do not establish semantic index
correspondence; that premise belongs to the caller.

A restriction can intentionally exclude answer sets. Exhaustion therefore
describes the constrained candidate region. An optimizer may use a verified
incumbent to exclude strictly worse costs, but must preserve equal costs if it
promises every optimum tie. Previously verified models remain independent of
a later search restriction. A classical candidate cannot supply an incumbent
without an acceptance check.

See [the restriction API](../src/ferraris.rs) and
[transaction tests](../tests/restrictions.rs).

## Failed-literal probing

The initial candidate region skips probing. After a successful restriction,
the new cursor probes after initial unit propagation and before choosing its
branching permutation. It visits original semantic variables in index order.
For each unassigned variable it trials false and true, then undoes each trial's
complete assignment suffix.

Watch movements remain valid under chronological backtracking. A unit conflict
under a trial literal entails its opposite in every satisfying candidate.
Only that entailed opposite becomes a permanent root assignment, and it is
propagated before continuing. An interrupted probe discards the incomplete
cursor state; an interrupted trial is not a conflict certificate.

Probes do not create decision frames. Trial propagation, conflicts, scans and
undo contribute to existing counters and poll cancellation/deadlines.
Scheduling depends on the candidate-query lifecycle rather than source shapes.
Direct CNF solving and fresh inner reduct queries retain their separate procedure.
One pass need not discover every forced literal, and probing can cost more than
the work it saves.

See [root probing](../src/search/probe.rs) and
[cursor tests](../src/search/cursor/tests.rs).

## Exact complete projection index

Each block is a canonical CNF clause containing one signed literal for every
original semantic atom. The cursor checks that shape and inserts the clause's
falsifying key into a binary trie. A complete leaf is rejected exactly when its
semantic prefix is already indexed. Auxiliary variables never enter the key.
The zero-atom case has an empty key and an empty blocking clause.

Each appended block is indexed once. Lookup examines at most the semantic width;
the arena contains at most one node per admitted blocking literal plus its root.
Growth is fallible under the existing CNF shape bound, and insertion/lookup
steps charge work.

After a successful restriction, old blocks are part of the new base CNF.
A fresh empty suffix index needs no copy of them because base solving already
enforces those clauses. The index filters completed assignments; it does not
propagate appended clauses or introduce clause learning.

See [projection indexing](../src/search/cursor/projections.rs).

## Regression and proof boundaries

Maintained tests compare complete initial and restricted/probed traversal with
truth tables, and indexed filters with linear filters under free auxiliary
extensions and empty keys. They exercise both trial polarities, assignment
restoration, two-sided conflict, malformed block rejection, every work cutoff
of a small traversal, interruption during insertion and fused failures.
[Ferraris comparisons](../tests/ferraris.rs) retain an independent minimality
reference.

[IndexedCandidates.lean](../../../proofs/Zetesis/IndexedCandidates.lean) proves
that finite recursive trie insertion/lookup agrees with all exact blocks,
preserves unseen keys and treats equal projections identically. Its refuted-trial
law permits forcing the opposite literal after an actual refutation. That is
a certificate law, not a proof that Rust unit propagation produces the certificate.

[ObjectiveBounds.lean](../../../proofs/Zetesis/ObjectiveBounds.lean) proves scalar
incumbent dominance and complete optimum-tie coverage from a verified original
stable model. It does not verify priority vectors, source tuple semantics or the
bound-formula compiler. Neither module proves the Rust arena, watch/trail state,
undo, Boolean encoding or machine resource accounting. See the
[correspondence chapter](../../../docs/book/lean/correspondence.md) for those
separate obligations.
