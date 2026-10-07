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
| `mastermind.lp` | many-answer enumeration | `#const colors` (6 pegs) | `colors=6`: 6080 answer sets |
| `mastermind-nested.lp` | nested aggregate assignments | `#const colors` (6 pegs, seven guesses) | `colors=8`: one answer set |

`n-queens.lp` grows steeply (n=12 → 14200 answer sets, n=13 → 73712), exercising
both region enumeration and grounding as `n` rises. `pigeonhole.lp` is
unsatisfiable by construction and measures the work of *proving* unsatisfiability.
`mastermind.lp` lists every secret code of six pegs consistent with one scored
guess, counting colour matches with aggregates; its answers grow with the number
of colours (5 → 976, 6 → 6080, 7 → 19600, 8 → 45832), so it measures
per-answer checking and delivery. The maintained population includes
`colors=8`; its report-reading limits account for that complete answer family
without increasing the solver's resource limits. Larger instances can exceed
either solver or measurement limits; keep the resulting refusal with the record.

`mastermind-nested.lp` composes counts, minima and sums to score seven guesses.
Its unique secret is `8,5,8,5,3,4`. Repeated guess positions exercise whether
grounding shares an aggregate's unchanged inputs. The source is authored for
this corpus.

## Run an example

From the repository root, using the installed solver:

```sh
zetesis solve examples/scalability/n-queens.lp --all
zetesis solve examples/scalability/pigeonhole.lp --stats
zetesis solve examples/scalability/mastermind.lp --all
zetesis solve examples/einstein-riddle.lp --all
```

Einstein uses automatic grounding and ordinary resource defaults. The solver
can defer eligible terminal definitions until it has checked each base answer;
it then reconstructs the complete answer before displaying it. Explicit eager
grounding instead materializes the entire program and can require a larger
grounding allowance.

The recorded contracts apply to the default `#const` values shown above. To select
a different size for an experiment, the maintained matrix runner performs a
parser-located, expected-value-checked edit in a private input copy. It records
the unchanged source hash, edit and resulting source hash.

## Check and measure thread scaling

The maintained `zetesis test scalability` and `zetesis-bench run --suite
scalability` commands use one library-owned population of fourteen workloads:
queens at n=8/9/10, pigeonhole at h=5/6/7, Mastermind at colors=5/6/8,
nested Mastermind at colors=8, the established correctness queens variant
02, SEND+MORE=MONEY and task-allocation cases, and the authored
[Sudoku grid](../sudoku.lp). Sudoku has eight givens per row and exercises
grounding and language handling. Test qualifications request CPU eager/indexed
region search at 1, 2, 4, 8 and 14 threads with one completion worker by default.

From the repository root, with zetesis and clingo installed:

```sh
zetesis test scalability --threads 1,2,4,8,14 --report scalability-check.json
zetesis-bench run --suite scalability --grounder eager \
  --compare-threads 1,2,4,8,14 --repetitions 4 --memory-runs 2 \
  --timeout-seconds 30 --campaign-seconds 1800 --report scalability-timing.json
```

`--include-einstein` adds the unchanged Einstein riddle as a fifteenth workload.
An explicit `--max-expansion-work 300000000` can be supplied when studying that input; the
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
without clap. See the
[command guide](../../docs/book/reference/commands.md#check-conformance) for
source roots, compact JSON views and preserved refusal diagnostics.

## Contracts

[manifest.json](manifest.json) records, for each program, its scaling knob, the
default `#const` value, the contract at that default (satisfiability and, for
enumeration, the answer-set count), and the source SHA-256. The contracts are the
expected complete results. `cargo test --locked -p zetesis-validation --test
integration authored_examples::` checks source integrity and contract registration without
solving large instances. `scripts/check.sh oracle` also runs the complete clingo
contracts for all defaults, Sudoku and Einstein. Native default checks live
in `zetesis-cli/tests/integration/authored_examples.rs`, including Sudoku and
Einstein with automatic grounding and unchanged resource defaults. These
checks run in the portable test suite.
