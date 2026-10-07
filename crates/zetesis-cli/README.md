# zetesis-cli

The `zetesis` command composes source admission, reduct-based solving and human
or machine output. It owns argument mapping, source loading, injected writer
views and process exit policy. Composed solving belongs to
[`zetesis-solve`](../zetesis-solve/README.md); compatibility exports here refer to
the same solver types.

See [Install and run](../../README.md#install-and-run) for installation and the
[command guide](../../docs/book/reference/commands.md) for user tasks and everyday
options. The [session guide](../../docs/book/rust/sessions.md) covers Rust composition.

## Run a program

```sh
zetesis solve input.lp
zetesis solve encoding.lp instance.lp --all
zetesis solve input.lp --all --stats
zetesis solve input.lp --all --json
printf 'a :- not b. b :- not a.\n' | zetesis solve - --all
zetesis devices
zetesis help solve
zetesis help solve --advanced
```

Bare `zetesis` shows the task list. Compact solve help lists everyday inputs,
answers, execution, resources and output options. Advanced help additionally
describes reduct selection, joins and source batching. Both views describe the
same solver. File-first syntax and numeric `--models` remain compatibility
adapters, as do `--workers` for `--threads` and `--memory-budget` for `--memory`.

`--threads`, `--memory` and optional `--time-limit` select the ordinary resource
policy. One library-owned `zetesis_solve::Resources` derives named capacities for
source admission, grounding, solving and publication. Mandatory work, visited
substitutions, rounds and copied-byte traffic remain checked counters without
selected cumulative operation ceilings. Per-stage work/count switches and
completion or device-effort tuning are no longer command options. Explicit
library limit types remain available to consumers that need bounded operations.

```sh
zetesis solve input.lp --threads 4 --memory 4GiB --time-limit 30s
```

Without objectives, the default returns one answer set; `--all` requests
exhaustive enumeration. With an active objective, the search phase ends before
retained incumbents are displayed. Exhaustion establishes optimality; an
interrupted search can return incumbents whose optimality remains unproved.
The `--answers N` limit bounds displayed ties. Equal displays can represent distinct
full answer sets.

`--time-limit DURATION` requests a cooperative deadline after input loading.
Whole nonnegative seconds, or a whole number with `s`, `m` or `h`, are accepted;
zero requests an immediate stop and
omission imposes no deadline. Controlled source preparation, grounding and search
poll the same `Cancellation` used by library consumers; a timer thread marks the
deadline and each poll reads that mark beside the cancellation flag, without
reading the clock at each poll. A deadline before search exhaustion leaves
coverage incomplete. A later deadline
during publication preserves the already established search coverage. Either
stop returns exit 3; the deadline is not a hard process timeout for source I/O,
frontend work or a running device kernel.
`--stats` includes the requested duration. Library callers supply their own
control; this process option does not override it.

Every accepted answer must satisfy the original program and its reduct
acceptance criterion. For example, `a :- a.` has only the empty answer set.
Classical satisfaction alone does not establish stability.

## Results and presentation

Human output starts with version, copyright and license information, then the
selected backend, configured thread capacity and effective grounding mode.
An eager base with answer reconstruction is named explicitly. These describe
the prepared execution, not a claim that every configured thread or GPU was busy.
Answer headings, atoms, optimization metadata and the terminal result follow.
A short timing line separates eager grounding from solving; lazy and hybrid
execution report their combined time because source work is interleaved.

Detailed measurements are off by default. Human `solve --stats` appends stage,
execution and work tables on stderr after the answer summary. Requested settings
remain separate from retained execution receipts; unavailable measurements are
not zeros. Work rows name their units and scope, including completed-check
closure totals and decoded device work. They are not additive across operations.
JSON statistics and the compatibility record view retain the full counter
catalog; selected eager rule/table rows cover rule instantiation only.
Warnings and errors remain visible without `--stats`.

`--color auto` resolves stdout and stderr independently, respects nonempty
`NO_COLOR` and `TERM=dumb`, and leaves redirected streams plain. `always` and
`never` provide explicit overrides. Answer headings use cyan with a bold label;
optimization metadata uses italic green. Configuration and timing labels use
blue with italic gray values; statistics tables use no italics.
Untagged satisfiability verdicts use bold italic gray. Fatal diagnostics use red;
themelios syntax messages retain source locations and excerpts. ANSI palette
colors inherit the terminal theme. Styling never changes the semantic result.

A resource stop reports `INCOMPLETE`; the stop itself establishes neither UNSAT
nor optimality. Checked answers, proved optimality and completed search remain
established if only their later publication stops. Exit codes are:

| Code | Meaning |
|---|---|
| 0 | The requested run completed. |
| 2 | Input, backend, protocol or output failure. |
| 3 | Preparation, search or publication was interrupted. |

These are zetesis exit codes, not clingo's codes.

The process explicitly flushes standard output before returning. A failed flush
returns exit 2, including after an otherwise complete or interrupted search, and
does not replace an earlier failure. Redirected output uses a fixed
8 KiB staging buffer; terminal output bypasses that buffer so answers remain
prompt. This process policy does not change the library's record acknowledgement:
acceptance by a caller-supplied writer promises neither flushing nor durability.

The pinned Rust runtime on Linux and macOS reopens closed standard descriptors
as `/dev/null` before the command enters its own code. At that boundary, a closed
stdin is indistinguishable from an intentionally empty stream, and a closed
stdout can discard output without a write error. The command cannot recover the
parent's intent from those descriptors. Library callers own source loading and
supply explicit writers; observed I/O errors retain their original causes.

`--json` streams one schema-2 document without human headers or ANSI styling.
Timing fields are `null` unless measurements are requested with `--stats`.
Failed outcomes retain a stable `error.kind`, the typed cause's human-readable
`error.detail`, and secondary-output-failure status. Detail is admitted under the same bounded
footer capacity as the rest of the outcome. Consumers should use the kind for
classification and retain the detail for diagnosis; historical schema-2 records
can omit this additive detail field. The document
spells each atom once: a model record's `atoms` are the typed atoms the
document has not spelled before, in the order it spells them, and the
document's atom table is every record's `atoms` in document order. A record's
`full_model` and its `shown.atom_indices` are indices into that table; shown
terms and costs stay per record, and terminal outcomes remain distinct. A
`#show` directive decides what is shown, as in human output; a program
without one shows the whole model. The memory policy derives capacities for each
complete model or terminal record and for the document's atom table. Integer
consumers need lossless parsing. A failed writer can leave a truncated document
or partial human record; successful semantic checking does
not imply successful publication. See [JSON views](src/output.rs) and
[output regression tests](tests/integration/json_output.rs).

## Select an execution route

`--oracle auto` first tries the normal-rule closure specialization, then retries
eligible richer source through finite Ferraris countermodel checking. Syntax,
arithmetic and resource failures are preserved. The
[frontend guide](../zetesis-themelios/README.md) defines admitted source profiles.

Advanced `--search clauses|regions` selects the formula route's search
method, for proposing candidates and for the reduct's proper-subset query
alike. Regions, the default, walk the region tree over the theory's atoms,
narrowed by the theory's readings, for both: the leaves of the candidate
tree are proposed, and the proper subsets of a candidate are searched as a
second tree under the frozen reduct, so no clause form is built at all.
Clauses is the classical search over a Tseitin encoding for both, exact
exclusion of every candidate proposed and a clause query of the frozen
reduct.
The reduct decides membership either way, and a countermodel is validated
independently of the method that found it; the flag changes which
candidate is proposed next and the work charged, never whether one is
accepted. `--stats` reports the regions visited, refuted and reached as
leaves for the candidate tree and, under `--search regions`, for the reduct
queries.

`--threads auto|N` (`--workers` in existing scripts) defaults to the host's
available parallelism, or one when availability is unknown. It sets the closure
route's pool and, under `--search regions`, the region walkers. Native CPU region
workers decide the leaves they reach. Device region workers produce unchecked
leaves in bounded rounds; all producers join before device membership checking
and any exact CPU residual completion. The family of answer sets is the same as
with one worker, each answer once, and
with more than one worker the order in which answers appear is the
schedule's and differs between runs. Under an objective the optimum and the retained ties keep
their meaning; only the order among equally scored answers is unspecified.
Consumers that need an order sort, or run one worker. A ceiling stops every
worker; the answers verified before the stop are still printed, and the
coverage is partial.

Advanced `--formula-joins indexed|table` selects positive joins within eager
formula grounding. Indexed matching is the default. Table matching reuses
prepared masks for flat patterns over completed possible support; support growth,
structural patterns and relational-source grounding retain their existing paths.
The flag does not force a formula execution route or move grounding onto a GPU.
Preparation and live query masks retain work receipts and obey the derived
storage capacities and shared cancellation. `--stats` distinguishes actual table
preparations, reuses and row visits.
See the [finite-table contract](../../docs/book/rust/finite-tables.md).

```sh
zetesis solve input.lp --backend cpu
zetesis solve input.lp --backend metal --grounder eager
zetesis solve input.lp --backend metal --grounder lazy
zetesis solve input.lp --oracle countermodel --threads 4 --all
```

Closure checks candidate gates against the least closure of the reduct,
constraints and candidate agreement. Countermodel checking rejects a model when
a proper subset satisfies its frozen Ferraris reduct. The latter uses native
Boolean search internally; semantic atoms alone define minimality and model
blocking. See [semantic foundations](../../docs/book/architecture/semantics.md).

GPU support is in the default build. `--backend metal` or `vulkan` requires
that API; `gpu` uses the platform's native API, Metal on macOS and Vulkan
elsewhere.
Software, virtual and unknown adapter categories are refused. The CLI discovers
and initializes its device during invocation; solving needs no qualification
script or stored pass marker. `zetesis devices` lists visible capabilities,
but successful discovery alone does not guarantee shader initialization.

For relational closure:

| Grounder | CPU (the default backend) | Explicit GPU |
|---|---|---|
| `auto` | Lazy source joins | Host source joins with per-world GPU consequences |
| `lazy` | Candidate-specific source joins | Host source joins with per-world GPU consequences |
| `eager` | Packed static closure | Static GPU closure |

Backend selection is explicit; `--backend auto` is refused. An explicit GPU
request prepares its device during session setup and never silently falls back
to CPU. Static GPU closure admits at most 4,096 atoms.

Automatic finite formula admission can defer eligible terminal positive definitions
and reconstruct them from verified base answers; it grounds the remaining rules
eagerly. Explicit lazy CPU execution
can stream ordinary constraints after complete support and arithmetic admission,
while retaining producers and ineligible constraints. This hybrid profile currently
refuses objectives, table joins and explicit devices; see the
[grounding contract](../../docs/book/architecture/grounding.md#eager-and-lazy-execution).
With `--oracle auto`, complete-theory checks can select ranked support
on CPU or a device. Device tight checking evaluates original truth and complete
positive support without CPU residual queries. When no tight certificate is
selected, explicit GPU execution uses general propagation and completes
unresolved reduct queries exactly on CPU. Outer candidate search and objective
scoring remain on the host. Formula device execution requires an eager theory.

CPU automatic membership can also select positive least consequences, including
positive recursion and checks of all original constraints after closure. The
device route has no positive-plan specialization. Source analysis chooses the
certificate attempt order; it never replaces complete ground-theory validation.
`--oracle countermodel` retains the general reduct comparison route explicitly.

Formula device propagation uses internal dispatch bounds of 100,000,000 charged
work units and 64 sweeps per candidate. Undecided candidates continue through
exact CPU residual search; mandatory device setup can refuse a dispatch that
exceeds its capacity. These physical bounds do not impose a cumulative host
operation ceiling or a wall-clock deadline. Optional class, domain and pruning
analyses also retain finite effort bounds; declining them preserves the general
exact route.

Formula statistics report the effective device limits, actual submitted
batches/candidates and successfully decoded batches/candidates separately.
Propagation work and sweeps count decoded results; an interrupted unreturned
submission does not establish how much shader work completed.

Independent exact formula completion uses the calling thread by default;
parallel CPU region workers decide their own leaves. Batch sizing and completion
scheduling are internal defaults. Typed `SolveConfig` consumers can configure
these operations directly.

`--memory SIZE` accepts bytes or a whole number with `KiB`, `MiB`, `GiB` or `TiB`.
The default is half of reported host physical memory, at least two gibibytes,
or two gibibytes when the host does not report its memory. The shared policy
derives source, expansion-family, support, formula, session and output capacities
from that allowance, including population limits for retained values and nodes.
Checked depth and representation limits remain. `--stats` reports the allowance,
reported host memory and actual configured named capacities.

The capacities are independent shares for named owners, which can overlap.
Their sum is not the allowance, and neither the allowance nor the population
limits guarantee resident memory. Allocator overhead, source-library allocations
and thread stacks remain outside the named accounting.

General checking retains one candidate-parametric reduct encoding. Completion
scratch counts that shared owner once plus reserved query/result slots; cold
encoding preparation and each complete query workspace have their own derived
capacity. Preparation work is counted once, and query work remains counted after
reuse. A capacity that cannot admit one required query yields incomplete
coverage. The direct CPU scalar cursor does not consume the completion-batch
allowance but still obeys the query workspace capacity. Optional class
preparation and checking separately use the completion scratch capacity; their
receipts do not establish a combined process-memory bound.

Advanced `--source-batching independent|union|worlds` selects relational source
sharing. Union/Worlds require lazy or automatic grounding with the CPU backend.
Source and per-world work retain distinct receipts. A stopped world makes the
whole batch incomplete. See [parallel execution](../../docs/book/rust/parallel.md).

Independent relational CPU execution can adopt the compatible immutable query
preparation retained by candidate narrowing; otherwise, the oracle prepares its
own. It reuses that preparation and empty workspace capacities across batches,
while each candidate keeps private truth state. Importing an owner retains its
original preparation work and checks the oracle's preparation policy. The memory
policy assigns each worker a share of the collective closure capacity. Collective
admission includes preparation, idle retained workspaces and assigned candidate
allowances. Explicit library configurations still reject an excessive or
overflowing worker-capacity product before allocating the pool, compiling static
rules or initializing candidates. This check also applies to eager and shared
CPU closure routes; formula and device execution use their own resource checks.
Source admission and policy checks can already have run. Preparation refusal and
individual candidate refusal retain different interruption kinds. See the
[ownership contract](../../docs/book/architecture/ownership.md#memory-contracts).

Named reserved capacity, canonical or encoded payload, and original file bytes
remain different quantities. Named capacity can include admitted allowances that
are not resident, while process RSS includes storage outside those owners.
The typed library limits document these accounting boundaries.

## Statistics and resource limits

`--stats` leaves answer stdout unchanged and adds configured limits, actual
execution counters, completion and host timings to stderr. JSON exposes typed
views of the same information. Unavailable counters remain unavailable, not zero.

For independent relational CPU execution, `query_execution` reports oracle
preparation builds and adoptions, the retained owner's original preparation work,
assigned and reused workspace slots, current retained capacity and the latest
reservation envelope. An adoption avoids another build; zero oracle builds does
not mean no preparation work occurred. Capacity receipts describe reserved
storage, not completed candidates or RSS. Reports retain a typed snapshot fault
separately from any earlier successful snapshot and any checked answers. Other
execution routes leave this observation absent.

For the independent CPU closure routes, lazy and eager, `closure_execution`
sums the counters every completed check returns: completed and stopped checks,
source rounds or rule passes, charged work in the route's documented units,
derived atoms and, for the lazy route, catalog work, bindings, tuple probes,
the heads recorded as bits of dense relations, the blocks of rows joined by
words and the largest admitted closure envelope. A stopped check returns no counters, so
its partial work is absent from the sums and counted only as a stop. The text
form is the `independent closure` and `closure joins` lines.

When relational admission expanded the source, the `expansion used` line
states accepted term work, templates, values, scalar bytes and origin locations
beside their effective typed limits, and the JSON `expansion` object holds the
charges. Mandatory cumulative counters use their representation maxima under
the ordinary policy; retained populations use memory-derived capacities. The
formula route reports its own admission receipts, not expansion usage.

Coarse host stages support the default human timing line without enabling
detailed clocks. With `--stats`, tables separate source preparation, eager
grounding, solving and output. Lazy joins are interleaved with solving, so a
separate lazy grounding duration is unavailable. Detailed phases distinguish
candidate generation, membership, objective work and output. GPU host-call time
includes transport, submission, waits and readback; it is not shader time.
Worker intervals can overlap and are reported separately from coordinator wall time.

Eager formula attribution renders the frontend's optional `domain_analysis`
phase and domain preparation, guard-row, comparison and rejection counters.
Ordinary formula admission enables bounded domain analysis. A completed analysis
can guard eligible positive-flat rules during support completion and final rule
instantiation, including programs with objective declarations. Inapplicable or
incomplete analysis retains conservative domains; objective-local evaluation and
observations keep their own contracts. The counters report actual domain work
and rejected rows, not a speedup; unavailable values remain distinct from zero.

Failed attempts retain available timing/accounting. Timing completeness does not
prove semantic completeness. Instrumentation is optional and adds overhead.
See [telemetry](../zetesis-telemetry/README.md) and
[timing regressions](tests/integration/phase_timing.rs).

The ordinary resource policy applies across source preparation, grounding,
candidate search, witness checks, observations and optimal-model retention.
A storage refusal or cooperative stop never means a smaller admitted program or
proved inconsistency. Full models are counted before `#show`; observation failure
cannot publish a complete Answer record.
A retained incumbent remains unproved when search coverage is incomplete.
Runtime observation diagnostics resolve the failing directive against the loaded
original source, including included files. The returned typed error retains that
one source for later rendering; changed disk contents cannot replace its excerpt.
See the [diagnostic and publication regressions](tests/integration/observation_diagnostics.rs).

## Compose the command adapter

Use `publish_prepared` with `PublicationConfig` and an `AnswerRenderer` to replace
presentation without parsing command arguments. `PublicationConfig::default()`
derives solving and observation capacities from the same ordinary `Resources`
policy. Explicit typed limits remain available for bounded library operations
and failure-boundary tests. Both built-in `HumanRenderer` and `JsonRenderer`
consume the same borrowed `AnswerView` and `PublicationView`
as custom consumers. The controller evaluates `#show` once in the observation
layer, streams one answer at a time and owns publication acknowledgements.
The optional `configuration(ConfigurationView)` callback receives typed backend,
effective grounding and worker-capacity information when execution is selected.
It defaults to no output and can recur after a backend change.
`needs_stage_timings()` defaults to `false`; custom renderers that display host
stages must request them. `HumanRenderer` requests coarse stages by default;
`JsonRenderer` retains opt-in measurement behavior. The injected renderer owns
this choice independently of `Options::json`.
Renderers cannot strengthen membership or coverage by accepting a record.
See the [checked renderer example](../../docs/book/examples/answer-renderer.rs)
and [view contract](src/view.rs) for bounds, terminal stages and failure behavior.

Use `zetesis_solve::Session` for typed answer sets without rendering or command
options. Its builder composes selection, observations, measurements and reusable
device execution resources. Complete collection returns a `WorldView` only after
capturing all original answers. See the
[library manual](../../docs/book/rust/sessions.md) for ownership, resource reuse,
partial results and failures.

Use this crate's `run_finalized` and diagnostic/bundle variants when an application
wants the ordinary source driver with injected output sinks. These finalized
functions return `Result<PublicationOutcome, PublicationFailure>`: `Completed`
contains a `PublicationReport`, while `Stopped` retains the cooperative control
reason, checked semantic evidence and accepted record counts. Cancellation during
observation or encoding does not itself invalidate the writer; an incomplete
footer is attempted, and actual writer failures still prevent delivery.
Search exhaustion and a proved optimum survive a later publication stop.
The process returns exit 3 for that stop; actual writer or flush errors return 2.
`run` and `run_detailed` keep their legacy return shapes through the explicit
`PublicationOutcome::into_legacy` adapter. See the
[completion and output manual](../../docs/book/rust/outcomes.md) for migration.
`PublicationFailure` retains the original cause and any semantic, publication and
timing evidence. The solver library's `SolveFailure` has no publication state.
Legacy `run_detailed` variants derive their report views from the same evidence.

The command adapter pulls the public session API. It acknowledges a published
record only after the writer accepts the complete record. Verification, pending
candidates, queued answers, completed publication and the final summary have
separate accounting. An exhausted semantic result may coexist with later output
failure; publishing no answers does not establish inconsistency.
Generic injected writers use plain `ColorMode::Auto`; callers can request
explicit styling. Color policy is separate from `SolveConfig`.

See [publication](src/publication.rs), [failure adapters](src/failure.rs) and the
[outcome guide](../../docs/book/rust/outcomes.md). These APIs require no global
stdout or clingo invocation.

## Original sources

File arguments are ordered roots of one bundle with shared constants and original
locations. Includes use a captured working directory, then the including source
directory after a relative lookup failure. Exact repeated selected paths are
included once; ambiguous aliases and include cycles are refused.

Standard input (`-`) must be the only root and has no implicit include base.
Use `zetesis -- devices` for a first input literally named `devices`; once
another solver argument appears, later `devices` arguments are file names.

Maintained controls include [route selection](tests/integration/oracle_selection.rs),
[source diagnostics](tests/integration/bundles.rs),
[failure accounting](tests/integration/failure_reports.rs),
[formula completion](tests/integration/formula_completion.rs) and
[shared CPU execution](tests/integration/shared_cpu.rs).
Portable regressions and actual physical-device tests have distinct purposes;
[contributing guide](../../CONTRIBUTING.md#verification-and-review) describes their execution.
