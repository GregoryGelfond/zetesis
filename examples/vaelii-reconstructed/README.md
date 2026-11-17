# Reconstructed Vaelii-shaped workloads

These fourteen programs are **reconstructions**: Datalog-shaped workloads
generated to match the row shapes of the Vaelii comparison (arithmetic joins,
transitive closure, seeded reachability, negation, aggregation, same
generation, stratified recursion and an enumeration control). They are not
Vaelii artifacts, and their timings are not Vaelii's results.

`gen.py` regenerates every file byte for byte from seed 20261117 (it also writes
search and optimization variants that are not part of the corpus). Each graph
is a random DAG with 1,000 or 10,000 edges.

| File | Shape |
|---|---|
| `arith1k.lp`, `arith10k.lp` | edge pairs joined through an arithmetic comparison |
| `closure1k.lp`, `closure10k.lp` | the full transitive closure |
| `firstask1k.lp`, `firstask10k.lp` | ancestors of one seeded node |
| `naf1k.lp`, `naf10k.lp` | the complement of that set by default negation |
| `agg1k.lp`, `agg10k.lp` | a per-node `#count` of transitive ancestors |
| `sg1k.lp` | same generation (OpenRuleBench shape) |
| `stratrec1k.lp`, `stratrec10k.lp` | recursion through a negated lower stratum, then a second recursive stratum |
| `queens12.lp` | complete enumeration of the 14,200 twelve-queens solutions |

The maintained benchmark corpus includes all but `sg1k` after the 94 catalog
cases and Family Crossing; `sg1k`'s single answer exceeds the corpus capture
limit, so it runs in the [`vaelii-slow`](../vaelii-slow/manifest.json)
supplement. The adjacent manifest records each file's bytes and the
expected complete family; each campaign requires a fresh clingo comparison.

```sh
zetesis-bench run --case vaelii-reconstructed/stratrec10k.lp --clingo /absolute/path/to/clingo
```
