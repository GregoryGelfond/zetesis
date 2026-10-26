# 2QBF by saturation over the pigeonhole principle

Decide `∃X ∀Y ¬C_X(Y)`, where `C_X` is a formula in conjunctive normal form that
the existential assignment selects, with the classical saturation encoding in
disjunctive rules. Each answer set is one assignment of the existential
variables `x(1..exists)` under which every assignment of the universal variables
falsifies `C_X`. Unlike the [parity](../two-qbf-parity/README.md) and
[Tseitin](../two-qbf-tseitin/README.md) workloads, where only the stable
candidate's check is heavy, here the other candidates' checks carry real work
too.

## The encoding

The core places `holes + 1` pigeons into `holes` holes: one clause per pigeon
says it sits in some hole, and one clause per hole and pair of pigeons says they
do not share it, so no assignment satisfies it. The [pigeonhole
workload](../pigeonhole.lp) states the same principle as an unsatisfiable
program; here it is the universal side's core.

Each existential assignment `A = 0..2^exists - 1` selects a core: the first
`stable` assignments get the whole contradiction, and every other assignment
gets it less the clause of rank `A - stable + 1`, which makes it satisfiable,
since every clause of the pigeonhole formula is needed for its contradiction.
The encoding is otherwise the parity workload's: saturation keeps only saturated
candidates, and the answer sets are exactly the first `stable` existential
assignments, each decided by the complete subset query.

## The instance

The default, `exists=2, stable=1, holes=4`, has one answer set, `nx(1) nx(2)`.
[manifest.json](manifest.json) seals the source and this complete default
contract. `stable=0` is unsatisfiable.

The subset queries walk about 470 regions at 4 holes, 3,100 at 5 and 33,000 at
6, since even the satisfiable cores are hard to search. It is the case where
dividing candidates among workers already helps, beside which dividing one
candidate's subset query must not cost.

From the repository root:

```sh
zetesis solve examples/scalability/two-qbf-pigeonhole/two_qbf_pigeonhole.lp --all
zetesis-bench run --workloads examples/scalability/two-qbf-pigeonhole/manifest.json \
  --compare-threads 1,14 --repetitions 3 --memory-runs 0 \
  --timeout-seconds 120 --campaign-seconds 1800 --report two-qbf-pigeonhole.json
```

This workload is selected separately; it does not change the default scalability
or correctness populations. The `authored_examples::` tests check source
integrity, the complete default contract, checked constant edits, and every
listed case's complete family against a direct evaluation of each existential
assignment's core; the oracle gate checks the same cases with clingo, and the
CLI's authored-example tests solve the default natively.
