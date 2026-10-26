# 2QBF by saturation over a parity contradiction

Decide `∃X ∀Y ¬C_X(Y)`, where `C_X` is a formula in conjunctive normal form that
the existential assignment selects, with the classical saturation encoding in
disjunctive rules. Each answer set is one assignment of the existential
variables `x(1..exists)` under which every assignment of the universal variables
falsifies `C_X`. Unlike the [random 2QBF workload](../two-qbf/README.md), whose
subset queries stay small, this workload makes one candidate's minimality check
the whole solve: the nested quantifier's search over a hard unsatisfiable core.

## The encoding

The core is a parity contradiction over `n` inputs. Two chains of
three-variable exclusive-or gates compute the inputs' parity, the second in an
order hashed from `seed`, and the two results are required to differ, so no
assignment of the inputs and the chains' auxiliary variables satisfies it. Each
gate forbids its four patterns of odd parity, one clause each, and two clauses
forbid the results' agreement: `8(n - 1) + 2` clauses over `3n - 2` universal
variables.

Each existential assignment `A = 0..2^exists - 1` selects a core: the first
`stable` assignments get the whole contradiction, and every other assignment
gets it less the clause of rank `A - stable + 1`, which makes it satisfiable. The
universal variables are guessed by `y(J) | ny(J)`, a clause is falsified when
each of its literals is, and `valid` holds when the current core has a falsified
clause. Saturation then makes every universal atom true once `valid` holds, and
`:- not valid` keeps only saturated candidates. A saturated candidate is a
minimal model of its reduct exactly when no universal assignment satisfies its
core, so the answer sets are exactly the first `stable` existential assignments.
Every candidate holds both `y(J)` and `ny(J)`, so the frozen-positive
certificate declines it and the complete subset query decides it.

## The instance

The default, `exists=2, stable=1, n=6, seed=1`, has one answer set,
`nx(1) nx(2)`. [manifest.json](manifest.json) seals the source and this complete
default contract. `stable=0` is unsatisfiable, and `stable=2` has two answer
sets.

The stable candidate's subset query refutes the parity contradiction by
narrowing, without learning, so its work doubles with each input: about 2,000
regions at `n=10`, 33,000 at `n=14` and 131,000 at `n=16`, with grounding in
milliseconds. With `exists=2` and `stable=1` there are four candidates and one
heavy query, fewer candidates than a parallel walk has workers; that is the
shape on which dividing one candidate's subset query among workers pays.

From the repository root:

```sh
zetesis solve examples/scalability/two-qbf-parity/two_qbf_parity.lp --all
zetesis-bench run --workloads examples/scalability/two-qbf-parity/manifest.json \
  --compare-threads 1,14 --repetitions 3 --memory-runs 0 \
  --timeout-seconds 120 --campaign-seconds 1800 --report two-qbf-parity.json
```

The matrix requests complete enumeration and qualifies every native answer
family against a fresh clingo run and the recorded contract.

This workload is selected separately; it does not change the default scalability
or correctness populations. The `authored_examples::` tests check source
integrity, the complete default contract, checked constant edits, and every
listed case's complete family against a direct evaluation of each existential
assignment's core. The maintained oracle gate checks the same cases with clingo,
and the CLI's authored-example tests solve the default natively. Other sizes use
the existing `Workload::authored` and `ConstantAmendment` APIs, which record the
expected old values and require fresh reference qualification after an edit.
