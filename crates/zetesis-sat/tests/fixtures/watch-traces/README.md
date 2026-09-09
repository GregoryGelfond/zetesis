# Candidate traversal reference records

These records were produced from the unpacked watch implementation in commit
`78a069b562c9dd141a416b134f7d4aeb0041592a`, crates tree
`b4735f601118958e7c76f941ae151e35eed813e2`, with themelios pinned at `87c11a3`.
They were frozen before the compact implementation was used to check them.
The source identities distinguish the reference implementation from the
implementation under test. These are deterministic traversal fixtures, not timing
or answer-set acceptance evidence.

The six queens inputs are the clean, unchanged N=8 examples included directly
by `tests/support/watch_traces.rs`. A private test exercises the real formula
encoder and retained candidate cursor. Each record contains formula and CNF
dimensions, ordered semantic candidate identities, all four cumulative search
counters after candidate construction, and the final exhaustion or stop. Exact
semantic blocks are appended after each candidate. No clingo call, certificate
check, objective work, timing or stable-model claim belongs to this trace.

`refined.txt` uses `1 { p(1..4) } 2.` through the refined cursor, including root
probing. Its complete traversal consumes 2,294 work units and nine decisions.
`exact.txt` uses precisely those ceilings; `work-short.txt` and
`decision-short.txt` lower the corresponding ceiling by one. A stopped trace
retains its candidate prefix and complete charged accounting.

The records intentionally capture implementation order and work, in addition to
semantic identities. A later justified search or encoding change may require
new expectations; updating them requires independent evidence and review, not
regenerating them from a failing implementation. The test helper contains no
fixture-update mode. A separate ignored diagnostic prints bounded watch-storage
dimensions without measuring time or RSS.
