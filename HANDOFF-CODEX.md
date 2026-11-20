# Zetesis 0.9.0 performance work: handoff to Codex

Date: 2026-11-20 (America/Chicago). Branch: `grok/integration-0.9.0`, PR against
`codex/post-0.8.0-performance` (base 798cf46a8).

## 1. What this branch contains

`grok/integration-0.9.0` merges every open 0.9.0 performance PR, #20 through #32,
with one `--no-ff` merge commit per PR and no rewritten history. Merging this one PR
lands all of them. The individual PRs can then be closed as merged-via-integration.

| PR | branch | what it does |
|---|---|---|
| #20 | grok/vaelii-datalog-0.9.0 | Datalog-shaped grounding: join-order deferral and direct definite recursion |
| #21 | grok/vaelii-classification-0.9.0 | Evaluate stratified negation and certain assignment aggregates directly |
| #22 | grok/vaelii-memory-0.9.0 | Bound retained search knowledge; remove grounding worker contention; faster objective scoring |
| #23 | grok/vaelii-exact-0.9.0 | Add Vaelii's exact ASP test programs to the benchmark corpus |
| #24 | grok/vaelii-hcf-0.9.0 | Shift head-cycle-free disjunctions into tight support checks |
| #25 | grok/vaelii-interner-0.9.0 | Ground through one content-addressed interner |
| #26 | grok/lazy-partition-0.9.0 | Bring lazy grounding close to eager speed |
| #27 | grok/grounder-redesign-0.9.0 | Grounder redesign (M1–M2) and coarse deadline observation |
| #28 | grok/algebra-names-0.9.0 | Rename algebra-name clashes outright, with no compatibility aliases |
| #29 | grok/native-learning-0.9.0 | Learn theory-class cores from refuted carrier regions (off-by-default switches) |
| #30 | grok/grounding-joins-0.9.0 | Ground saturated components, equality filters and count aggregates with less work |
| #31 | grok/algebra-operators-0.9.0 | Columnar Bind, Filter, Project, Union and Least |
| #32 | grok/columnar-join-0.9.0 | Indexed columnar set-at-a-time Join for instantiation and support |

The PRs had grown into a tree, not a chain: #22, #23 and #24 were siblings of #25
(#25 carries rebased duplicates of most of their commits), #26 and #27 branched off #25
separately, and #29, #30 and #31 branched off #28 separately. The merge notes in the PR
body record every conflict and how it was resolved. In short:

- #25 vs #22: #22's last commit (block merge sort on the grounding pool, 825ab5205) is
  ported in #25 as c54584ba3 onto the galloping merge sort; #25's side was kept.
- Proof record files (`proofs/verification.json`, `proofs/verification/current/*`) are
  generated; they were regenerated with `scripts/maintenance.sh proof-capture` on the
  merged tree (2357 theorems).
- #31 vs #30 in `formula_source_activity/acyclic.rs`: #30 moved saturated-component rules
  out of `rule_exact` into `rule_saturated` (read heads from completed support), so #31's
  `bind_exact_rows` keeps its Bind path without the now-unreachable `closed` branch.

Two fixes were made on #32's branch during integration and merged here; PR #32 at its
original tip (a27109888) is unsound without the first:

- e27d7ac0e: the columnar Join hook's semi-naive step took deltas only of the rule's
  head-predicate occurrences, and emitted nothing when those had no new rows. Under a
  positive-flat plan every predicate is treated as extensional, so rows a derived partner
  gained later never met the old rows of the recursive predicate and support stopped
  short (guerin/grandpa lost `grandpa(me)`, `parent(father,son)`, …). #32 alone hid one
  step behind the source-scan rejoin; #30's `rule_saturated` reads heads straight from
  support and exposed it fully. The hook now takes a delta for every occurrence whose
  relation grew. Regression test:
  `indexed_joins::recursive_rules_join_old_rows_with_a_grown_partner`.
- 3a9cc50ac: the hook's partitioned path on the grounding pool charged different work
  than the serial evaluation, so t1 and t14 grounding receipts disagreed (120 of 282
  pairs). The hook now runs serially at every thread count: t1 wall is unchanged and t14
  costs at most 1.07× on sg1k. Restoring a parallel hook with deterministic receipts is a
  follow-up (section 2).

#30 also changes the receipts for objective (optimization) programs: the `work` counter
of 44 corpus/Guerin rows differs from 9a63 with identical answers and optima; #30's own
tip (023f0f4df) gives the same values.

The PR body records the macOS and Linux gates and the answer checks (corpus plus Guerin
against #31's tip 9a63b57f6 and the recorded clingo runs, at t1 and t14) on this branch.

## 2. Open follow-up workstreams (not in this branch)

All are local branches on Gregory's Mac, based on #31's tip 9a63b57f6, so each needs to be
merged onto this integration branch (which also has #29, #30 and #32) before its PR.

| branch | status |
|---|---|
| grok/parallel-grounding-0.9.0 | 5 local commits (72086cbc2): 56-group A′ parallel path for worker-pure bodies, packed u16/u32 occurrence coordinates, residual replay in the Join walk, streaming support publication, adaptive prefix depth d ≤ 3. All answer checks passed (750/750 answers and order, 330/330 t1-vs-t14 receipt pairs, 361 corpus+Guerin rows unchanged). Still to do: emission and aggregate continuations, scaling measurements at 1/2/4/8/14 threads with peak memory, gates, PR. |
| grok/timeouts-0.9.0 | Started: enumerating every timeout across the corpus and Guerin set (no commits yet beyond 9a63b57f6). |
| grok/algebra-perf-0.9.0 | Core-algebra operator benchmarks against known designs (Soufflé, DuckDB, Velox, Arrow, CUB) with a bit-packing audit. Not started beyond the branch point. |
| grok/prim-refuter-reason-0.9.0 | 3 local commits (5e6f64023) plus uncommitted off/used/minimized conflict-core modes, all off by default. The 17-assumption cap is fixed but learning has not paid off: the two-phase probe costs 1.4–1.9× with nothing to learn (s_crossing-n5 10.8×). Not to be enabled unless it wins at t1 with no timeouts. |
| (new) parallel columnar Join | Not started. Bring back a partitioned delta Join for the hook in `formula_ground/algebra_join.rs` whose charged work is independent of thread count (t1 and t14 receipts must agree); the removed version (a27109888..e27d7ac0e) gained about 10% on sg1k at t14 and nothing on closure10k or agg10k. |
| grok/prim-propagation-0.9.0 | Uncommitted. Trail-based undo and region recycling were both measured and rejected (slower at t1, more memory). Now measuring packed `close_events` worklists (57% of planning time) against 9a63 and clingo. |

## 3. Gregory's standing rulings

- Wins at one thread first; parallelism should magnify them, not be their only source.
  Report t1 and t14 side by side.
- No timeouts. A timeout is a top-priority gap and a hard gate for turning anything on
  by default.
- Bit packing is the default layout everywhere; unpacked layouts need a justification.
- Beat or tie clingo on the Guerin problems and on the 330+ program corpus. A loss within
  10× is acceptable while clear wins outnumber such losses; losses of 10× or more are gaps.
- CI is local, on macOS and Linux: `scripts/check.sh portable`, `proofs`, `book` (macOS)
  and `oracle`. GitHub Actions is not the gate. Never weaken a gate or hide a regression.
- No SAT/CDNL migration, no encoding-specific shortcuts; keep the CPU path beside the GPU path.
- Size-specialized implementations of algebra operations are allowed, like insertion sort
  inside merge sort. That includes packed-bitset enumeration and a bounded CDNL-style leaf
  below a measured region-size cutoff. They are framed as algebra operations, not a separate
  engine, and each needs a Lean refinement proof before it can be on by default.
- Single-shot solving at the full horizon is the planning goal (no multi-shot needed).
- Measure each change on its own against a common base, then combined; verify answers,
  not just timing. PGO only with gains across the whole corpus. Distributed build flags
  and target-cpu are Gregory's call.
- Commit as Gregory Gelfond <gregory_gelfond@fastmail.com> with no trailers.

## 4. Known gaps

- Long-horizon planning: the planning horizons (vanilla_b-h7 and beyond, integrated-h5,
  s_crossing) are where zetesis is weakest; `close_events` dominates and conflict
  learning has not yet paid for itself.
- Three programs time out on both the 9a63 base and this branch:
  `family_crossing_vanilla_mode_b`, `five_crosser_light_mode_b_plan` and
  `guerin/wolf-goat-cabbage`.
- Guerin bin-packing: the default path is about 2.7 s against clingo's 0.015 s (≈180×).
- Measurement scripts and logs live outside the repo on Gregory's Mac in
  `~/Worktrees/zetesis/vaelii-artifacts/` (the longer running handoff log is its
  `HANDOFF-CODEX.md`).

## 5. Follow-ups from prior-art study

Ranked top 10 from the Z3 / MiniZinc (Chuffed, CP-SAT, Gecode) / cube-and-conquer study,
`~/Worktrees/zetesis/vaelii-artifacts/prior-art/Z3-MINIZINC-STUDY.md` on Gregory's Mac
(full reasoning and measurements there):

1. Explained operators and refuter reasons, LCG-style.
2. A cube-and-conquer cutoff metric for the leaf solvers, with a Lean proof before default-on.
3. Gecode-style events, advisors and priority scheduling for `close_events`.
4. Useful-width 1UIP composition.
5. Budgeted failed-literal / root probing.
6. Sparse region clones plus recomputation.
7. PDR/Spacer-style inductive rows for planning prefixes.
8. Two-watch application of short learned rows.
9. Lifting compact reasons to algebraic Filters (Stage D).
10. An explained bound/objective operator, with an optional LP relaxation; defer LNS and a portfolio.
