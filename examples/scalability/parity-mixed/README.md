# Parity mixed with clauses

A satisfiable mixture of structured and unstructured constraints. Tseitin's
parity formula with every vertex charged 0, on the same graphs as the
[Tseitin workload](../parity-tseitin/README.md), has the graph's even subgraphs
as its solutions. `clauses` random three-literal clauses over the edges then cut
them down. The parity constraints form a linear system over GF(2); the random
clauses do not, so a solver must still search among the even subgraphs for
those the clauses allow. The answer sets are those subgraphs.

## The encoding

Every vertex states its parity as one integrity constraint per pattern of its
three edges with an odd number on, and every random clause as the one
constraint forbidding the pattern that falsifies its three literals. All
constraints are written directly over the edge atoms `on/1`.

## The instance

The graph has `2p` vertices, a cycle through them all and a perfect matching
built by the Tseitin workload's permutation, `pi(I) = (I + seed)^3 + 7 mod p`,
which is a bijection when `p` is a prime whose remainder modulo 3 is not 1. A
fixed hash of `seed`, the clause and the literal chooses each clause's three
distinct edges and their signs. It squares modulo the prime 46337, so every
intermediate value stays below `2^31`.

The default, `p=5, clauses=8, seed=1`, has 15 edge atoms and 48 constraints and
21 answer sets. [manifest.json](manifest.json) seals the source and this
complete default contract. Without clauses the graph has `2^(p + 1)` answer
sets, 64 at the default.

From the repository root:

```sh
zetesis solve examples/scalability/parity-mixed/parity_mixed.lp --all
zetesis-bench run --workloads examples/scalability/parity-mixed/manifest.json \
  --compare-grounders --compare-threads 1,14 \
  --repetitions 3 --memory-runs 0 \
  --timeout-seconds 120 --campaign-seconds 1800 --report parity-mixed.json
```

## Scaling

Each random clause removes about an eighth of the remaining even subgraphs, so
about `2^(p + 1) (7/8)^clauses` answer sets remain. Only the default's contract
is committed. Larger instances are selected through the matrix runner's checked
constant edits, with a clause count that keeps a few answer sets:

| `p` | `clauses` | Edge atoms | Constraints | Answer sets |
| ---: | ---: | ---: | ---: | ---: |
| 5 | 8 | 15 | 48 | 21 |
| 11 | 40 | 33 | 128 | 19 |
| 17 | 70 | 51 | 206 | 86 |
| 29 | 130 | 87 | 362 | 24 |
| 41 | 190 | 123 | 518 | 12 |

clingo 5.8.2 enumerates each of these families in under 0.1 s on one thread.

This workload is selected separately; it does not change the default scalability
or correctness populations. The `authored_examples::` tests check source
integrity, the complete default contract, checked constant edits, and the
complete family of every listed case against a direct evaluation of the
constraints over every assignment of the edges. The maintained oracle gate
checks the same cases with clingo, and the CLI's authored-example tests solve
the default natively.
