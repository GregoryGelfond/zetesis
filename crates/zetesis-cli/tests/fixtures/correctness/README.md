# Correctness fixtures

Test data that zetesis-cli derives from the
[correctness examples](../../../../../examples/correctness/README.md).

The `excerpts/` fragment fixtures copy selected rules from the examples'
encodings and add explicitly marked synthetic scalar facts. They are fragment
conformance tests, not executions of the complete shortest-path or
task-allocation optimization examples, and never count as complete cases.
No source-language extension is implied.

The copied rules are unchanged from
[GregoryGelfond/kr-domains](https://github.com/GregoryGelfond/kr-domains) at
revision `38f0660ded448ed268c5a68759ceb0e2840dd497`. Their MIT license,
**Copyright (c) 2026 Gregory Gelfond**, is retained in [LICENSE](LICENSE).

## Rule excerpts

| File under `excerpts/` | Verbatim source | Synthetic instance | Expected complete result |
|---|---|---|---|
| `shortest-path-reachable.lp` | `encodings/shortest-path/variant-01.lp`, lines 35–52 | Vertices a/b/c; start a, end c; fixed included edges a→b→c | One model with reachable(a), reachable(b), reachable(c); one seed |
| `shortest-path-disconnected-cycle-unsat.lp` | Same lines 35–52 | Start/end a; fixed included cycle b→c→b disconnected from a | UNSAT before any seed |
| `task-allocation-projections.lp` | `encodings/task-allocation/variant-01.lp`, lines 30 and 37 | Two cost entries and one already selected assignment | One model deriving two compatibility atoms and the selected assignment cost; one seed |

The shortest-path excerpt has six unchanged rules: two structural constraints,
the base and recursive reachability rules, the end-reachability constraint, and
the constraint against included edges leaving unreachable vertices. No choice,
cardinality, aggregate objective, or output directive is copied. The edges are
input facts here: these tests check closure and consistency of a supplied path,
not discovery of an optimal path. The disconnected-cycle case specifically
checks that guessing reachability cannot manufacture its positive support.

The task-allocation excerpt derives `assigned_cost/3` and `compatible_with/2`.
Its `assigned_to/2` atom is an input fact. It checks joins and anonymous positive
variables; it does not solve optimal assignment or enforce assignment cardinality.

The shortest-path rules consult only `reachable/1` gates, and gate-free rules
derive reachability: every reachable vertex is a held gate atom and no other is
derivable. One seed therefore decides the reachable excerpt, where the
conservative carrier over the synthetic values `1`, `a`, `b` and `c` offered
sixteen. The disconnected excerpt is refuted before any seed, since a
constraint on an unreachable vertex fires under every seed. The allocation
excerpt has no candidate gates, so its complete search contains only the empty
seed.

Extraction is reproducible from the examples: retain lines 35–52 inclusive
for shortest path, or lines 30 and 37 for allocation, preserving their text.
Only header comments and the separate synthetic fact block are new. SHA-256 of
the shortest-path excerpt, including its final LF, is
`f77feafba5fccda0450d76cc96390e46c1bcaf7372aa5022cb8dbae53e62f8ce`.
The allocation lines with one blank line between them hash to
`4bad5c8d9cd83776e06a060eb39ddb7544fad28d448dd13fdd7a775bd30af45c`.

## Complete answers

`complete-models.json` records clingo 5.8.2's complete answers for all 94
correctness cases, run with `--opt-mode=optN --models=0`: every optimal shown
model set, the full-model count and the cost vector, with each entry file's
SHA-256. The CLI regression runs each case with its includes, requests every
model and compares complete results and exhausted coverage without requiring
clingo; the batch-publication tests compare the same answers through the
batched and region routes. The examples' manifest records the witnesses and
costs the corpus validator checks; these answers are complete. The fixture
does not count excerpts, refusals or resource-limited runs as full passes.
