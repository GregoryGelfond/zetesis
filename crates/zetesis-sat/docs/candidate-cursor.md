# Retaining candidate search state

The outer query proposes classical models of the original Ferraris theory.
Its retained cursor resumes a fixed traversal instead of starting over after
each semantic interpretation is blocked. The reduct acceptance criterion
remains separate from candidate generation.

See the [search library](../README.md) for public APIs and the
[semantic chapter](../../../docs/book/architecture/semantics.md) for the
distinction between original satisfaction and reduct minimality.

## Candidate regions and exact blocking

A candidate region has a fixed base query. Initial admission creates the first
region; a successful candidate restriction creates another containing the
previous clauses and the persistent exact exclusion index. Within a region, the cursor retains
its watch registry, assignment trail, decision frames and branching permutation.

The base clause and variable counts are fixed. Exact exclusions introduce no
variables or clauses. They filter complete assignments before semantic projection
and candidate accounting. A rejected leaf resumes the same traversal. Every
returned full assignment is checked against the base clauses and, independently
of traversal state, the persistent exclusion index.

A complete semantic-prefix block is indexed by its exact Boolean key.
Auxiliary variables are absent from that key, so several auxiliary extensions
cannot produce duplicate semantic candidates. This argument does not require
unique auxiliary extension, although the encoder retains full gate equivalences.
Tests compare the index with an independent linear exclusion reference.

Within a region, completed leaves are not revisited and every unvisited base
model remains in a pending branch. False-first traversal and occurrence-based
variable ordering are deterministic. Ordering is computed once for each base
query, so its output order need not match repeated independently reordered queries.

## Acceptance and stopping

The general membership path checks original satisfaction and asks whether a
proper subset satisfies the candidate's frozen Ferraris reduct. An UNSAT
countermodel query establishes minimality; a returned countermodel is independently
checked for proper containment and reduct satisfaction. Optional complete-theory
certificates can discharge membership under their own validated premises;
residual cases still use the general check. Neither route treats a classical
candidate as an answer set merely because the outer query returned it.

Scalar iteration installs an exact block after a completed membership decision,
excluding that semantic interpretation from future candidates, whether it was
stable or nonminimal.
An inconclusive membership check terminates the iterator without blocking the
candidate and continuing. If block storage fails after stability was established,
the proved model is returned first and the pending failure follows on the next
call. Exhaustion is never inferred from that failure.

Batched iteration installs exact exclusions as proposals enter its pending
owner, before membership checking. An exclusion records proposal identity, not
stability. Checker, shape and scratch failures retain that owner for permitted
retries; exact-search and delayed blocking failures follow the terminal contract.
No failure establishes exhaustion.

Candidate restrictions deliberately narrow the search region. Exhaustion means
that region has been covered; an optimizer must separately justify any claim
about all answer sets or all optimum ties.
See [restriction and projection contracts](candidate-pruning.md).

## Work and retained storage

The original CNF and accumulated restrictions share `AdmissionLimits`.
The exact exclusion owner separately admits distinct entries, logical nodes
and named header/vector capacity through `ProjectionLimits`. Exclusions consume
no submitted clause or literal units. A duplicate key needs no new entry; the
empty projection uses one entry and zero nodes. Capacity includes actual spare
vector slots and conservative old/new overlap during growth, excluding allocator
metadata and other SAT owners. A failed insertion changes no complete key or
logical node count, although its enlarged capacity can remain retained.
The retained outer state stays live while an inner query runs, and each owns
bounded variable/watch/trail arrays. No additional base-clause copy is required.
Resumption, undo, complete-leaf validation and exact-index operations charge
the cumulative work counter and poll control.
The history work receipt counts the trie part of those same steps, including
incomplete attempts; it must not be added to the search total a second time.

Exact blocks act at completed leaves. This mechanism does not add clause
learning, propagated incremental nogoods or a guaranteed speedup. Retaining a
traversal avoids restarting its completed decision regions but does not remove
exponential candidate or reduct-search behavior.

Implementation boundaries are in [the cursor](../src/search/cursor.rs),
[projection indexing](../src/search/cursor/projections.rs) and
[Ferraris membership](../src/ferraris.rs).

## Regression and proof boundaries

[Cursor tests](../src/search/cursor/tests.rs) compare complete truth-table model
sets, including free auxiliary extensions, appended clauses, empty/root-forced
queries, cancellation between yields and exact cumulative work/decision ceilings.
[Ferraris tests](../tests/ferraris.rs) compare original membership and complete
models with an independent reference.

[CandidateCursor.lean](../../../proofs/Zetesis/CandidateCursor.lean) specifies a
finite forest. It proves remaining-leaf accounting, preservation on interruption,
completion with sufficient mathematical fuel, duplicate-free semantic projection
coverage and a separate original-Ferraris stability filter theorem. It does not
prove Rust CNF-to-tree correspondence, watch/trail/backtracking refinement,
machine resource accounting or oracle implementation correctness.
