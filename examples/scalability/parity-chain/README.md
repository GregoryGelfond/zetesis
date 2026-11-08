# Parity chains

Two chains compute the exclusive-or of the same inputs `x(1..p)`, in two
different orders, and the program requires the exclusive-or of their two results
to be `parity`. Both results are the exclusive-or of every input, so with
`parity = 1` the requirement is impossible and the program is unsatisfiable.
With `parity = 0` every assignment of the inputs extends to exactly one answer
set, `2^p` in all.

## The encoding

Each chain adds one input at a time through a gate `x(C) = x(A) xor x(B)` over
an auxiliary atom for the running result, and each gate states its parity as one
integrity constraint per forbidden pattern of its three atoms: four constraints
forbidding an odd number of them true. The final comparison is two more
constraints. These are the formula's clauses, written directly over the atoms
`x/1`. When the second chain reads the inputs in an unrelated order, resolution,
and so a solver that learns clauses, needs exponentially many steps to refute
them; read as linear equations over GF(2), they are refuted by elimination in
polynomial time.

## The instance

The first chain reads the inputs in order. The second reads input
`pi(K - 1) + 1` at its `K`-th step, where `pi(I) = (I + seed)^3 + 7 mod p`.
Cubing permutes the residues modulo a prime whose remainder modulo 3 is not 1,
so for such a prime (2, 3, 5, 11, 17, 23, 29, 41, 47, 53, ...) the second chain
reads every input once. Other values of `p` give a different formula. Every
intermediate value stays below `p^2`. The atoms `x(p + 1..2p - 1)` are the first
chain's running results and `x(2p..3p - 2)` the second's; an answer set shows
the inputs it holds as `on/1`.

The default, `p=5, seed=1, parity=1`, has 13 atoms and 34 constraints and is
unsatisfiable. [manifest.json](manifest.json) seals the source and this complete
default contract. With `parity=0` it has 32 answer sets.

From the repository root:

```sh
zetesis solve examples/scalability/parity-chain/parity_chain.lp --all
zetesis-bench run --workloads examples/scalability/parity-chain/manifest.json \
  --compare-grounders --compare-threads 1,14 \
  --repetitions 3 --memory-runs 0 \
  --timeout-seconds 120 --campaign-seconds 1800 --report parity-chain.json
```

## Scaling

Every instance with `parity=1` is unsatisfiable by construction. Only the
default's contract is committed. Larger instances are selected through the
matrix runner's checked constant edits; their outcome follows from the
construction, and clingo can confirm it only while its search stays within
reach:

| `p` | Atoms | Constraints | clingo 5.8.2, one thread |
| ---: | ---: | ---: | ---: |
| 5 | 13 | 34 | under 0.05 s |
| 17 | 49 | 130 | under 0.05 s |
| 29 | 85 | 226 | about 0.1 s |
| 41 | 121 | 322 | 5 to 6 s |
| 47 | 139 | 370 | about 58 s |
| 53 | 157 | 418 | over 120 s |

The times are indicative, from one or two runs on one machine shared with other
work.

This workload is selected separately; it does not change the default scalability
or correctness populations. The `authored_examples::` tests check source
integrity, the complete default contract, checked constant edits, and the
complete family of every listed case against a direct evaluation of the
constraints over every assignment of the atoms. The maintained oracle gate
checks the same cases with clingo, and the CLI's authored-example tests solve
the default natively.
