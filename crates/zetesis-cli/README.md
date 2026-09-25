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
answers, execution, limits and output options. Advanced help additionally
describes reduct selection, batching and individual resource ceilings.
Both views describe the same solver. File-first syntax, numeric `--models`
and the old option spellings remain compatibility adapters.

Omitting `--max-expansion-work` preserves independent library defaults for
source-term expansion and eager formula grounding. An explicit value sets both
ceilings; `--stats` reports their effective values. This pre-1.0 configuration API
change makes `Options::max_expansion_work` an `Option<usize>`: use `None` for those
defaults and `Some(limit)` for the shared override. Library admission continues
to accept separate `ExpansionLimits` and `FormulaLimits` without a CLI adapter.

Without objectives, the default returns one answer set; `--all` requests
exhaustive enumeration. With an active objective, the search phase ends before
retained incumbents are displayed. Exhaustion establishes optimality; an
interrupted search can return incumbents whose optimality remains unproved.
The `--answers N` limit bounds displayed ties. Equal displays can represent distinct
full answer sets.

`--time-limit DURATION` requests a cooperative deadline after input loading.
Whole nonnegative seconds, or a whole number with `s`, `m` or `h`, are accepted;
zero requests an immediate stop and
omission imposes no deadline. Search polls the same `Cancellation` used by library
consumers; a timer thread marks the deadline and each poll reads that mark
beside the cancellation flag, so an unreached deadline does not slow the run. A deadline during search leaves coverage incomplete. A later deadline
during publication preserves the already established search coverage. Either
stop returns exit 3; the deadline is not a hard process timeout for source I/O,
frontend work or a running device kernel.
`--stats` includes the requested duration. Library callers supply their own
control; this process option does not override it.

Every accepted answer must satisfy the original program and its reduct
acceptance criterion. For example, `a :- a.` has only the empty answer set.
Classical satisfaction alone does not establish stability.

## Results and presentation

Statistics are off by default; `--stats` requests them on stderr. Human output separates answer headings, atoms, optimization metadata and the
terminal result. `--color auto` resolves stdout and stderr independently,
respects nonempty `NO_COLOR` and `TERM=dumb`, and leaves redirected streams
plain. `always` and `never` provide explicit overrides.

Explicit human `solve --stats` uses compact phase, execution and work tables.
Requested settings remain separate from retained execution receipts; unavailable
measurements are not zeros. Work rows name their units and scope, including
completed-check closure totals and decoded device work. They are not additive
across operations. JSON statistics and the compatibility record view retain the
full counter catalog; selected eager rule/table rows in the compact view cover
rule instantiation only.

Answer headings use cyan with a bold label; optimization metadata uses italic
green. Source/oracle/grounder/backend labels use blue with italic gray values.
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
| 3 | Search or publication was interrupted. |

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

`--json` streams one schema-2 document without ANSI styling. Failed outcomes
retain a stable `error.kind`, the typed cause's human-readable `error.detail`,
and secondary-output-failure status. Detail is admitted under the same bounded
footer capacity as the rest of the outcome. Consumers should use the kind for
classification and retain the detail for diagnosis; historical schema-2 records
can omit this additive detail field. The document
spells each atom once: a model record's `atoms` are the typed atoms the
document has not spelled before, in the order it spells them, and the
document's atom table is every record's `atoms` in document order. A record's
`full_model` and its `shown.atom_indices` are indices into that table; shown
terms and costs stay per record, and terminal outcomes remain distinct. A
`#show` directive decides what is shown, as in human output; a program
without one shows the whole model. `--max-json-record-bytes` bounds each model
and terminal record, with an 8 MiB default, and `--max-atoms` bounds the
table. Integer consumers need lossless parsing. A failed writer can leave a
truncated document or partial human record; successful semantic checking does
not imply successful publication. See [JSON views](src/output.rs) and
[output regression tests](tests/json_output.rs).

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

`--threads auto|N` (`--workers` in existing scripts) defaults to at most four
available host threads, or one when availability is unknown. It sets the closure
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
Preparation and live query masks consume the existing source work and storage
budgets. `--stats` distinguishes actual table preparations, reuses and row visits.
See the [finite-table contract](../../docs/book/rust/finite-tables.md).

```sh
zetesis solve input.lp --device cpu
zetesis solve input.lp --device metal --grounder eager
zetesis solve input.lp --device metal --grounder lazy
zetesis solve input.lp --oracle countermodel --completion-workers 4 --all
```

Closure checks candidate gates against the least closure of the reduct,
constraints and candidate agreement. Countermodel checking rejects a model when
a proper subset satisfies its frozen Ferraris reduct. The latter uses native
Boolean search internally; semantic atoms alone define minimality and model
blocking. See [semantic foundations](../../docs/book/architecture/semantics.md).

GPU support is in the default build. Explicit `metal`, `vulkan`, `dx12` or
`gl` requests require that API. `gpu` accepts a physical adapter through a
compiled wgpu API; `nvidia` adds a vendor filter, without a CUDA backend.
Software, virtual and unknown adapter categories are refused. The CLI discovers
and initializes its device during invocation; solving needs no qualification
script or stored pass marker. `zetesis devices` lists visible capabilities,
but successful discovery alone does not guarantee shader initialization.

For relational closure:

| Grounder | CPU | Automatic hardware | Explicit GPU |
|---|---|---|---|
| `auto` | Lazy source joins | Lazy CPU source joins | Host source joins with per-world GPU consequences |
| `lazy` | Candidate-specific source joins | Lazy CPU source joins | Host source joins with per-world GPU consequences |
| `eager` | Packed static closure | Packed static CPU closure | Static lowering |

Automatic hardware selection retains CPU until measurements establish a GPU
crossover for a supported execution profile. An explicit GPU request prepares
its device during session setup and never silently falls back to CPU. Static
GPU closure admits at most 4,096 atoms.

Finite formula execution defaults to eager admission. Explicit lazy CPU execution
can stream ordinary constraints after complete support and arithmetic admission,
while retaining producers and ineligible constraints. This hybrid profile currently
refuses objectives, table joins and explicit devices; see the
[grounding contract](../../docs/book/architecture/grounding.md#eager-and-lazy-execution).
Automatic backend selection
uses CPU. With `--oracle auto`, complete-theory checks can select ranked support
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

`--gpu-formula-work` and `--gpu-formula-rounds` independently bound device
propagation per formula candidate. Their defaults are 100,000,000 charged work
units and 64 sweeps, matching the device library. `--max-work` bounds CPU
oracle/source work and each independent formula verification call. Formula
verification evaluates original truth or a frozen-reduct witness; certified
candidate checks also use this per-call ceiling. These operations retain their
own charged units. Cumulative encoding/search/certificate work remains bounded
by `--max-search-work`. Neither host limit sets the device propagation limit.
Values above `u32::MAX` are argument errors. Zero device work refuses a nonempty
setup that needs work; zero rounds still checks original truth and sends
undecided candidates to exact CPU residual search. Neither limit bounds driver
initialization or wall-clock duration.

Formula statistics report the effective device limits, actual submitted
batches/candidates and successfully decoded batches/candidates separately.
Propagation work and sweeps count decoded results; an interrupted unreturned
submission does not establish how much shader work completed.

`--completion-workers` controls the independent exact formula checks under
`--search clauses`, and under `--search regions` when one CPU worker walks
the tree or a device route runs; with more than one CPU worker under regions
the workers decide their leaves and it is unused. The default of one keeps
exact completion on the calling thread when that completion route applies;
it does not make parallel region search scalar. `--workers` is described above.
`--memory` is the session's memory allowance in bytes, half of the host's
physical memory by default and at least two gibibytes, or two gibibytes when
the host does not report its memory (Linux and macOS report it). The session's
byte ceilings, the projection, objective key, optimal, reduct, completion
scratch, candidate, closure, closure batch and batch bytes, are the shares of
a two-gibibyte allowance; each one not given on the command line is that
share scaled by the allowance, and the closure ceiling is shared by the
workers, as `SolveConfig::for_allowance` states, so a larger host admits
larger problems before one refuses, and a given ceiling is taken as given.
The admission and output ceilings, the source, expansion, support, JSON
record and observation bytes, keep their fixed defaults. Work, count and
structural ceilings are not memory and do not scale.
The ceilings bound named storage, not resident memory; `--stats` prints the
allowance, the host's memory and each ceiling as the session takes it.
`--batch-size`, `--max-batch-bytes` and
`--max-completion-scratch-bytes` bound batches and concurrent query storage.
General checking retains one candidate-parametric reduct encoding. The scratch
limit counts that shared owner once plus reserved query/result slots, excluding
allocator, thread, shared-theory and GPU overhead; it is not RSS.
`--max-reduct-bytes` independently bounds cold encoding preparation and each
complete query workspace. Preparation consumes cumulative search work once;
query work remains charged after reuse. A budget that cannot admit
one required query yields incomplete coverage. The direct CPU scalar cursor
does not consume the completion-batch allowance, but does use the reduct byte
limit. Optional class preparation and checking separately use the completion
scratch ceiling; the two uses do not establish a combined process-memory bound.

Advanced `--source-batching independent|union|worlds` selects relational source
sharing. Union/Worlds require lazy or automatic grounding with CPU/automatic
backend; explicit sharing resolves automatic hardware to CPU.
Source and per-world work have distinct ceilings. A stopped world makes the
whole batch incomplete. See [parallel execution](../../docs/book/rust/parallel.md).

Independent relational CPU execution prepares query dimensions once per session
and retains empty workspace capacities across batches. `--max-source-work` bounds
that preparation separately from candidate `--max-work`.
`--max-closure-bytes` bounds one candidate's reserved named capacity;
`--max-closure-batch-bytes` admits preparation, idle retained workspaces and
assigned candidate allowances together. CPU closure setup conservatively admits
every worker at the per-closure allowance, so `--threads` times
`--max-closure-bytes` must not exceed `--max-closure-batch-bytes`. It refuses an
excessive or overflowing product before allocating the execution pool, compiling
static rules or initializing candidates. This check also applies to eager and
shared CPU closure routes; formula and device execution use their own resource
checks. Source admission and policy checks can already have run. When
`--max-closure-bytes` is omitted, it is each worker's share of the collective
ceiling. Preparation refusal and
individual candidate refusal retain different interruption kinds. See the
[ownership contract](../../docs/book/architecture/ownership.md#memory-contracts).

Every byte ceiling in `--help-all` says which quantity it bounds: named reserved
capacity, canonical or encoded payload without capacity or allocator slack, or
original file bytes. Named capacity can include admitted allowances that are not
resident, while process RSS also includes storage outside those owners. These
quantities are not interchangeable, and none of these ceilings is a process RSS
cap.

## Statistics and resource limits

`--stats` leaves answer stdout unchanged and adds configured limits, actual
execution counters, completion and host timings to stderr. JSON exposes typed
views of the same information. Unavailable counters remain unavailable, not zero.

For independent relational CPU execution, `query_execution` reports actual
preparation builds/work, assigned and reused workspace slots, current retained
capacity and the latest reservation envelope. These count what was reserved, not
completed candidates or RSS. Reports retain a typed snapshot fault
separately from any earlier successful snapshot and any checked answers. Other
execution routes leave this observation absent.

For the independent CPU closure routes, lazy and eager, `closure_execution`
sums the counters every completed check returns: completed and stopped checks,
source rounds or rule passes, charged work in the route's `--max-work` units,
derived atoms and, for the lazy route, catalog work, bindings, tuple probes,
the heads recorded as bits of dense relations, the blocks of rows joined by
words and the largest admitted closure envelope. A stopped check returns no counters, so
its partial work is absent from the sums and counted only as a stop. The text
form is the `independent closure` and `closure joins` lines.

When relational admission expanded the source, the `expansion used` line
states each accepted charge beside the ceiling it was checked against, term
work, templates, values, scalar bytes and origin locations, and the JSON
`expansion` object holds the charges. The formula route admits through its
own budgets and reports no expansion usage.

Stage timings separate source preparation, eager grounding, solving and output.
Lazy joins are interleaved with solving, so a separate lazy grounding duration
is unavailable. Detailed phases distinguish candidate generation, membership,
objective work and output. GPU host-call time includes transport, submission,
waits and readback; it is not shader time. Worker intervals can overlap and are
reported separately from coordinator wall time.

Eager formula attribution renders the frontend's optional `domain_analysis`
phase and domain preparation, guard-row, comparison and rejection counters.
The ordinary command leaves this library option disabled: its phase is absent
and entered phases have zero domain work. A caller that prepares source through
the frontend's opt-in API may use the same `SolveMeasurements` observer to retain
actual domain work. These counters describe final-rule guards, not support
completion savings; unavailable values remain distinct from zero.

Failed attempts retain available timing/accounting. Timing completeness does not
prove semantic completeness. Instrumentation is optional and adds overhead.
See [telemetry](../zetesis-telemetry/README.md) and
[timing regressions](tests/phase_timing.rs).

Source, expansion, candidate search, witness checks, observations and optimal-model
retention have independent ceilings listed by `--help-all`. A limit never means
a smaller admitted program or proved inconsistency. Full models are counted
before `#show`; observation failure cannot publish a complete Answer record.
A retained incumbent remains unproved when search coverage is incomplete.
Runtime observation diagnostics resolve the failing directive against the loaded
original source, including included files. The returned typed error retains that
one source for later rendering; changed disk contents cannot replace its excerpt.
See the [diagnostic and publication regressions](tests/observation_diagnostics.rs).

## Compose the command adapter

Use `publish_prepared` with `PublicationConfig` and an `AnswerRenderer` to replace
presentation without parsing command arguments. Both built-in `HumanRenderer`
and `JsonRenderer` consume the same borrowed `AnswerView` and `PublicationView`
as custom consumers. The controller evaluates `#show` once in the observation
layer, streams one answer at a time and owns publication acknowledgements.
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

Maintained controls include [route selection](tests/oracle_selection.rs),
[source diagnostics](tests/bundles.rs),
[failure accounting](tests/failure_reports.rs),
[formula completion](tests/formula_completion.rs) and
[shared CPU execution](tests/shared_cpu.rs).
Portable regressions and actual physical-device tests have distinct purposes;
[contributing guide](../../CONTRIBUTING.md#verification-and-review) describes their execution.
