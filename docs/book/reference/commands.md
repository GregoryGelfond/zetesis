# Using the zetesis command

The `zetesis` command solves answer-set programs and checks selected contracts.
Install it using the [installation guide](https://github.com/GregoryGelfond/zetesis/blob/main/INSTALL.md).
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
| `test` | Check a corpus, scalability workloads or an execution backend. |
| `devices` | List the GPU devices found, their capabilities and the one `--backend gpu` would use. |
| `help` | Explain a command, including nested tasks. |
| `version` | Report the installed program version. |

Benchmarking is the separate `zetesis-bench` tool, described in
[Benchmarking with zetesis-bench](benchmarking.md); `zetesis bench` answers
with a pointer to it. Help and version requests do not read a program or
initialize a GPU.
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
| `--backend cpu` | CPU execution. `gpu` uses the platform's native API (Metal on macOS, Vulkan elsewhere); `metal` and `vulkan` name one. A GPU request is honored or refused. |
| `--threads auto` | At most four available host threads; one if availability is unknown. |
| `--grounder auto` | Prefer lazy source joins where admitted; formula admission can defer eligible terminal definitions and ground the remaining rules eagerly. |
| `--time-limit DURATION` | No deadline when omitted; accepts whole seconds or `s`, `m`, `h`. |
| `--memory-budget SIZE` | Host-based allowance for named storage; accepts bytes or `B`, `KiB`, `MiB`, `GiB`, `TiB`. |

For formula inputs, automatic admission can reconstruct terminal positive
definitions from each verified base answer. It returns full original answers;
`#show` does not determine which definitions qualify. See the
[grounding contract](../architecture/grounding.md#terminal-definition-analysis).
Explicit `--grounder eager` still requests complete materialization.

For a formula input, explicit `--grounder lazy` selects CPU hybrid grounding:
the producer core is materialized, while eligible integrity constraints are
checked from their admitted source families. This hybrid profile is separate from
automatic terminal-definition reconstruction. It runs on the CPU backend, uses indexed
joins and refuses objective declarations and table joins. Relational lazy
closure retains its existing CPU and device routes.

Hybrid statistics distinguish retained-core models from original answers accepted
after complete constraint checks. The existing source work, substitution and
scalar-byte options also set separate cumulative ceilings for constraint replay;
admission and replay do not share one remaining allowance. A stopped check is
incomplete, never an accepted answer or an UNSAT result.

```sh
zetesis solve program.lp --backend cpu --threads 4
zetesis solve program.lp --backend metal --grounder eager --all
zetesis solve program.lp --time-limit 30s --memory-budget 4GiB
```

An explicit positive thread count is not capped at four. The thread setting
selects the applicable host search or closure pool. General GPU execution also
has host candidate production and exact CPU completion; a GPU request does not
move all solving work to the device. An unavailable or failed GPU does not
silently retry on CPU. Choosing a backend per problem is not implemented, so no
value promises that choice: the retired `auto` is refused and names `cpu`, the
default; `dx12` and `gl` are refused because zetesis targets Metal and Vulkan;
`nvidia` is refused and names `gpu` or `vulkan`, since choosing one GPU among
several is not implemented yet.
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

Formula model construction has separate controls: `--max-model-work` bounds
cumulative preparation of atom order and construction of selected models;
`--max-model-bytes` bounds prepared ranks and active construction metadata.
Catalog storage and retained answer families remain under their own owners.
Work defaults to one billion operations. The byte default is 64 MiB before
scaling by the memory allowance; an explicit byte override is not scaled.

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

Detailed grounding attribution reports accepted formula work for support
construction. `support_construction_work` includes the complete fixed-point
build. Four disjoint subtotals identify its operations:

- `support_production_work`: selected-rule traversal, variants, domain guards,
  head production and subsequent producer selection.
- `support_order_work`: sorting pending support rows by typed identity.
- `support_wake_work`: preparing the next producer wake set.
- `support_publication_work`: initial, round and final publication, including
  canonical commit and relation postings.

Within production, `support_join_work` measures advancing support-generation
joins, including local joins, and `support_head_work` measures resolving,
admitting and selecting a derived head. These two disjoint subsets exclude join
setup and keyed-group validation. Do not add them to production or to the four
construction subtotals; the remaining production work includes those excluded
steps, rule traversal, variants, domain guards and subsequent producer selection.

Subtracting these subtotals from construction work leaves plan preparation,
initial scheduling, snapshot/query preparation and round control. Preparation
before this build and later completed-support query setup are outside the total.
These are accepted charges against the formula work limit, not durations or
expansion-budget units. Failed and unwinding operations retain their accepted
prefix; a refused charge itself is excluded. A partial count does not establish
completed support. The existing public phase sequence is unchanged.

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
report on stdout. Corpus and backend checks optionally retain that report with
`--report NEW.json`; scalability checks require this destination for their full
sealed evidence and present a compact view on stdout. Existing report files are
refused. Diagnostics go to stderr.

```sh
zetesis test corpus --repo . --clingo clingo
zetesis test corpus --repo . --json --report corpus-check.json
zetesis test backend --backend cpu --json
zetesis test backend --backend metal --stats --report metal-check.json
zetesis test scalability --threads 1,2,4,8,14 --report scalability-check.json
```

`test corpus` checks all 94 entries in the repository's pinned clean corpus
against external clingo. `--repo` defaults to the current directory; it must
contain `examples/correctness` and the maintained manifest. Inputs are verified
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
60-test physical qualification suite.

Corpus and backend checks accept every backend: `--backend cpu` (the default),
`gpu`, `metal` or `vulkan`. GPU corpus checks request the eager general formula
route. Decoding a Vulkan route awaits qualification on a Vulkan host. An
unavailable GPU remains a nonpass; it does not trigger CPU fallback.

`test scalability` uses the same nine workloads as `zetesis-bench corpus --suite
scalability`: authored queens at n=8/9/10, authored pigeonhole at h=5/6/7,
unchanged queens variant 02, SEND+MORE=MONEY and task allocation. It checks one
complete clingo family and one native family per requested thread count, with
no warmup, timed or RSS rounds. Native profiles request CPU eager grounding,
indexed formula joins, region search and one exact completion worker. Thread
counts default to `1,2,4,8,14`; one through eight profiles, each at most 256
threads, are admitted. The positional corpus root defaults to
`examples/correctness`, and `--examples` defaults to `examples`. Both must come
from the maintained checkout. `--include-einstein` adds the unchanged Einstein
riddle; `--max-expansion-work` supplies an explicit native grounding ceiling.
All workload contracts, original/derived source digests, executable identities,
observations and failed or unlaunched positions remain in `--report NEW.json`.
Amended inputs use the complete reference family rather than the original
default-size model count. A refusal or timeout remains a nonpass.

The scalability workflow requires Linux or macOS for bounded child capture.
Its overall scheduling deadline defaults to 1,800 seconds, adjustable with
`--campaign-seconds`; cumulative captures and serialized evidence are separately
bounded by `--total-capture-bytes` and `--report-bytes`. It never raises a native
work ceiling or replaces an incomplete case to finish the population.

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
for `test`: it stops further launches, settles owned children under the cleanup
bounds and retains incomplete evidence. Reusable library operations accept
caller-owned cancellation flags and install no global signal handlers. SIGKILL,
a crash or forced process termination cannot use this cooperative cleanup path.

`--stats` follows the test kind, for example `test backend --stats`.
It adds optional elapsed-time details and is off by default. Backend and
scalability checks always capture the internal statistics required to verify
actual execution; Metal corpus checks also require route telemetry. Turning off the optional
view never removes evidence needed to pass a check.

## Consume machine output

| Command with `--json` | Stdout format |
| --- | --- |
| `solve` | One versioned answer/outcome document |
| `test corpus`, `test backend` | One structured conformance report |
| `test scalability` | Compact `zetesis_scalability_conformance`, schema 1; full sealed evidence is at required `--report` |

The compact scalability view includes workload/profile identities, source and
executable seals, each check's decision and explanation, its blocker when any,
model count/cost, required route observations and process stop/exit/failure
diagnostics. Full bounded raw captures remain in the required report file.
`--stats` adds elapsed observations to the view; it does not change retained
evidence or schedule measurement rounds.

Argument parsing errors precede a typed command: they leave stdout empty and
write a diagnostic to stderr, including when `--json` was requested. Process
initialization failures, such as failure to register interruption handling, also
return exit 2 with a diagnostic and no machine document. Once the workflow has
started, a test failure before any stdout write attempt emits one
`zetesis-test-failure` document, including failure to publish the retained
report. It uses schema 1. Once publication has been attempted, no second
document is appended: a writer can modify its sink before reporting failure.
JSON output may therefore be truncated or retain only a prefix. Check the
process status and terminal report; successful earlier samples do not establish
completion.
Diagnostics remain on stderr, and JSON never contains terminal styling.

For `test`, exit `0` means all requested checks passed, `1` means the published
report contains nonpasses, and `2` means setup, operation or publication failed.
These statuses
do not replace the independent Rust, Lean, coverage, oracle and physical gates
in the [validation guide](validation.md). Published performance claims remain
bound to the protocols and identities in the [performance records](performance.md).

## Existing scripts

File-first invocations remain accepted, including `zetesis program.lp --models 0`.
Their missing-input default remains standard input, and their existing statistics
records are preserved. Prefer explicit `solve` in new scripts. The old flags
`--workers` and `--memory` remain aliases for `--threads` and `--memory-budget`;
`--models 0` maps to `--all`, and positive `--models N`
maps to `--answers N`. Conflicting answer-selection flags are rejected.

`--help-all` remains an alias for full solve help. Historical measurement recipes
keep the command spellings of the binaries they identify; do not rewrite those
records when migrating an ordinary invocation.
