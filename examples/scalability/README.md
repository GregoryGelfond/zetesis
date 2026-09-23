# scalability examples

Parametric ASP programs for measuring solver throughput and scaling, authored for
this corpus. Where the [correctness](../kr-domains) group covers semantic breadth
on small instances, these carry one scaling knob each so the same encoding spans a
wide workload — from a quick default up to instances large enough to profile.

They use only portable ASP-Core-2 constructs, so clingo and zetesis run each one
unchanged; every recorded contract is confirmed to agree with clingo 5.8.2.

## Programs

| Program | Shape | Scaling knob | Default contract |
| --- | --- | --- | --- |
| `n-queens.lp` | enumeration | `#const n` (board size) | `n=8`: 92 answer sets |
| `pigeonhole.lp` | unsatisfiability | `#const h` (holes; `h+1` pigeons) | `h=7`: UNSAT |

`n-queens.lp` grows steeply (n=12 → 14200 answer sets, n=13 → 73712), exercising
both region enumeration and grounding as `n` rises. `pigeonhole.lp` is
unsatisfiable by construction and measures the work of *proving* unsatisfiability.

## Run an example

From the repository root, using the installed solver:

```sh
zetesis solve examples/scalability/n-queens.lp --all
zetesis solve examples/scalability/pigeonhole.lp --stats
```

The recorded contracts apply to the default `#const` values shown above. To select
a different size, change the `#const` declaration in the file (the solver takes no
constant-override flag); a benchmarking sweep generates one file per size from the
parametric source.

## Contracts

[manifest.json](manifest.json) records, for each program, its scaling knob, the
default `#const` value, the contract at that default (satisfiability and, for
enumeration, the answer-set count), and the source SHA-256. The contracts are the
values clingo 5.8.2 and zetesis both return.
