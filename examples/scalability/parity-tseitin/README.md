# Tseitin parity

Tseitin's parity formula on a connected 3-regular graph, the classic hard case
for resolution. Each edge is a free choice, and at each vertex the exclusive-or
of its three edges must equal the vertex's charge. Vertex 0 has charge `odd` and
every other vertex charge 0. With `odd = 1` the charges' sum is odd, and no
choice of edges satisfies every vertex: summing the vertex equations counts
every edge twice, so the sum of the charges would have to be even. With
`odd = 0` the solutions are the graph's even subgraphs.

## The encoding

Every vertex states its parity as one integrity constraint per forbidden pattern
of its three edges: four constraints, each forbidding one assignment whose
exclusive-or differs from the charge. These are the formula's clauses, written
directly over the edge atoms `on/1`. Resolution, and so a solver that learns
clauses, needs a number of steps exponential in the graph's size to refute them
when the graph expands; read as linear equations over GF(2), they are refuted by
elimination in polynomial time.

## The instance

The graph has `2p` vertices: a cycle through them all, which makes it connected,
and a perfect matching that joins vertex `I < p` to vertex `p + pi(I)`, where
`pi(I) = (I + seed)^3 + 7 mod p`. Cubing permutes the residues modulo a prime
whose remainder modulo 3 is not 1, so for such a prime (2, 3, 5, 11, 17, 23, 29,
41, 47, 53, ...) the matching is perfect and every vertex has three edges. Other
values of `p` give a different formula. Every intermediate value stays below
`p^2`.

The default, `p=5, seed=1, odd=1`, has 10 vertices, 15 edge atoms and 40
constraints, and is unsatisfiable. [manifest.json](manifest.json) seals the
source and this complete default contract. With `odd=0` the same graph has
`2^(p + 1)` answer sets, 64 at the default.

From the repository root:

```sh
zetesis solve examples/scalability/parity-tseitin/parity_tseitin.lp --all
zetesis-bench run --workloads examples/scalability/parity-tseitin/manifest.json \
  --compare-grounders --compare-threads 1,14 \
  --repetitions 3 --memory-runs 0 \
  --timeout-seconds 120 --campaign-seconds 1800 --report parity-tseitin.json
```

## Scaling

Every odd-charged instance is unsatisfiable by the parity argument above. Only
the default's contract is committed. Larger instances are selected through the
matrix runner's checked constant edits; their outcome follows from the
construction, and clingo can confirm it only while its search stays within
reach:

| `p` | Vertices | Edge atoms | Constraints | clingo 5.8.2, one thread |
| ---: | ---: | ---: | ---: | ---: |
| 5 | 10 | 15 | 40 | under 0.01 s |
| 17 | 34 | 51 | 136 | about 0.01 s |
| 29 | 58 | 87 | 232 | about 0.2 s |
| 41 | 82 | 123 | 328 | 2 to 12 s |
| 47 | 94 | 141 | 376 | about 31 s |
| 53 | 106 | 159 | 424 | over 100 s |

The times are indicative, from one or two runs on one machine shared with other
work.

This workload is selected separately; it does not change the default scalability
or correctness populations. The `authored_examples::` tests check source
integrity, the complete default contract, checked constant edits, and the
complete family of every listed case against a direct evaluation of the
constraints over every assignment of the edges. The maintained oracle gate
checks the same cases with clingo, and the CLI's authored-example tests solve
the default natively.
