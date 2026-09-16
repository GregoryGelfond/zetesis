# zetesis-cli

The `zetesis` command composes source admission, reduct-based solving and human
or machine output. It owns argument mapping, source loading, injected writer
views and process exit policy. Composed solving belongs to
[`zetesis-solve`](../zetesis-solve/README.md); compatibility exports here refer to
the same solver types.

See [Install and run](../../README.md#install-and-run) for installation and the
[session guide](../../docs/book/rust/sessions.md) for Rust composition.

## Run a program

```sh
zetesis input.lp
zetesis encoding.lp instance.lp --models 0
zetesis input.lp --models 0 --stats
zetesis input.lp --models 0 --json
printf 'a :- not b. b :- not a.\n' | zetesis --models 0
zetesis devices
zetesis --help
zetesis --help-all
```

The compact help lists everyday input, enumeration, backend, grounder and output
options. `--help-all` additionally describes oracle selection, workers, batching
and resource ceilings. Both views describe the same solver.

Omitting `--max-expansion-work` preserves independent library defaults for
source-term expansion and eager formula grounding. An explicit value sets both
ceilings; `--stats` reports their effective values. This pre-1.0 configuration API
change makes `Options::max_expansion_work` an `Option<usize>`: use `None` for those
defaults and `Some(limit)` for the shared override. Library admission continues
to accept separate `ExpansionLimits` and `FormulaLimits` without a CLI adapter.

Without objectives, the default returns one answer set; `--models 0` requests
exhaustive enumeration. With an active objective, the search phase ends before
retained incumbents are displayed. Exhaustion establishes optimality; an
interrupted search can return incumbents whose optimality remains unproved.
The model limit bounds displayed ties. Equal displays can represent distinct
full answer sets.

`--time-limit SECONDS` requests a cooperative deadline after input loading.
Whole nonnegative seconds are accepted; zero requests an immediate stop and
omission imposes no deadline. Search polls the same `Control` used by library
consumers. A deadline during search leaves coverage incomplete. A later deadline
during publication preserves the already established search coverage. Either
stop returns exit 3; the deadline is not a hard process timeout for source I/O,
frontend work or a running device kernel.
`--stats` includes the requested duration. Library callers supply their own
control; this process option does not override it.

Every accepted answer must satisfy the original program and its reduct
acceptance criterion. For example, `a :- a.` has only the empty answer set.
Classical satisfaction alone does not establish stability.

## Results and presentation

Human output separates answer headings, atoms, optimization metadata and the
terminal result. `--color auto` resolves stdout and stderr independently,
respects nonempty `NO_COLOR` and `TERM=dumb`, and leaves redirected streams
plain. `always` and `never` provide explicit overrides.

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

`--json` streams one schema-1 document without ANSI styling. Full semantic atoms,
shown atom indices, shown terms, costs and terminal outcomes remain distinct.
`--max-json-record-bytes` bounds each model and terminal record, with an 8 MiB
default. Integer consumers need lossless parsing. A failed writer can leave a
truncated document or partial human record; successful semantic checking does
not imply successful publication. See [JSON views](src/output.rs) and
[output regression tests](tests/json_output.rs).

## Select an execution route

`--oracle auto` first tries the normal-rule closure specialization, then retries
eligible richer source through finite Ferraris countermodel checking. Syntax,
arithmetic and resource failures are preserved. The
[frontend guide](../zetesis-themelios/README.md) defines admitted source profiles.

Advanced `--formula-joins indexed|table` selects positive joins within eager
formula grounding. Indexed matching is the default. Table matching reuses
prepared masks for flat patterns over completed possible support; support growth,
structural patterns and relational-source grounding retain their existing paths.
The flag does not force a formula execution route or move grounding onto a GPU.
Preparation and live query masks consume the existing source work and storage
budgets. `--stats` distinguishes actual table preparations, reuses and row visits.
See the [finite-table contract](../../docs/book/rust/finite-tables.md).

```sh
zetesis input.lp --backend cpu
zetesis input.lp --backend metal --grounder eager
zetesis input.lp --backend metal --grounder lazy
zetesis input.lp --oracle countermodel --completion-workers 4 --models 0
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

General formula execution requires eager admission. Automatic backend selection
uses CPU; explicit GPU selection batches propagation and completes residual
reduct queries exactly on CPU. Outer candidate search and objective scoring also
remain on the host. This route is hybrid, and explicit lazy formula execution is
unsupported.

With `--oracle auto` on CPU, complete-theory checks can select ranked support or
positive least consequences. The latter includes positive recursion and checks
all original constraints after closure. Source analysis chooses attempt order;
it never replaces complete ground-theory validation. `--oracle countermodel`
retains the general reduct comparison route explicitly.

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

`--workers` controls closure workers. `--completion-workers` separately controls
independent exact formula checks, with a scalar CPU default of one.
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
`--max-closure-bytes` bounds one candidate's named storage;
`--max-closure-batch-bytes` admits preparation, idle retained workspaces and
assigned candidate allowances together. Preparation refusal and individual
candidate refusal retain different interruption kinds. See the
[ownership contract](../../docs/book/architecture/ownership.md#memory-contracts).

## Statistics and resource limits

`--stats` leaves answer stdout unchanged and adds configured limits, actual
execution counters, completion and host timings to stderr. JSON exposes typed
views of the same information. Unavailable counters remain unavailable, not zero.

For independent relational CPU execution, `query_execution` reports actual
preparation builds/work, assigned and reused workspace slots, current retained
capacity and the latest reservation envelope. These are ownership receipts, not
completed candidate counts or RSS. Reports retain a typed snapshot fault
separately from any earlier successful snapshot and any checked answers. Other
execution routes leave this observation absent.

For the independent CPU closure routes, lazy and eager, `closure_execution`
sums the counters every completed check returns: completed and stopped checks,
source rounds or rule passes, charged work in the route's `--max-work` units,
derived atoms and, for the lazy route, catalog work, bindings, tuple probes and
the largest admitted closure envelope. A stopped check returns no counters, so
its partial work is absent from the sums and counted only as a stop. The text
form is the `independent closure` and `closure joins` lines.

When relational admission expanded the source, the `expansion used` line and
the JSON `expansion` object state each accepted charge beside the ceiling it
was checked against: term work, templates, values, scalar bytes and origin
locations. The formula route admits through its own budgets and reports no
expansion usage.

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
