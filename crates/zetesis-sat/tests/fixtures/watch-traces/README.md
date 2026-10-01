# Candidate traversal reference records

These records were produced from the unpacked watch implementation in commit
`78a069b562c9dd141a416b134f7d4aeb0041592a`, crates tree
`b4735f601118958e7c76f941ae151e35eed813e2`, with themelios pinned at `87c11a3`.
They were frozen before the compact implementation was used to check them.
The source identities distinguish the reference implementation from the
implementation under test. These are deterministic traversal fixtures, not timing
or answer-set acceptance evidence.

The six queens inputs are the clean, unchanged N=8 examples included directly
by `src/search/tests/watch_traces.rs`. A private test exercises the real formula
encoder and retained candidate cursor. Each record contains formula and CNF
dimensions, ordered semantic candidate identities, all four cumulative search
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

The fixed input keeps exact search-work tests independent of equivalent
frontend DAG layouts. In particular, sharing a threshold table across choice
guards changes intermediate node order and the historical first-true-literal
rescan charges. Current source lowering is covered by the frontend aggregate
and choice tests; the six queens traces above still admit their source inputs.

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
