# Benchmarking with zetesis-bench

`zetesis-bench` measures the installed `zetesis` against clingo on the example
programs in this repository, and compares saved runs. It runs both solvers as
separate programs, as a user would, and counts a time only after zetesis's
answers agree with clingo's. [INSTALL.md](https://github.com/GregoryGelfond/zetesis/blob/main/INSTALL.md)
installs it beside `zetesis`.

```sh
zetesis-bench corpus examples/correctness --report cpu-run.json
zetesis-bench compare --report before=cpu-before.json --report after=cpu-after.json
zetesis-bench --help
```

| Command | Purpose |
| --- | --- |
| `corpus` | Compare complete selected answer families, timings and memory with clingo. |
| `compare` | Compare saved reports with matching workload and profile identities. |
| `perf` | Measure a suite with the ordinary campaign or the instrumented profile matrix. |
| `series` | Compare published series reports: medians, ratios, counters and a scoreboard. |

`perf` and `series` were the separate `zetesis-perf` and `zetesis-series`
executables; the [validation crate's guide](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-validation/README.md)
describes their campaigns. On Linux and macOS, SIGINT and SIGTERM stop a
`corpus` campaign cooperatively: it launches nothing further, settles its
children under the cleanup bounds and publishes the incomplete evidence.

## Measure a corpus

Benchmarks prescribe their instrumentation, including statistics. There is no
`--stats` toggle. Human tables use the shared terminal styling and remain
plain when redirected; `--json` selects the command's structured view.
`corpus` currently requires Linux or macOS for bounded process capture
and fresh-child RSS receipts. It refuses an unsupported platform explicitly;
saved-report comparison does not launch children and has no such requirement.

```sh
zetesis-bench corpus examples/correctness --report cpu-run.json
zetesis-bench corpus examples/correctness --suite baseline \
  --backend metal --grounder eager --report metal-run.json
zetesis-bench corpus examples/correctness --threads 2 --json \
  --report two-thread-run.json > two-thread-summary.json
```

The positional directory defaults to `examples/correctness` relative to the
current directory. Supply the directory from a repository checkout when running
elsewhere; the installed tool does not fetch or bundle these inputs.
`--suite corpus` selects all 94 cases. `baseline` selects SEND, queens variant 02
and task allocation; `queens` selects the six unchanged encodings; `series`
selects the maintained 22 generated, constant-amended and original workloads.
`scalability` selects the nine authored/corpus workloads described above, with
indexed formula joins and region search. Only this suite accepts `--examples`
and `--include-einstein`; its authored root defaults to `examples`.

| Setting | Default |
| --- | --- |
| Native execution | CPU, automatic grounding and automatic reduct procedure |
| `--threads auto` | At most four available host threads; one if unknown |
| `--completion-workers` / `--clingo-threads` | One / one |
| `--batch-size` | 64 candidate occurrences |
| `--warmups` / `--repetitions` | One warmup / three timed rounds per solver and case |
| `--memory-runs` | One separate fresh-child RSS round per solver and case |
| `--timeout-seconds` / `--campaign-seconds` | 10 per child / 180 for campaign scheduling |

A qualification round precedes warmups and timings. Native children use
`solve --all --json --stats`; clingo retains its stock search heuristics and
exhaustive optimum-tie output. Wall time includes process launch, grounding,
solving and captured machine output. Qualification requires complete selected
output families and costs to agree. RSS is a separate child-resource observation,
not device memory or an allocator counter. Primitive work counts are not elapsed
time or machine instructions.

The maintained corpus telemetry decoder reads `--backend cpu|gpu|metal|vulkan`
and `--grounder auto|eager|lazy`; its Vulkan decoding awaits qualification on a
Vulkan host. It reports unsupported combinations and actual
execution failures rather than silently replacing the requested route. Explicit
thread counts are accepted within the campaign's finite bounds. Capture, decoder
and evidence byte ceilings are separate controls; `zetesis-bench corpus -h`
shows the common options and `zetesis-bench corpus --help` includes advanced
measurement controls.

`--report NEW.json` is required and stores the full bounded campaign evidence,
including executable/source identities, raw captures, failed and unlaunched
positions. Existing files are never replaced. A first nonpass disables later
launches for that cell; a campaign stop retains the remaining planned positions.
Neither is replaced by another sample. The human and JSON summaries compute
wall-time distributions only when the cell's entire timed population passed;
failures are not assigned a synthetic timeout duration. Validated memory rounds
have their own median and may be unavailable.

Non-pass summaries include the recorded reason and schedule phase. If a failed
qualification prevented later samples, those slots identify the original blocker.
Known resource limits and required amounts remain in the reason; missing historical
details are labelled unavailable. A refusal, timeout or process failure never
counts as an UNSAT result or a successful timing sample.

Omitting `--zetesis` measures the installed `zetesis`, the one beside
`zetesis-bench` or else the first on `PATH`, through its explicit `solve`
command. An explicit `--zetesis PATH` defaults to the legacy flat
interface for retained binaries; add `--native-interface solve` for another
modern executable. The report retains the actual argument sequences.

`--compare-grounders` requests eager and lazy profiles of the same native
executable, with identical device, worker, batch and completion settings. It
conflicts with an explicit `--grounder`. Clingo supplies one complete
qualification census per case; only the two native profiles have warmup,
timing and RSS rounds. These reports therefore contain no clingo timing or
memory comparison. Without this option, the existing single-profile campaign
continues to measure clingo in every phase unless a thread comparison is selected.

`--compare-threads 1,2,4,8,14` compares native profiles differing only in their
candidate/closure thread count. It conflicts with explicit `--threads` and with
`--compare-grounders`. It accepts one through eight profiles with each count at
most 256. Clingo supplies only the qualification census; native profiles retain
the requested warmup, timing and RSS rounds. `--max-expansion-work` applies the
same explicit grounding ceiling to every native profile and records it in the
evidence. This is independent of the process deadline.

```sh
zetesis-bench corpus examples/correctness --suite queens --backend cpu \
  --threads 1 --compare-grounders --repetitions 4 --memory-runs 2 \
  --report grounding-comparison.json
zetesis-bench corpus examples/correctness --suite scalability --examples examples \
  --grounder eager --compare-threads 1,2,4,8,14 --repetitions 4 --memory-runs 2 \
  --timeout-seconds 30 --campaign-seconds 1800 --report thread-comparison.json
```

All native populations must agree on complete full-model identities as well as
the selected outputs and costs qualified by clingo. Lazy formula execution
records the eager retained core and streamed constraint checks separately.
Deferring constraints can increase core enumeration work, so a refusal or a
slower lazy result is a meaningful result of the comparison. Four timed rounds
balance the rotating eager/lazy order; RSS rounds remain separate.

For hybrid formula execution, the grounding interval covers initial support and
core admission. Streamed constraint checks occur during solving and are included
in `original_validation`. The initial grounding time alone therefore does not
measure all source evaluation work. Compare complete solve time, storage and
the separate constraint-work counters as well.

For a smaller fixed population, the maintained
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
clingo invocations. A nonpass exits unsuccessfully after publishing the evidence.

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
refusals remain nonpasses; an incomplete family does not supply a speed ratio.
Both studies compare complete native families within each workload. Clingo
supplies the selected-output census only, with no reference timings or RSS runs.

## Compare reports

```sh
zetesis-bench compare --report before=cpu-before.json --report after=cpu-after.json
zetesis-bench compare --report before=cpu-before.json --report after=cpu-after.json \
  --json --output comparison.json
```

`compare` launches no solver. It checks matching ordered workloads, sealed
corpus identities and requested profiles before deriving comparisons. Search
method is the explicitly supported profile difference; other profile settings,
such as thread counts, must match. `--report LABEL=PATH` can be repeated in the
chosen order. Optional `--output NEW.json` saves the derived comparison without
replacing a file. `--report-bytes` bounds each input document; it defaults to
4 GiB of source bytes and is not a decoded-memory guarantee.

## Machine output and exit status

| Command with `--json` | Stdout format |
| --- | --- |
| `corpus` | One compact summary: `zetesis_benchmark_summary`, schema 1; full evidence is at `--report` |
| `compare` | One structured derived comparison |

Argument errors leave stdout empty and write a diagnostic to stderr, including
when `--json` was requested. Once a command has started, a failure before any
stdout write attempt emits one `zetesis-benchmark-failure` document, schema 1.
Once publication has been attempted, no second document is appended: a writer
can modify its sink before reporting failure, so JSON output may be truncated.
Check the process status; diagnostics remain on stderr, and JSON never contains
terminal styling.

Exit `0` means every requested measurement passed, `1` means the published
report contains nonpasses, and `2` means setup, operation or publication
failed. `compare` exits `0` when the comparison view was produced, even if its
input reports contain failed measurements. Published performance claims remain
bound to the protocols and identities in the [performance records](performance.md).
