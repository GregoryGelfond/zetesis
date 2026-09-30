# Benchmarking with zetesis-bench

`zetesis-bench` runs the zetesis benchmark suite and shows the results as
tables or as JSON. It compares zetesis with the installed clingo when there is
one, times zetesis alone when there is not, and compares saved runs. It runs
the solvers as separate programs, as a user would, and counts a time only after
zetesis's answers are qualified: against clingo's when clingo takes part, and
against each workload's recorded answers when it does not.
[INSTALL.md](https://github.com/GregoryGelfond/zetesis/blob/main/INSTALL.md)
installs it beside `zetesis`.

```sh
zetesis-bench run
zetesis-bench compare before=cpu-before.json after=cpu-after.json
zetesis-bench --help
```

| Command | Purpose |
| --- | --- |
| `run` | Measure the benchmark suite: answer families, timings and memory, beside clingo when there is one and alone when there is not. |
| `compare` | Compare saved reports as tables, JSON or Markdown. |

## Run the suite

From a repository checkout, `zetesis-bench run` measures the installed
`zetesis` on the whole corpus, keeps the evidence in a new file whose name it
prints, and prints the tables. Benchmarks prescribe their instrumentation,
including statistics; there is no `--stats` toggle. Human tables use the shared
terminal styling and remain plain when redirected; `--json` prints the compact
summary instead, and `--color` sets the styling. `run` requires Linux or macOS
for bounded process capture and fresh-child RSS receipts. It refuses an
unsupported platform explicitly; `compare` launches no child and has no such
requirement.

```sh
zetesis-bench run
zetesis-bench run --suite baseline --backend metal --grounder eager \
  --report metal-run.json
zetesis-bench run --threads 2 --json --report two-thread-run.json > two-thread-summary.json
```

### What is measured

Omitting `--zetesis` measures the installed `zetesis`: the one beside
`zetesis-bench`, else the first on `PATH`. Every measured executable runs
through its explicit `solve` command; `--native-interface legacy` measures an
older binary through its historical flat arguments instead. The report retains
the actual argument sequences.

### What runs

The positional directory, the corpus, defaults to `examples/correctness`
relative to the current directory. Supply the directory from a repository
checkout when running elsewhere; the tool does not fetch or bundle these
inputs.

| `--suite` | Workloads |
| --- | --- |
| `corpus` (the default) | All 94 verified corpus cases |
| `baseline` | SEND, queens variant 02 and task allocation |
| `queens` | The six unchanged eight-queens encodings |
| `series` | The maintained 22 generated, constant-amended and original workloads |
| `scalability` | Nine authored and corpus workloads, with indexed formula joins and region search |

`--case PATH` measures one case of the suite, named relative to the corpus
directory; repeat it to measure several, in the order given. For `series`, use
the workload entry path (including `generated/...`); a repeated amended entry
such as `standalone/n-queens/variant-01.lp` selects both its ten- and eleven-queen
cells in their series order. A case outside the suite is refused before anything
launches. The scalability suite measures its
own workloads and accepts no `--case`; it alone accepts `--examples`, its
authored root, which defaults to `examples`, and `--include-einstein`, which
adds the unchanged Einstein riddle. The series suite raises the per-invocation,
capture, evidence and native-decoder ceilings to the sizes of its records.

### Profiles

A run measures one native profile, set by `--backend`, `--grounder` and
`--threads`, or compares several. Each compared axis replaces its single
setting, and the axes combine into their product, backends slowest and thread
counts fastest, at most eight profiles:

| Axis | Profiles | Replaces |
| --- | --- | --- |
| `--compare-backends cpu,gpu` | One per listed backend | `--backend` |
| `--compare-grounders` | Eager and lazy grounding | `--grounder` |
| `--compare-threads 1,2,4` | One per listed thread count, each at most 256 | `--threads` |

Every other setting is the same in each profile. Clingo times beside a single
profile in every phase; when profiles are compared it supplies one
qualification census per case, and only the native profiles have warmup,
timing and memory rounds.

| Setting | Default |
| --- | --- |
| `--backend cpu\|gpu\|metal\|vulkan` | `cpu`. A GPU backend is required: its absence is retained as a non-pass. |
| `--grounder auto\|eager\|lazy` | `auto` |
| `--threads auto\|N`, also `--workers` | At most four available host threads; one if unknown |

The advanced controls, listed by `zetesis-bench run --help` and omitted from
`-h`, apply to every profile:

| Advanced control | Default |
| --- | --- |
| `--oracle auto\|closure\|countermodel` | `auto` |
| `--search regions\|clauses` | The suite's: region search for `scalability`, the solver's own otherwise |
| `--formula-joins indexed\|table` | The suite's: indexed joins for `scalability`, the solver's own otherwise |
| `--completion-workers` | One |
| `--batch-size` | 64 candidate occurrences |
| `--max-expansion-work` | None; a ceiling is recorded in the evidence, independent of the process deadline |
| `--time-limit SECONDS` | None; a cooperative native deadline |
| `--clingo-threads`, also `--clingo-workers` | One; clingo retains its stock search heuristics |

The maintained telemetry decoder reads every backend and grounder; its Vulkan
decoding awaits qualification on a Vulkan host. It reports unsupported
combinations and actual execution failures rather than silently replacing the
requested route.

```sh
zetesis-bench run --suite queens --threads 1 --compare-grounders \
  --repetitions 4 --memory-runs 2 --report grounding-comparison.json
zetesis-bench run --suite scalability --grounder eager --compare-threads 1,2,4,8,14 \
  --repetitions 4 --memory-runs 2 --timeout-seconds 30 --campaign-seconds 1800 \
  --report thread-comparison.json
zetesis-bench run --suite baseline --compare-backends cpu,metal --report backends.json
```

All native populations must agree on complete full-model identities as well as
the qualified selected outputs and costs. Lazy formula execution records the
eager retained core and streamed constraint checks separately. Deferring
constraints can increase core enumeration work, so a refusal or a slower lazy
result is a meaningful result of the comparison. Four timed rounds balance the
rotating eager/lazy order; memory rounds remain separate.

For hybrid formula execution, the grounding interval covers initial support and
core admission. Streamed constraint checks occur during solving and are included
in `original_validation`. The initial grounding time alone therefore does not
measure all source evaluation work. Compare complete solve time, storage and
the separate constraint-work counters as well.

### clingo

`--clingo PATH` names the clingo to compare with, and it must be a runnable
file: anything else is an error, never a silent fallback. Without it, the first
`clingo` on `PATH` takes part. When there is none, the run measures zetesis
alone and says so on standard error; `--without-clingo` measures zetesis alone
on purpose, and cannot be combined with `--clingo`.

Without clingo, each native answer family is qualified against the workload's
recorded answers, and every later native sample against the first qualified
family. A workload with no recorded answers, such as an amended series cell,
whose family only clingo establishes, is recorded as needing clingo: it is not
launched, and it is a non-pass shown with that reason. Every corpus case has
recorded answers, so a plain run without clingo measures the whole default
suite. The evidence records the policy the run used: `all_phases`,
`qualification_only` or `clingo_free`. The table and the JSON summary say what
qualified each cell: clingo, the recorded contract, or nothing, because the
cell needs clingo.

### Evidence

`--report NEW.json` names the evidence file; by default it is
`zetesis-bench-<suite>-<UTC time>.json` in the current directory, for example
`zetesis-bench-corpus-20260928T193012Z.json`. The name is printed on standard
error before anything launches, and an existing file is never replaced. The
evidence holds the whole bounded campaign: the tool that produced it and its
version, executable and source identities, the plan and its reference policy,
raw captures, and failed and unlaunched positions.

A qualification round precedes warmups and timings. Native children use
`solve --all --json --stats`; clingo retains its exhaustive optimum-tie output.
Wall time includes process launch, grounding, solving and captured machine
output. RSS is a separate child-resource observation, not device memory or an
allocator counter. Primitive work counts are not elapsed time or machine
instructions.

A first non-pass disables later launches for that cell; a campaign stop retains
the remaining planned positions. Neither is replaced by another sample. The
human and JSON summaries compute wall-time distributions only when the cell's
entire timed population passed; failures are not assigned a synthetic timeout
duration. Validated memory rounds have their own median and may be unavailable.
Non-pass summaries include the recorded reason and schedule phase. If a failed
qualification prevented later samples, those slots identify the original
blocker. A refusal, timeout or process failure never counts as an UNSAT result
or a successful timing sample. A child stopped by the campaign's own deadline
or retained-capture budget, before its per-child timeout or sample limit, is
recorded as `campaign_deadline` or `campaign_capture_budget`, never as its own
`timeout` or `capture_limit`: it says nothing of the workload.

On Linux and macOS, SIGINT and SIGTERM stop a run cooperatively: it launches
nothing further, settles its children under the cleanup bounds and publishes
the incomplete evidence.

### Schedule and bounds

| Setting | Default |
| --- | --- |
| `--warmups` / `--repetitions` | One warmup / three timed rounds per solver and case |
| `--memory-runs` | One separate fresh-child RSS round per solver and case |
| `--timeout-seconds` / `--campaign-seconds` | 10 per child / 180 for campaign scheduling |
| `--sample-bytes` | 16 MiB of standard output and error per invocation |
| `--native-report-bytes` | 16 MiB of native JSON read by the decoder, separate from capture |
| `--capture-bytes` | 512 MiB of captures retained across the run |
| `--report-bytes` | 1 GiB of serialized evidence |

### A smaller fixed population

The maintained
[grounding comparison example](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-validation/examples/grounding_comparison.rs)
uses the same typed matrix API. Its default `--study storage` preserves nine
workloads: queens variants 01 and 03 at n=4/5,
variant 02 at n=4 as a control for lost early constraint pruning, and variant 06
at n=4 whose scoped constraints remain eager. Generated monotone choices at
n=6 and redundant transitivity at n=8/12 provide additional pruning and storage
controls. Workload constructors retain source identities and leave the corpus
unchanged. Neither those labels nor successful qualification predicts a speedup.

```sh
cargo run --release -p zetesis-validation --example grounding_comparison -- \
  --zetesis /absolute/path/to/zetesis --clingo /absolute/path/to/clingo \
  --helper /absolute/path/to/zetesis-bench --workers 1 \
  --report grounding-small.json > grounding-small-summary.json
```

The native path must name the current zetesis executable, and the helper path
the `zetesis-bench` executable, which answers the memory rounds' helper
protocol. The
example uses one warmup, four timed rounds and two separate RSS rounds per
native profile, a ten-second child timeout and a 180-second campaign deadline.
All 153 planned positions are retained, including nine qualification-only
clingo invocations. A non-pass exits unsuccessfully after publishing the evidence.

The fixed `refutation` study compares earlier constraint pruning with repeated
scan costs. Its twenty workloads are queens 01/03 at n=4/5/6, queens 02/04/05/06
at n=4/5, monotone choices at n=6/8/10 and redundant transitivity at n=8/12/16.
It retains the same profiles and rounds, with a 300-second campaign deadline
and 340 planned positions, including twenty qualification-only clingo invocations.

```sh
cargo run --release -p zetesis-validation --example grounding_comparison -- \
  --study refutation --zetesis /absolute/path/to/zetesis \
  --clingo /absolute/path/to/clingo --helper /absolute/path/to/zetesis-bench \
  --workers 4 --report grounding-refutation.json > grounding-refutation-summary.json
```

The larger cases can reach the fixed resource ceilings or timeout, particularly
when hybrid execution checks every core answer before rejecting it. Those
refusals remain non-passes; an incomplete family does not supply a speed ratio.
Both studies compare complete native families within each workload. Clingo
supplies the selected-output census only, with no reference timings or RSS runs.

## Compare saved runs

```sh
zetesis-bench compare before=cpu-before.json after=cpu-after.json
zetesis-bench compare before=cpu-before.json after=cpu-after.json \
  --json --output comparison.json
zetesis-bench compare main=series-main.json change=series-change.json --markdown
```

`compare` launches no solver. Its reports are operands, `LABEL=PATH`, one or
more, in the order compared. It checks matching ordered workloads, sealed
corpus identities and requested profiles before deriving the comparison.
Search method is the explicitly supported profile difference; other profile
settings, such as thread counts, must match. `--input-bytes` bounds the bytes
read from each report; it defaults to 4 GiB of source bytes and is not a
decoded-memory guarantee. `--output NEW.json` keeps the derived comparison
without replacing a file.

The tables show each report's median time and memory per workload and profile,
the recorded failure reasons, and each report's identity. For each report run
with clingo, a table per profile stands it against clingo: the cells where both
passed, fastest ratio first, each split into grounding, candidate proposal and
membership beside clingo's own grounding and solving times, headed by the
number of cells zetesis decided faster; a second table gives their peak memory
when it was measured. clingo's columns read "not run" for a report run without
it, apart from a missing or failed value's dash. `--json` prints the structured
comparison, which records what qualified each cell in each report. `--markdown`
prints the same comparison as Markdown tables: medians, ratios between the
reports, counters and the standing against clingo per cell.

## Machine output and exit status

| Command and view | Standard output |
| --- | --- |
| `run --json` | One compact summary: `zetesis_benchmark_summary`, schema 1; the full evidence is in the report file |
| `compare --json` | One structured derived comparison |
| `compare --markdown` | The comparison's Markdown tables |

Argument errors leave standard output empty and write a diagnostic to standard
error, including when `--json` was requested. Once a command has started, a
failure before any output write attempt emits one `zetesis-benchmark-failure`
document, schema 1. Once publication has been attempted, no second document is
appended: a writer can modify its sink before reporting failure, so JSON output
may be truncated. Check the process status; diagnostics remain on standard
error, and JSON never contains terminal styling.

Exit `0` means every requested measurement passed, `1` means the published
report contains non-passes, and `2` means setup, operation or publication
failed. `compare` exits `0` when the comparison view was produced, even if its
input reports contain failed measurements. Published performance claims remain
bound to the protocols and identities in the [performance records](performance.md).
