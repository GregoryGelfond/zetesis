# Using the zetesis command

The `zetesis` command solves answer-set programs, checks selected contracts and
measures performance. Install it using the [repository instructions](https://github.com/GregoryGelfond/zetesis/blob/main/README.md#install-and-run).
Installed commands do not require Cargo at runtime. Programs use the admitted
[ASP language](language.md); these command examples are separate from the
themelios library's source-representation examples.

```sh
zetesis
zetesis solve examples/network-repair.lp --all
zetesis help solve
zetesis help solve --advanced
zetesis version
```

Bare `zetesis` displays concise help. Each task has its own options:

| Command | Purpose |
| --- | --- |
| `solve` | Find answer sets or optimize a program. |
| `test` | Check a corpus or an execution backend. |
| `bench` | Measure corpus or primitive performance, or compare saved reports. |
| `devices` | List available execution devices and their capabilities. |
| `help` | Explain a command, including nested tasks. |
| `version` | Report the installed program version. |

Help and version requests do not read a program or initialize a GPU.
`zetesis version` prints the version, copyright year and holder, and MIT license
on one line. `--version` and `-V` provide the same information.
`zetesis devices` performs discovery; it does not prove that a complete solve
will succeed on a listed adapter.

## Supply a program

Pass one or more source files, in the order in which their roots should be read.
Use `-` explicitly for standard input; it cannot be combined with file roots.

```sh
zetesis solve encoding.lp instance.lp --all
printf 'a :- not b. b :- not a.\n' | zetesis solve - --all
```

`solve` without a file or `-` is an argument error. Source loading, includes,
admission and source diagnostics retain their bounded contracts. A successful
parse does not establish that every construct is admitted; unsupported programs
receive an explicit diagnostic. Source diagnostics and arithmetic warnings go
to stderr. See the [language reference](language.md) for supported constructs
and the policy for evaluated zero divisors.

## Select answers

The default displays one answer set. Use a positive `--answers N` to request a
different display limit, or `--all` for exhaustive enumeration. The two options
cannot be combined.

```sh
zetesis solve program.lp --answers 3
zetesis solve program.lp --all
```

Without objectives, reaching the answer limit stops the request without claiming
that all answer sets were enumerated. With an objective, search still seeks a
proved optimum; the limit then selects how many tied optima to display. An
interrupted optimization may report verified incumbents whose optimality is
unproved. `--all` requests all tied optima, rather than every nonoptimal model.

Shown atoms and terms are a presentation of an answer set. Two full answer sets
can have the same display. Parallel search can change answer order and which
answers appear before a finite display limit; it preserves the complete family
when enumeration finishes. Use one thread or sort complete results when order
matters. The reduct remains the criterion for accepting an answer.

## Choose execution and limits

| Option | Default and meaning |
| --- | --- |
| `--device auto` | CPU execution. Explicit GPU requests are honored or refused. |
| `--threads auto` | At most four available host threads; one if availability is unknown. |
| `--grounder auto` | Prefer lazy source joins where admitted; use eager admission for general formulas. |
| `--time-limit DURATION` | No deadline when omitted; accepts whole seconds or `s`, `m`, `h`. |
| `--memory-budget SIZE` | Host-based allowance for named storage; accepts bytes or `B`, `KiB`, `MiB`, `GiB`, `TiB`. |

For a formula input, explicit `--grounder lazy` selects CPU hybrid grounding:
the producer core is materialized, while eligible integrity constraints are
checked from their admitted source families. Automatic formula admission remains
eager. The first hybrid profile accepts `--device cpu` or `auto`, uses indexed
joins and refuses objective declarations and table joins. Relational lazy
closure retains its existing CPU and device routes.

Hybrid statistics distinguish retained-core models from original answers accepted
after complete constraint checks. The existing source work, substitution and
scalar-byte options also set separate cumulative ceilings for constraint replay;
admission and replay do not share one remaining allowance. A stopped check is
incomplete, never an accepted answer or an UNSAT result.

```sh
zetesis solve program.lp --device cpu --threads 4
zetesis solve program.lp --device metal --grounder eager --all
zetesis solve program.lp --time-limit 30s --memory-budget 4GiB
```

An explicit positive thread count is not capped at four. The thread setting
selects the applicable host search or closure pool. General GPU execution also
has host candidate production and exact CPU completion; a GPU request does not
move all solving work to the device. Automatic device selection currently stays
on CPU. Explicit unavailable or failed devices do not silently retry on CPU.
The [execution chapter](../architecture/execution.md) explains each route.

The deadline starts after input loading and is cooperative. `0` requests an
immediate stop; blocking input, frontend operations and a running device kernel
cannot be preempted. Bare numeric durations retain their historical meaning in
seconds. Fractional values and overflowing unit conversions are rejected.

The memory allowance defaults to half of reported physical memory, with a
minimum of two GiB; an unavailable reading falls back to two GiB. It scales
specified session storage ceilings, not every allocation. Fixed admission/output
limits and work/count limits retain their own defaults. This allowance is not
a process RSS cap. Advanced help identifies which bytes each ceiling counts.

```sh
zetesis help solve --advanced
```

Reaching a resource limit or deadline leaves the affected operation incomplete.
That stop does not prove unsatisfiability or optimality. If search finished but
later publication stopped, its established search result remains distinct from
incomplete delivery.

## Read results and statistics

Human output uses automatic terminal color and italics. Each stream resolves
its styling independently; redirected streams stay plain in automatic mode.
A nonempty `NO_COLOR` or `TERM=dumb` disables automatic styling. `--color always` and
`--color never` explicitly override human styling. JSON is always unstyled.

Statistics are off by default. `--stats` requests phase and work measurements on
stderr; the human view groups them by subject. Required resource and correctness
accounting still operates when statistics are disabled. Optional statistics can
add measurement and output overhead.

The human view separates requested settings from retained execution receipts.
A selected adapter or route does not by itself prove work ran: device submissions
and decoded results are separate counters. Compact work rows include applicable
search/region work, closure checks, device work and eager rule/table activity.
Closure totals cover completed checks only; stopped checks are counted separately.
Eager rule/table rows cover rule instantiation, rather than every grounding phase.
Different work units and search subtotals must not be added together. Unavailable
receipts remain unavailable, even when hardware or instrumentation was requested.
The full counter catalog remains in JSON statistics, the compatibility record
view and the typed library values.

```sh
zetesis solve program.lp --all --stats
zetesis solve program.lp --all --json > answers.json
```

`--json` emits one versioned document on stdout. It preserves full model identity,
shown channels, costs and the terminal outcome. Diagnostics and optional
statistics remain on stderr; machine requests retain line-oriented statistics.
A writer failure can leave a truncated document. Consumers must check the final
outcome and process status rather than treat a valid prefix as a complete family.
See the [CLI stream reference](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-cli/README.md#results-and-presentation)
for the schema and publication boundary.

For solving, exit `0` means that the requested run completed, `2` reports an
input, backend, protocol or output failure, and `3` reports interrupted search or
publication. A completed finite answer request is not necessarily exhaustive.
These statuses are distinct from clingo's exit codes.

## Check conformance

`test` checks a declared contract. Its elapsed times are diagnostic observations,
not a benchmark. Human tables are the default; `--json` emits one structured
report on stdout, and optional `--report NEW.json` retains that report in a new
file. Existing report files are refused. Diagnostics go to stderr.

```sh
zetesis test corpus --repo . --clingo clingo
zetesis test corpus --repo . --json --report corpus-check.json
zetesis test backend --device cpu --json
zetesis test backend --device metal --stats --report metal-check.json
```

`test corpus` checks all 94 entries in the repository's pinned clean corpus
against external clingo. `--repo` defaults to the current directory; it must
contain `examples/kr-domains` and the maintained manifest. Inputs are verified
locally and are never downloaded. The comparison preserves selected displays,
model multiplicities, optimum ties and costs. It does not recover hidden clingo
interpretations from a projected display. Clingo remains an external reference,
never the grounder or solver for ordinary zetesis solving.

`test backend` needs no repository corpus or clingo installation. It checks
three fixed complete answer families and their requested execution routes:
tight support, a general reduct query, and optimum ties. It compares the
selected route with CPU execution and known full-model contracts. Actual route
and work evidence is mandatory; a requested device name alone cannot pass a
check. This small installed check is distinct from the repository's maintained
59-test physical qualification suite.

Both commands currently accept `--device cpu` (the default) or `--device metal`.
Metal corpus checks request the eager general formula route. This is the scope
of these check decoders, not a restriction of `solve`, whose explicit wgpu
backend choices are listed in solve help. An unavailable device remains a
nonpass; it does not trigger CPU fallback.

`test backend` currently requires Linux or macOS for bounded child-process
capture. On other platforms it reports the unavailable capture capability as a
nonpass. `test corpus` has a separate portable direct-child capture path; that
path does not provide the Linux/macOS process-group cleanup contract.

The native executable defaults to the installed command's own `solve` entry
point. `--zetesis PATH` selects another executable supporting that modern
interface. Corpus checks resolve `--clingo clingo` through `PATH`. Each child
has a default 30-second timeout and an 8 MiB combined stdout/stderr capture
ceiling, adjustable with `--timeout-seconds` and `--capture-bytes`.

On Linux and macOS, the process entry handles SIGINT and SIGTERM cooperatively
for `test` and `bench corpus`: it stops further launches, settles owned children
under the cleanup bounds and retains incomplete evidence. Reusable library
operations accept caller-owned cancellation flags and install no global signal
handlers. SIGKILL, a crash or forced process termination cannot use this
cooperative cleanup path. Primitive benchmarks do not currently consume this
campaign cancellation flag.

`--stats` follows the test kind, for example `test backend --stats`.
It adds optional elapsed-time details and is off by default. Backend checks
always capture the internal statistics required to verify actual execution;
Metal corpus checks also require route telemetry. Turning off the optional
view never removes evidence needed to pass a check.

## Measure a corpus

Benchmarks prescribe their instrumentation, including statistics. There is no
`bench --stats` toggle. Human tables use the shared terminal styling and remain
plain when redirected; `--json` selects the command's structured view.
`bench corpus` currently requires Linux or macOS for bounded process capture
and fresh-child RSS receipts. It refuses an unsupported platform explicitly;
saved-report comparison does not launch children and has no such requirement.

```sh
zetesis bench corpus examples/kr-domains --report cpu-run.json
zetesis bench corpus examples/kr-domains --suite baseline \
  --device metal --grounder eager --report metal-run.json
zetesis bench corpus examples/kr-domains --threads 2 --json \
  --report two-thread-run.json > two-thread-summary.json
```

The positional directory defaults to `examples/kr-domains` relative to the
current directory. Supply the directory from a repository checkout when running
elsewhere; the installed command does not fetch or bundle these inputs.
`--suite corpus` selects all 94 cases. `baseline` selects SEND, queens variant 02
and task allocation; `queens` selects the six unchanged encodings; `series`
selects the maintained 22 generated, constant-amended and original workloads.

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

The maintained corpus telemetry decoder supports `--device cpu|metal` and
`--grounder auto|eager|lazy`. It reports unsupported combinations and actual
execution failures rather than silently replacing the requested route. Explicit
thread counts are accepted within the campaign's finite bounds. Capture, decoder
and evidence byte ceilings are separate controls; `zetesis bench corpus -h`
shows the common options and `zetesis bench corpus --help` includes advanced
measurement controls.

`--report NEW.json` is required and stores the full bounded campaign evidence,
including executable/source identities, raw captures, failed and unlaunched
positions. Existing files are never replaced. A first nonpass disables later
launches for that cell; a campaign stop retains the remaining planned positions.
Neither is replaced by another sample. The human and JSON summaries compute
wall-time distributions only when the cell's entire timed population passed;
failures are not assigned a synthetic timeout duration. Validated memory rounds
have their own median and may be unavailable.

Omitting `--zetesis` measures this installed executable through its explicit
`solve` command. An explicit `--zetesis PATH` defaults to the legacy flat
interface for retained binaries; add `--native-interface solve` for another
modern executable. The report retains the actual argument sequences.

`--compare-grounders` requests eager and lazy profiles of the same native
executable, with identical device, worker, batch and completion settings. It
conflicts with an explicit `--grounder`. Clingo supplies one complete
qualification census per case; only the two native profiles have warmup,
timing and RSS rounds. These reports therefore contain no clingo timing or
memory comparison. Without this option, the existing single-profile campaign
continues to measure clingo in every phase.

```sh
zetesis bench corpus examples/kr-domains --suite queens --device cpu \
  --threads 1 --compare-grounders --repetitions 4 --memory-runs 2 \
  --report grounding-comparison.json
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
  --helper /absolute/path/to/zetesis --workers 1 \
  --report grounding-small.json > grounding-small-summary.json
```

The native and helper paths must name the current zetesis executable. The
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
  --clingo /absolute/path/to/clingo --helper /absolute/path/to/zetesis \
  --workers 4 --report grounding-refutation.json > grounding-refutation-summary.json
```

The larger cases can reach the fixed resource ceilings or timeout, particularly
when hybrid execution checks every core answer before rejecting it. Those
refusals remain nonpasses; an incomplete family does not supply a speed ratio.
Both studies compare complete native families within each workload. Clingo
supplies the selected-output census only, with no reference timings or RSS runs.

## Measure primitives

Primitive measurements compare matched operations and qualified result batches.
They exclude ordinary outer answer-set search and do not establish whole-solver
speedups. Each profile retains its own finite dimensions, reference checks,
setup intervals and timing boundaries.

```sh
zetesis bench primitives relation --device cpu --threads 2 \
  --rows 256 --queries 8 --report relation.jsonl
zetesis bench primitives tight --device metal --atoms 4 --batches 32 \
  --json > tight.jsonl
zetesis help bench primitives aggregate
```

| Profile | Measured operation | Default device |
| --- | --- | --- |
| `relation` | Packed equality masks and typed row reconstruction | CPU |
| `aggregate` | Exact native aggregate reductions | Metal |
| `tight` | Tight support classification with exact residual completion | Metal |
| `lazy` | Matched scalar, Rayon and lazy source-round operations | Metal |

All four profiles accept explicit `--device cpu|metal|vulkan`. Their retained
experiment defaults use four Rayon threads; select a positive `--threads N`
explicitly when another count is required. Physical selections never fall back
to CPU. Profile-specific help describes dimensions, work bounds and schedules.

`--json` emits the selected profile's versioned **JSON-lines event stream**,
including preparation, sample populations and a final `complete` event only
when the whole requested measurement succeeds. Optional `--report NEW.jsonl`
retains the same structured events independently of the human view. Its default
per-sink serialized ceiling is 256 MiB, adjustable with `--report-bytes`.
A stopped operation retains its event prefix; completed sample rows do not turn
an incomplete campaign into a pass.

The current primitive command requires a build with the `gpu` feature, including
when selecting a CPU primitive profile. CPU-only builds explicitly refuse this
command; corpus measurements and saved-report comparison remain available.
The compatibility `zetesis-bench` executable retains its other legacy experiment
profiles and views, documented in the [experiment library](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-experiments/README.md).

## Compare reports and consume machine output

```sh
zetesis bench compare --report before=cpu-before.json --report after=cpu-after.json
zetesis bench compare --report before=cpu-before.json --report after=cpu-after.json \
  --json --output comparison.json
```

`bench compare` launches no solver. It checks matching ordered workloads, sealed
corpus identities and requested profiles before deriving comparisons. Search
method is the explicitly supported profile difference; other profile settings,
such as thread counts, must match. `--report LABEL=PATH` can be repeated in the
chosen order. Optional `--output NEW.json` saves the derived comparison without
replacing a file. `--report-bytes` bounds each input document; it defaults to
4 GiB of source bytes and is not a decoded-memory guarantee.

| Command with `--json` | Stdout format |
| --- | --- |
| `solve` | One versioned answer/outcome document |
| `test corpus`, `test backend` | One structured conformance report |
| `bench corpus` | One compact summary: `zetesis_benchmark_summary`, schema 1; full evidence is at `--report` |
| `bench compare` | One structured derived comparison |
| `bench primitives …` | The selected profile's versioned JSON-lines events |

Argument parsing errors precede a typed command: they leave stdout empty and
write a diagnostic to stderr, including when `--json` was requested. Process
initialization failures, such as failure to register interruption handling, also
return exit 2 with a diagnostic and no machine document. Once the workflow has
started, a test failure before any stdout write attempt emits one
`zetesis-test-failure` document, including failure to publish the retained
report. A benchmark failure at that boundary emits one
`zetesis-benchmark-failure` document, including when no primitive event could
be produced. Both use schema
1. Once publication has been attempted, no second document is appended: a writer
can modify its sink before reporting failure. JSON/JSONL output may therefore be
truncated or retain only a prefix. Check the process status and terminal report
or `complete` event; successful earlier samples do not establish completion.
Diagnostics remain on stderr, and JSON never contains terminal styling.

For `test` and measurement commands, exit `0` means all requested checks passed,
`1` means the published report contains nonpasses, and `2` means setup, operation
or publication failed. `bench compare` exits `0` when the comparison view was
produced, even if its input reports contain failed measurements. These statuses
do not replace the independent Rust, Lean, coverage, oracle and physical gates
in the [validation guide](validation.md). Published performance claims remain
bound to the protocols and identities in the [performance records](performance.md).

## Existing scripts

File-first invocations remain accepted, including `zetesis program.lp --models 0`.
Their missing-input default remains standard input, and their existing statistics
records are preserved. Prefer explicit `solve` in new scripts. The old flags
`--backend`, `--workers` and `--memory` remain aliases for `--device`, `--threads`
and `--memory-budget`; `--models 0` maps to `--all`, and positive `--models N`
maps to `--answers N`. Conflicting answer-selection flags are rejected.

`--help-all` remains an alias for full solve help. Historical measurement recipes
keep the command spellings of the binaries they identify; do not rewrite those
records when migrating an ordinary invocation.
