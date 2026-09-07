# kr-domains source fixtures

Upstream: [GregoryGelfond/kr-domains](https://github.com/GregoryGelfond/kr-domains).
Pinned revision: `38f0660ded448ed268c5a68759ceb0e2840dd497`.
License: MIT, **Copyright (c) 2026 Gregory Gelfond**. The complete upstream notice
is retained in [LICENSE.kr-domains](LICENSE.kr-domains).

The `accepted/` fragment fixtures preserve selected upstream rules and add explicitly marked
synthetic scalar facts. They are fragment conformance tests, not executions of
the original complete shortest-path or task-allocation optimization domains.
No source-language extension is implied.

Full support for the 94 original non-clingcon cases is the required future
target. None of these extracted cases counts as a full original case passing;
the compatibility target and exact manifest are in
`docs/verification/kr-domains-compatibility.md` and
`docs/verification/kr-domains-target-manifest.json` at the repository root.

## Accepted extracted cases

| File under `accepted/` | Verbatim upstream source | Synthetic instance | Expected complete result |
|---|---|---|---|
| `shortest-path-reachable.lp` | `encodings/shortest-path/variant-01.lp`, lines 35–52 | Vertices a/b/c; start a, end c; fixed included edges a→b→c | One model with reachable(a), reachable(b), reachable(c); 16 seeds |
| `shortest-path-disconnected-cycle-unsat.lp` | Same lines 35–52 | Start/end a; fixed included cycle b→c→b disconnected from a | UNSAT after 16 seeds |
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

The shortest-path rules consult only `reachable/1` gates. Synthetic values are
`1`, `a`, `b`, and `c`, so the conservative gate carrier has four atoms and all
16 seeds are tested. `reachable(1)` is an extra carrier tuple that has no
support and must remain false in an accepted seed. The allocation excerpt has
no candidate gates, so its complete search contains only the empty seed.

Extraction is reproducible from the pinned source: retain lines 35–52 inclusive
for shortest path, or lines 30 and 37 for allocation, preserving their text.
Only header comments and the separate synthetic fact block are new. SHA-256 of
the shortest-path excerpt, including its final LF, is
`f77feafba5fccda0450d76cc96390e46c1bcaf7372aa5022cb8dbae53e62f8ce`.
The allocation lines with one blank line between them hash to
`4bad5c8d9cd83776e06a060eb39ddb7544fad28d448dd13fdd7a775bd30af45c`.

## Historical S0 refusal cases

`refused/shortest-path-variant-01.lp` is a byte-for-byte copy of the entire
`encodings/shortest-path/variant-01.lp`. It includes `#defined`, a conditional
choice, aggregates, optimization, and `#show`; S0 must refuse it. Its current
first diagnostic is an unsupported non-rule statement.

The directory name records the original S0 boundary. The current automatic
frontend admits this encoding through the Ferraris route; the CLI regression
checks its complete empty-instance result. Complete encoding-plus-instance
results are covered separately by the snapshots below.

`refused/n-queens-variant-01.lp` is a byte-for-byte copy of the entire
`standalone/n-queens/variant-01.lp`. It uses intervals, bounded conditional
choices, ordering, arithmetic, and `#show`; S0 must refuse it. Its current first
diagnostic is an unsupported non-scalar term/operator.

All **155 unchanged `.lp` files** in the pinned snapshot were checked for S0
admission: **none was admitted**. The 20 open encodings, 125 scenario files, and
10 standalone cases therefore supply no unchanged complete solve benchmark for
this profile. See `docs/verification/kr-domains.md` and the recorded inventory.

## Complete original-source snapshots

`complete-models.json` is separate from the historical S0 excerpts above. It
records external clingo results for 68 unchanged original entry graphs selected by the
aggregate-assignment checkpoint, including every optimal displayed model set,
raw full-model count and cost vector. The CLI regression runs each original
vendored entry graph with its unchanged includes, requests every model and
compares complete results and exhausted coverage without requiring clingo.
The corpus validator separately checks the full source manifest and contracts.

The snapshot covers equality-generalized TSP, all shortest-path variants,
task-allocation variants 01/03, traveling-salesman variants 01/02/03, queens
variants 01/03 and SEND + MORE = MONEY.
The fixture records the original reference report and entry-source hashes.
It does not count excerpts, refusals or resource-limited runs as full passes.
