# Strategic companies

A holding has companies that produce products and companies jointly controlled by
others. A set of companies is strategic when its members produce every product, it
is closed under control (a company whose controllers are all in the set is in it
too), and no proper subset has both properties. This authored workload enumerates
every strategic set with the standard disjunctive encoding:

```text
strat(C1) | strat(C2) | strat(C3) | strat(C4) :- produced_by(P, C1, C2, C3, C4).
strat(C) :- controlled_by(C, C1, C2, C3, C4), strat(C1), strat(C2), strat(C3), strat(C4).
```

Each answer set is one strategic set. Minimality is the disjunction's: a
candidate is an answer set only when no proper subset is a model of its frozen
reduct, so every candidate runs the proper-subset query. Deciding whether a
company belongs to some strategic set is Σ2P-complete.

## The instance

The constants `companies`, `products` and `seed` determine the holding. Every
product has four producers, and one company in three is jointly controlled by four
companies other than itself; repeated companies are allowed, as in the standard
encoding. A fixed hash of the seed, the kind of draw, the index and the slot
chooses every company. It squares modulo the prime 46337, so every intermediate
value stays below 2^31 and every solver evaluates it the same way.

The default, `companies=12, products=24, seed=1`, has 29 strategic sets.
[manifest.json](manifest.json) seals the source and lists all of them. The family
grows with the holding: `companies=16, products=32` has 269 strategic sets and
`companies=20, products=40` has 697.

From the repository root:

```sh
zetesis solve examples/scalability/strategic-companies/strategic_companies.lp --all
zetesis-bench run --workloads examples/scalability/strategic-companies/manifest.json \
  --compare-grounders --compare-threads 1,14 \
  --repetitions 3 --memory-runs 0 \
  --timeout-seconds 120 --campaign-seconds 1800 --report strategic-companies.json
```

The matrix requests complete enumeration and qualifies every native answer
family against a fresh clingo run and the recorded contract.

This workload is selected separately; it does not change the default scalability
or correctness populations. The `authored_examples::` tests check source
integrity, the complete default contract, checked constant edits, and every
listed case's complete family against a direct enumeration of the minimal
covering, control-closed sets. The maintained oracle gate checks the same cases
with clingo, and the CLI's authored-example tests solve the default natively.
Other sizes use the existing `Workload::authored` and `ConstantAmendment` APIs,
which record the expected old values and require fresh reference qualification
after an edit.
