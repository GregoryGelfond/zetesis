# Retaining candidate search state

The outer classical query is fixed within each candidate region. Admission
creates the first region; a successfully appended candidate restriction starts
a new region containing all prior clauses and exact semantic blocks.
Every completed reduct check then blocks precisely one semantic interpretation.
Restarting DPLL after each block revisits previously explored decision regions.
The retained cursor keeps the original watch registry, assignment trail,
decision frames and branching permutation, and resumes after the yielded leaf.

The initial clause count and variable count are fixed. Later clauses introduce
no variables and do not change the original prefix. They are checked as filters
at complete assignments before semantic projection or candidate accounting.
They are not dynamically installed into watch lists. A rejected leaf resumes the
same traversal. Every returned full assignment is independently checked against
both original clauses and appended filters. Complete semantic-prefix blocks
are indexed by an exact binary trie; generic internal cursor tests also retain
the linear reference filter.

After a successful candidate restriction, root failed-literal probing may
establish additional entailed root assignments
before traversal and ordering. It does not remove any satisfying base-CNF
assignment. Within the retained traversal, completed leaves cannot be
revisited, and every unvisited base model remains in a pending branch. Exact
semantic filters eliminate duplicate projections even if several auxiliary
assignments extend one interpretation. The implementation does not need unique
auxiliary extension for this argument. The existing encoder still provides full
equivalences for every allocated auxiliary gate.

The reduct remains the acceptance authority. Each candidate is checked against
the original Ferraris theory, receives a fresh frozen-reduct countermodel query,
and is accepted only after that query completes UNSAT. SAT countermodels remain
independently checked. An inconclusive reduct check terminates enumeration. The
cursor never blocks it and continues. A successfully proved model may precede a
pending blocking-storage failure; that existing API behavior remains explicit.

The same shape ceilings bound the original state and accumulated clauses.
The retained outer state remains allocated while an inner query runs; each has
its own bounded variable/watch/trail arrays. No base-clause copy is added. All
resumption, undo, leaf validation and appended-filter operations charge the
existing cumulative work counter and poll control. The false-first traversal
and initial occurrence-based variable ordering remain deterministic. Ordering
is computed once for the base query, so enumeration order can differ from a
sequence of independently reordered fresh queries.

There is no claim of clause learning, propagated incremental nogoods, or a
universal speedup. An appended block acts only at completed leaves. The change
removes repeated outer traversal work while retaining the existing inner
algorithm, all original-atom subset tests and all logical completion obligations.

## Bounded measurement

A release-mode harness used the unchanged pinned kr-domains source
`standalone/n-queens/variant-02.lp`, SHA-256
`1c5451a6cbafe0f433347b5253fc4876e7c56b0a16f2237b513d9c62ee6142a2`.
The admitted theory had 80 atoms, 4,516 nodes and 1,617 roots. Its original outer
query had 2,671 variables and 9,310 clauses. This is a measured workload; none of
these constants or source identifiers appears in the search policy.

| Search state | Complete models | Charged work | Coverage |
| --- | ---: | ---: | --- |
| Restart after each semantic block | 11 | 100,000,000 | Incomplete |
| Retain the original traversal | 92 | 30,007,222 | Exhausted |

The retained run made 93 outer queries, 92 reduct queries, 17,660 decisions,
2,653,599 propagations and 17,661 conflicts. It found no countermodels. Every
returned board, and the complete set of 92 boards, matched an independent
enumeration of all 8! column permutations with both diagonal constraints.
No default limit changed. The measurement used the prior checkpoint's compiled
frontend with the new SAT source; the installed full-corpus checkpoint is a
separate integration result. No wall-clock speedup is claimed.

## Checks and formal boundary

Portable tests enumerate all 522 canonical CNFs through two variables plus
noncanonical input, compare complete truth-table model sets, and exercise
multiple free auxiliary extensions, appended units/binary/empty clauses,
zero-variable/root-forced completion, cancellation between yields and exact
cumulative work/decision ceilings. Existing complete Ferraris differential and
renaming tests remain in place. Six independent Boolean choices also compare
complete sets and charged work against restarting after every exact block.

[CandidateCursor.lean](../../../proofs/Zetesis/CandidateCursor.lean) provides an
executable finite-forest specification. It proves exact remaining-leaf
accounting, preservation on interruption, finite completion with sufficient
mathematical fuel, duplicate-free complete semantic projection coverage, and a
separate original-Ferraris stability filter theorem. Its checked example has
four auxiliary leaves but only two semantic outputs. It does not prove the Rust
CNF-to-tree correspondence, watches/trail/backtracking, machine resource
accounting or oracle refinement.


The subsequent [candidate-pruning extension](candidate-pruning.md) adds
transactional candidate restrictions, bounded root probing and exact indexed
projection filters. The queens measurement above is historical evidence for
the earlier retained-cursor change, not a timing claim about the later version.
