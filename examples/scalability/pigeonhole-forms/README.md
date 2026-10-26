# Pigeonhole in two encodings

The pigeonhole principle: `holes + extra` pigeons, `holes` holes, every pigeon in
some hole and no hole holding two pigeons. With `extra = 1` there are more
pigeons than holes, and the program is unsatisfiable. With `extra = 0` every
pigeon occupies exactly one hole, and the answer sets are the `holes!`
one-to-one placements.

## The encodings

Each pigeon may occupy any of the holes, a free choice of the atom `in(P, H)`.
The constant `form` selects how the same two conditions are written:

- `form = 1`, clauses: one integrity constraint per pigeon that it occupies no
  hole, and one per pair of pigeons sharing a hole;
- `form = 2`, counts: a `#count` bound per pigeon, at least one hole, and per
  hole, at most one pigeon.

The scalability corpus's [`pigeonhole.lp`](../pigeonhole.lp) writes the first
condition as a choice bound instead. Resolution, and so a solver that learns
clauses, needs exponentially many steps to refute any of them. Counting the
incidences twice, once by pigeon and once by hole, refutes them at once: the
pigeons need at least `holes + 1` places, and the holes offer at most `holes`.

## The instance

The default, `holes=4, extra=1, form=1`, has 20 choice atoms and is
unsatisfiable. [manifest.json](manifest.json) seals the source and this complete
default contract. With `extra=0` the default sizes have 24 answer sets in either
form.

From the repository root:

```sh
zetesis solve examples/scalability/pigeonhole-forms/pigeonhole_forms.lp --all
zetesis-bench run --workloads examples/scalability/pigeonhole-forms/manifest.json \
  --compare-grounders --compare-threads 1,14 \
  --repetitions 3 --memory-runs 0 \
  --timeout-seconds 120 --campaign-seconds 1800 --report pigeonhole-forms.json
```

## Scaling

Every instance with `extra=1` is unsatisfiable. Only the default's contract is
committed. Larger instances are selected through the matrix runner's checked
constant edits; their outcome follows from the construction, and clingo can
confirm it only while its search stays within reach:

| `holes` | Choice atoms | clingo 5.8.2, clauses | clingo 5.8.2, counts |
| ---: | ---: | ---: | ---: |
| 4 | 20 | under 0.01 s | under 0.01 s |
| 8 | 72 | about 0.1 s | about 0.1 s |
| 9 | 90 | about 1 s | about 0.9 s |
| 10 | 110 | 16 to 18 s | 13 to 15 s |
| 11 | 132 | over 300 s | over 300 s |

The times are indicative, from one or two runs on one machine shared with other
work.

This workload is selected separately; it does not change the default scalability
or correctness populations. The `authored_examples::` tests check source
integrity, the complete default contract, checked constant edits, and the
complete family of every listed case against a direct evaluation of the
conditions over every assignment of the atoms. The maintained oracle gate checks
the same cases with clingo, and the CLI's authored-example tests solve the
default natively.
