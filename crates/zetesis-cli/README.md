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

Without objectives, the default returns one answer set; `--models 0` requests
exhaustive enumeration. With an active objective, the search phase ends before
retained incumbents are displayed. Exhaustion establishes optimality; an
interrupted search can return incumbents whose optimality remains unproved.
The model limit bounds displayed ties. Equal displays can represent distinct
full answer sets.

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

A resource stop reports `INCOMPLETE`; it cannot establish UNSAT or optimality.
Verified incumbents may still carry fully evaluated costs. Exit codes are:

| Code | Meaning |
|---|---|
| 0 | The requested run completed. |
| 2 | Input, backend, protocol or output failure. |
| 3 | Search was interrupted. |

These are zetesis exit codes, not clingo's codes.

The process explicitly flushes standard output before returning. A failed flush
returns exit 2, including after an otherwise complete or interrupted search, and
does not replace an earlier failure. Redirected output uses a fixed
8 KiB staging buffer; terminal output bypasses that buffer so answers remain
prompt. This process policy does not change the library's record acknowledgement:
acceptance by a caller-supplied writer promises neither flushing nor durability.

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
| `auto` | Lazy source joins | Lazy CPU first; possible later lazy GPU batches | Host source joins with per-world GPU consequences |
| `lazy` | Candidate-specific source joins | Lazy CPU first; possible later lazy GPU batches | Host source joins with per-world GPU consequences |
| `eager` | Packed static closure | Static graph retained across GPU attempts and CPU fallback | Static lowering |

The automatic closure policy may attempt a GPU after the first seed when a batch
contains at least 32 candidates. This is a provisional heuristic, not a measured
performance crossover. Automatic failure can retry unreported work on CPU;
explicit hardware requests never silently fall back. Static GPU closure admits
at most 4,096 atoms.

General formula execution requires eager admission. Automatic backend selection
uses CPU; explicit GPU selection batches propagation and completes residual
reduct queries exactly on CPU. Outer candidate search and objective scoring also
remain on the host. This route is hybrid, and explicit lazy formula execution is
unsupported.

`--workers` controls closure workers. `--completion-workers` separately controls
independent exact formula checks, with a scalar CPU default of one.
`--batch-size`, `--max-batch-bytes` and
`--max-completion-scratch-bytes` bound batches and concurrent query storage.
The scratch limit includes logical reserved query/result slots, not allocator,
thread, shared-theory or GPU overhead; it is not RSS. A budget that cannot admit
one required query yields incomplete coverage. The direct CPU scalar cursor
does not consume the completion-batch allowance.

Advanced `--source-batching independent|union|worlds` selects relational source
sharing. Union/Worlds require lazy or automatic grounding with CPU/automatic
backend; explicit sharing resolves automatic hardware to CPU.
Source and per-world work have distinct ceilings. A stopped world makes the
whole batch incomplete. See [parallel execution](../../docs/book/rust/parallel.md).

## Statistics and resource limits

`--stats` leaves answer stdout unchanged and adds configured limits, actual
execution counters, completion and host timings to stderr. JSON exposes typed
views of the same information. Unavailable counters remain unavailable, not zero.

Stage timings separate source preparation, eager grounding, solving and output.
Lazy joins are interleaved with solving, so a separate lazy grounding duration
is unavailable. Detailed phases distinguish candidate generation, membership,
objective work and output. GPU host-call time includes transport, submission,
waits and readback; it is not shader time. Worker intervals can overlap and are
reported separately from coordinator wall time.

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
wants the ordinary source driver with injected output sinks. `PublicationReport`
separates the library's `SemanticOutcome` from publication acknowledgements.
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
