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
Earlier restrictions remain in the CNF and exact semantic exclusions remain in
their separate index. The outer cursor restarts against the new fixed prefix
without rebuilding that index or placing its keys in watch lists.

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

Each exclusion is the fixed-width Boolean key of one original interpretation.
The cursor inserts that key directly into a binary trie; no equivalent blocking
clause is allocated. A complete leaf is rejected exactly when its semantic
prefix is already indexed. Auxiliary variables never enter the key. The
zero-atom case has one empty key, represented by an explicit exclusion flag.

Lookup examines at most the semantic width. Admission charges one logical clause
unit and the key width in literal units; the arena contains at most one node per
admitted exclusion bit plus its root. These are shape bounds, not byte limits.
Growth is fallible and amortized. A new suffix is attached only after all its
nodes are constructed. Failure restores the previous key set and admission
counts while retaining spent work; insertion and lookup poll control.

Each node contains two optional 32-bit child links, occupying eight bytes.
A present link stores the positive successor of its zero-based arena position;
absence remains `None`. Neither semantic atom IDs nor key width are narrowed.
The complete planned arena length must fit `u32::MAX` nodes before suffix
reservation or publication. A larger length is a typed shape refusal
(`Incomplete::Admission(AdmissionError::Overflow)`), including when the caller
raises the logical admission limits. Host arithmetic overflow is still reported
as `CounterOverflow`, and a representable but failed reservation as `Allocation`.
Control is checked before these bounds; prior prefix work remains charged.

The default 12,582,912 literal units bound the arena to at most 12,582,913 nodes,
so the compact representation adds no restriction within that default. This
does not change how many logical exclusions the literal limit admits or create
an allocated-byte ceiling. Spare capacity and allocator overhead remain outside
those logical units; a failed insertion can retain newly reserved capacity even
though its unpublished nodes and admission counts are rolled back. Smaller
nodes do not by themselves establish a whole-solver memory or runtime gain.

After a successful restriction, the same index still excludes every earlier
projection. Only traversal state is rebuilt. Final exact membership is checked
on the completed assignment independently of DPLL state, over the same stored
index. This does not introduce clause learning or propagate the exclusions.

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

[OptionalIndex.lean](../../../proofs/Zetesis/OptionalIndex.lean) proves bounded
successor encoding preserves index identity and absence, and that replacing a
link commutes with the representation. For the compact arena, the implementation
must additionally establish that each decoded link is allocated, complete paths
have the fixed semantic width, and failed suffix construction changes no old
link. Starting at the same root, induction on the remaining key width then
relates represented link traversal to the same exact-key lookup. The laws do not
prove Rust layout, reservation, rollback or work/cancellation behavior. Retained
cursor coverage is separately specified in
[CandidateCursor.lean](../../../proofs/Zetesis/CandidateCursor.lean).

[ObjectiveBounds.lean](../../../proofs/Zetesis/ObjectiveBounds.lean) proves scalar
incumbent dominance and complete optimum-tie coverage from a verified original
stable model. It does not verify priority vectors, source tuple semantics or the
bound-formula compiler. Neither module proves the Rust arena, watch/trail state,
undo, Boolean encoding or machine resource accounting. See the
[correspondence chapter](../../../docs/book/lean/correspondence.md) for those
separate obligations.
