# 2QBF by saturation over a Tseitin contradiction

Decide `∃X ∀Y ¬C_X(Y)`, where `C_X` is a formula in conjunctive normal form that
the existential assignment selects, with the classical saturation encoding in
disjunctive rules. Each answer set is one assignment of the existential
variables `x(1..exists)` under which every assignment of the universal variables
falsifies `C_X`. Like the [parity workload](../two-qbf-parity/README.md), it
makes one candidate's minimality check the whole solve, over a different hard
unsatisfiable core.

## The encoding

The core is Tseitin's parity formula on a connected 3-regular graph: one
universal variable per edge, and at each vertex the exclusive-or of its three
edges equals the vertex's charge. Vertex 1 has charge 1 and every other vertex
charge 0, so the charges' sum is odd and no assignment of the edges satisfies
the formula. Each vertex forbids its four patterns whose parity differs from its
charge, one clause each.

Each existential assignment `A = 0..2^exists - 1` selects a core: the first
`stable` assignments get the whole contradiction, and every other assignment
gets it less the clause of rank `A - stable + 1`, a clause of vertex 1 or 2. That
makes it satisfiable, since removing either vertex leaves each committed graph
connected. The encoding is otherwise the parity workload's: saturation keeps
only saturated candidates, and the answer sets are exactly the first `stable`
existential assignments, each decided by the complete subset query.

## The instance

The graphs are fixed random 3-regular graphs on 12, 18, 24 and 30 vertices,
committed in the source as `edge(N, E, U, V)` facts; `vertices` selects one. The
default, `exists=2, stable=1, vertices=12`, has one answer set, `nx(1) nx(2)`.
[manifest.json](manifest.json) seals the source and this complete default
contract. `stable=0` is unsatisfiable.

The subset queries walk about 700 regions on 12 vertices, 4,800 on 18 and
56,000 on 24, with grounding in milliseconds. With `exists=2` and `stable=1`
there are four candidates and one heavy query.

From the repository root:

```sh
zetesis solve examples/scalability/two-qbf-tseitin/two_qbf_tseitin.lp --all
zetesis-bench run --workloads examples/scalability/two-qbf-tseitin/manifest.json \
  --compare-threads 1,14 --repetitions 3 --memory-runs 0 \
  --timeout-seconds 120 --campaign-seconds 1800 --report two-qbf-tseitin.json
```

This workload is selected separately; it does not change the default scalability
or correctness populations. The `authored_examples::` tests check source
integrity, the complete default contract, checked constant edits, and every
listed case's complete family against a direct evaluation of each existential
assignment's core; the oracle gate checks the same cases with clingo, and the
CLI's authored-example tests solve the default natively.
