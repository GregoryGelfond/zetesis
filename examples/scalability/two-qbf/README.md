# 2QBF by saturation

Decide `∃X ∀Y D(X, Y)`, where `D` is a formula in disjunctive normal form, with the
classical saturation encoding in disjunctive rules. Each answer set is one
assignment of the existential variables `x(1..exists)` under which `D` holds for
every assignment of the universal variables `y(1..forall)`; the program is
unsatisfiable when there is none. This authored workload is the corpus's first
disjunctive program: every membership check runs the proper-subset query over a
candidate's frozen reduct, the nested quantifier's search.

## The encoding

Each existential and universal variable is guessed by a disjunction, `x(I) | nx(I)`
and `y(J) | ny(J)`. A term holds when its literals do, and `valid` holds when some
term does. Saturation then makes every universal atom true once `valid` holds, and
the constraint `:- not valid` keeps only saturated candidates. A saturated candidate
is a minimal model of its reduct exactly when no universal assignment falsifies
`D` under its existential assignment, so the program's answer sets are the
existential assignments that make `∀Y D` true. Every candidate holds both
`y(J)` and `ny(J)` for every `J`, so the frozen-positive certificate declines it
and the complete subset query decides it.

## The instance

The constants `exists`, `forall`, `width`, `terms` and `seed` determine the
formula. Each of its `terms` terms has one existential literal and `width`
universal literals over distinct variables, so whether a term holds always
depends on the universal assignment. A fixed hash of the seed, the term and the
variable chooses every literal. It squares modulo the prime 46337, so every
intermediate value stays below 2^31 and every solver evaluates it the same way.
The ranks that select a term's universal variables append the variable's index
to its key, which keeps them distinct while `forall` stays below 32.

The default, `exists=3, forall=8, width=3, terms=68, seed=1`, has two answer sets,
`nx(1) nx(2) nx(3)` and `nx(3) x(1) x(2)`. [manifest.json](manifest.json) seals
the source and this complete default contract. With three literals per term,
instances near 8.5 terms per universal variable mix true and false outcomes:
for example, `terms=52, seed=1` is unsatisfiable, and `terms=68, seed=2` has four
answer sets. Larger `forall` and `width` lengthen each candidate's subset query.

From the repository root:

```sh
zetesis solve examples/scalability/two-qbf/two_qbf.lp --all
zetesis-bench run --workloads examples/scalability/two-qbf/manifest.json \
  --compare-grounders --compare-threads 1,14 \
  --repetitions 3 --memory-runs 0 \
  --timeout-seconds 120 --campaign-seconds 1800 --report two-qbf.json
```

The matrix requests complete enumeration and qualifies every native answer
family against a fresh clingo run and the recorded contract.

This workload is selected separately; it does not change the default scalability
or correctness populations. The `authored_examples::` tests check source
integrity, the complete default contract, checked constant edits, and every
listed case's complete family against a direct evaluation of the quantified
formula. The maintained oracle gate checks the same cases with clingo, and the
CLI's authored-example tests solve the default natively. Other sizes use the
existing `Workload::authored` and `ConstantAmendment` APIs, which record the
expected old values and require fresh reference qualification after an edit.
