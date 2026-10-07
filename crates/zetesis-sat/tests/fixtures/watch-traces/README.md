# Candidate traversal reference records

These records were produced from the unpacked watch implementation in commit
`78a069b562c9dd141a416b134f7d4aeb0041592a`, crates tree
`b4735f601118958e7c76f941ae151e35eed813e2`, with themelios pinned at `87c11a3`.
They were frozen before the compact implementation was used to check them.
The source identities distinguish the reference implementation from the
implementation under test. These are deterministic traversal fixtures, not timing
or answer-set acceptance evidence.

The six queens inputs are fixed admitted theories in `queens-01.rs` through
`queens-06.rs`. They were captured from the unchanged N=8 examples under
`examples/correctness/standalone/n-queens/` at commit
`80df23ef6134959dcb5e42fcba265d2a4a525de1`, using `admit_formula` with default
admission, expansion and formula limits. Atom counts, node order, both child
indices and root order were copied without transformation. The old binary nodes
use the current pair constructors and an empty operand arena. `Theory::new`
validates each frozen input before the current encoder and retained candidate
cursor run. This input capture is separate from the original trace provenance
above; all six unchanged reference records must agree exactly. Each record
contains formula and CNF dimensions, ordered semantic candidate identities, all
four cumulative search
counters after candidate construction, and the final exhaustion or stop. Exact
semantic blocks are appended after each candidate. No clingo call, certificate
check, objective work, timing or stable-model claim belongs to this trace.

`refined.txt` uses the fixed admitted theory in `choices.rs` through the refined
cursor, including root probing. That input was captured by calling
`zetesis_themelios::admit_formula` with default admission, expansion and formula
limits on `1 { p(1..4) } 2.` at commit
`54f1ed4afbb373048300d1934d58190a5e0118c5`, then copying `Theory::atom_count`,
`Theory::nodes` and `Theory::roots` without reordering. This input capture is
separate from the original trace provenance above. The test validates those
four atoms, 45 nodes and nine roots through `Theory::new`, then runs the current
encoder and cursor against the unchanged reference records.

These fixed inputs keep exact search-work tests independent of equivalent
frontend DAG layouts. Sharing a threshold table across choice guards changes
intermediate node order and the historical first-true-literal rescan charges.
Transposing complete support guards and grouping source conjunctions can also
change the CNF and candidate traversal. Current source lowering remains covered
by the frontend tests and source oracle comparisons; these historical watch
regressions retain their original candidate identities, counters and outcomes.

The complete historical traversal consumes 2,294 work units and nine decisions.
A run with precisely those ceilings must reproduce it, so the same record is that
test's expectation; `work-short.txt` and `decision-short.txt` lower the
corresponding ceiling by one. A stopped trace
retains its candidate prefix and complete charged accounting.

The current replacement operation omits binary scans and inspects only the
unwatched occurrence of a ternary clause. The trace observer independently
replays the original position scan and restores its omitted work charges for
comparison with these unchanged records. Actual solver statistics retain only
performed work. Bounded reruns subtract those observed omissions from the
reference ceiling; full candidate identities and terminal outcomes still have
to match.

The records intentionally capture implementation order and work, in addition to
semantic identities. A later justified search or encoding change may require
new expectations; updating them requires independent evidence and review, not
regenerating them from a failing implementation. The test helper contains no
fixture-update mode.

Witness completion no longer rescans the base clauses: a complete assignment
that propagation left without conflict satisfies every clause, which a debug
build asserts. The observer restores that scan's historical charge for each
candidate, one unit per literal up to and including the first true one in every
base clause, so these records stay comparable; solver statistics count only
performed work.

Native formula encoding also charges every operand occurrence once while
building its classical gate view, including both implication operands. This is
one setup charge E before any search observation. The reference observer
subtracts exactly the admitted theory's E from each cumulative work record;
it leaves every other counter, candidate and terminal outcome intact. The
four-atom input has E = 80, so its actual inclusive work ceiling adds 80 before
subtracting the independently observed watch, witness and exclusion savings.
The historical fixture files and their original provenance remain unchanged.
