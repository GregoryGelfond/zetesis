# scalability examples

Parametric ASP programs for measuring solver throughput and scaling, authored for
this corpus. Where the [correctness](../correctness) group covers semantic breadth
on small instances, these carry one scaling knob each so the same encoding spans a
wide workload — from a quick default up to instances large enough to profile.

The defaults have typed display contracts checked by the maintained fixture tests
and the opt-in clingo comparison. Measurements must retain completion and source
identity; increasing a parameter can instead reach a solver or campaign limit.

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
a different size for an experiment, the maintained matrix runner performs a
parser-located, expected-value-checked edit in a private input copy. It records
the unchanged source hash, edit and resulting source hash.

## Check and measure thread scaling

The maintained `test scalability` and `bench corpus --suite scalability`
commands use one library-owned population: queens at n=8/9/10, pigeonhole at
h=5/6/7, and the established correctness queens variant 02, SEND+MORE=MONEY and
task-allocation cases. Test qualifications request CPU eager/indexed region
search at 1, 2, 4, 8 and 14 threads with one completion worker by default.

From the repository root, with zetesis and clingo installed:

```sh
zetesis test scalability --threads 1,2,4,8,14 --report scalability-check.json
zetesis bench corpus --suite scalability --grounder eager \
  --compare-threads 1,2,4,8,14 --repetitions 4 --memory-runs 2 \
  --timeout-seconds 30 --campaign-seconds 1800 --report scalability-timing.json
```

`--include-einstein` adds the unchanged Einstein riddle. An explicit
`--max-expansion-work 300000000` can be supplied when studying that input; the
override is retained in every native profile. No limit is raised silently.

The test runs one complete-family qualification per producer and workload, with
no warmup, timed or RSS rounds. Clingo establishes each complete displayed family;
native full model identities must also agree across profiles. The benchmark
command above then requests one warmup, four timed samples and two separate
child-RSS samples per native profile. Native JSON and statistics, including
phase timings, are inside the timed intervals. Reference qualification and
memory samples stay separate. The example explicitly selects a 30-second
process limit and 1,800-second campaign limit; plain benchmark defaults remain
10 and 180 seconds. Use `--timeout-seconds` and `--campaign-seconds` to change
these bounds. Failed and unlaunched positions
remain in the new, no-clobber evidence report. Failed or skipped timed positions
prevent a timing distribution for that cell. A later memory failure can leave
completed timing samples intact but still makes the overall campaign fail.

The report's top-level correctness manifest identifies the catalog supplying the
three established cases. Each authored workload carries its own source seal and
typed contract; it makes no upstream-cleaning or generated-family proof claim.
The command adapters use `performance::scalability::run_with_cancellation`; Rust
consumers can select the same workloads and qualification or measurement plan
without clap or a separate experiment executable. See the
[command guide](../../docs/book/reference/commands.md#check-conformance) for
source roots, compact JSON views and preserved refusal diagnostics.

## Contracts

[manifest.json](manifest.json) records, for each program, its scaling knob, the
default `#const` value, the contract at that default (satisfiability and, for
enumeration, the answer-set count), and the source SHA-256. The contracts are the
expected complete results. `cargo test -p zetesis-validation --test
authored_examples` checks source integrity and contract registration without
solving large instances. `scripts/check.sh oracle` also runs the complete clingo
contracts for both defaults and Einstein. Native default checks live in
`zetesis-cli/tests/authored_examples.rs`; the larger Einstein native test is an
explicit release qualification, separate from the portable population.
