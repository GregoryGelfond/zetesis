# Observations and measurements

An execution observation describes a selected route or attempted operation.
A measurement records host work or an explicitly enabled device interval. Neither establishes that an interpretation is
an answer set: membership and coverage belong to the session's semantic result.

## Observe execution

Implement `ExecutionObserver` to receive typed, borrowed
`ExecutionObservation` values. Use `SessionBuilder::start_observed` during setup
and `Session::next_observed` for later pulls. Each operation borrows its observer
independently; ordinary iterator pulls discard subsequent observations.
`SessionBuilder::collect_observed` uses one observer through setup and complete
collection. The solver retains no observer or event queue.

The [checked observer example](sessions.md#reuse-and-identity) records a selected
grounder. An application can instead update its own diagnostics, trace or user
interface. It owns those effects and their storage limits. A callback runs
synchronously, so its elapsed work contributes to the enclosing call.

The observer's error must implement `Error + Send + Sync + 'static`.
`SolveError::ExecutionObservation` preserves it separately from a device fault;
an observer failure cannot cause GPU fallback and a second observation attempt.
Initialization may return a setup failure or retain it for the first pull, as
documented on `Session::new_observed`. After a failed pull, subsequent pulls
return `None`. The failure retains any established evidence and original subject.

## Define a measurement scope

`SolveMeasurements::new(true)` records host stages, detailed phase intervals and
frontend grounding work. `stages_only()` records exclusive host stages and driver
elapsed time while leaving detailed phase and grounding entries absent.
`new(false)` disables every measurement clock and returns no snapshots.
`is_enabled()` includes stage-only scopes; `details_enabled()` identifies full
instrumentation.

Clones share the recorder and its original start time. Pass `&owner` through
`SessionBuilder::measurements`, which clones the handle; its detailed setting
replaces `SolveConfig::stats`. Independently created owners measure independent
scopes.

| Operation | Recorded scope |
| --- | --- |
| `measure(phase, operation)` or `enter(phase)` | A completed attempted interval, including returned errors |
| `stage(stage)` | An exclusive host stage, with properly nested children removed from its parent |
| `grounding_observer()` | Closed frontend grounding attempts and their work counters |
| `snapshot()` | Available measurements without resetting the recorder |

Source preparation and consumer output can use the same owner as the session.
The application supplies those boundaries explicitly. In a stage-only scope,
admission and observation phase guards still enter their coarse stages; other
phase guards read no clocks. The grounding observer records its coarse interval
and route without detailed counters. A label cannot attest
that the work was performed, and no timing record establishes source admission,
membership, complete search or delivery.

The following example measures admission and a complete CPU solve in one scope.
Its assertions concern presence and semantic completion, not a timing threshold.

```rust
# extern crate zetesis_solve;
# extern crate zetesis_cpu;
# extern crate zetesis_themelios;
{{#include ../examples/measurements.rs:example}}
```

Detailed phase intervals may overlap. Their sum is not coordinator wall time.
Formula sessions import only the new portion of their cumulative search timings;
repeated progress snapshots do not import it again. Grounding callbacks report
eager work separately when that frontend operation exposes the boundary. Lazy
source work occurs during membership, so it cannot also be claimed as a disjoint
grounding stage. Host intervals around GPU calls include preparation, transfers,
submission, waiting and readback; they are not shader timings.

Formula timing distinguishes `ReductPreparation`, the first attempt to compile
an immutable parametric query, from `ExactReductMembership`, the per-candidate
parameterization, subset search and returned-witness checks. Both remain within
the cumulative search allowance. Original satisfaction has its own interval;
successful preparation is not counted again when later batches reuse its owner.
The `reduct` statistics retain construction work and named capacity, original
evaluation work, parameter work and the largest observed worker workspace.
The collective completion receipt counts the shared prepared owner once.

`ModelConstruction` measures preparation of semantic atom ranks and selection
of verified formula interpretations into full models. It precedes scoring and
output and includes refused attempts. The separate model-construction receipt
records accepted operations and actual metadata peaks without requiring clocks;
constructed model counts establish neither publication nor complete enumeration.

The grounding profile also exposes `support_producer_visits`,
`support_rich_traversals`, `support_unchanged_visits` and
`support_snapshot_preparations`. These count entered operations, including
those followed by failure. Rich traversals and unchanged visits are disjoint
subsets of producer visits. `support_rich_traversals` includes the first round.
`support_unchanged_visits` counts the rich-producer visits after the first round
that ended without a traversal because nothing the producer reads had changed.
They distinguish affected-producer scheduling from a full round scan without
asserting a timing benefit. Fields missing from older
reports remain unavailable.

## GPU compute-pass timing

For formula and tight-support execution, `SolveConfig::stats` requests optional
wgpu timestamp queries when the session creates its own device. The CLI's
`solve --stats` uses this same library path. Adapter selection still applies the
selected primitive's capability requirements. No timing feature or storage is
requested by the ordinary statistics-off constructors.

Caller-supplied resources retain their exact context and compiled profile;
statistics do not replace either owner. To opt in explicitly, use
`GpuContext::new_selected_with_timing` and pass it through
`ExecutionResources::with_gpu`, or create a
`GpuFormulaProfile::new_selected_with_timing` for reuse. Standalone formula and
tight oracles also have `new_selected_with_timing` constructors.
`GpuContext::compute_timing()` reports `NotRequested`, `Unsupported` or `Enabled`.
Unsupported timestamps do not prevent solving.

`FormulaExecutionStatistics::gpu_compute_timing` counts each successfully
decoded batch once: measured, with its own compute pass's duration; unavailable,
when that interval was not established; or unresolved, when the record was taken
before the interval could be read. `elapsed` sums the measured durations and stays
absent until a valid sample is recorded. Equal device counter readings are
unavailable: they do not distinguish an unresolved interval from zero compute
cost. So are failed samples, unwritten or unrewritten counter slots, and intervals
longer than the host's own observation of their submission.
`measured + unavailable + unresolved` equals the decoded batches.
`is_complete()` requires at least one valid sample, no unavailable or unresolved
batch, and no accounting overflow. Submitted batches that fail before decoding
retain their submission receipts separately. Neither timing completeness nor a
timestamp establishes complete enumeration or optimality.

The interval covers the compute pass, including memory access within it. It
excludes host preparation, command-buffer copies, submission and waiting.
`GpuHostOracle` continues to measure the broader host call; these durations are
not interchangeable. Each device oracle owns four timestamp slots, used as two
pairs, for as long as it keeps its residency. A batch writes the pair that is not
pending. After its pass, the same command buffer resolves the previous batch's
pair, whose completion the host has already observed, and appends it to the
batch's ordinary readback, so every batch reports the one before it from that
batch's own pass. No batch adds a submission or a wait. When an execution
settles, one resolve-only submission reads its final batch's interval and is
counted as a flush, not a batch. A stopped execution skips it, and a settlement
that meets a busy or unhealthy context cannot make it; either reports that batch
unresolved. Admission reserves 80 logical bytes per timed batch: the
oracle's query slots, resolve buffer and flush readback, and the transport's
16-byte readback tail. This allowance does not measure additional driver-private
allocation. Timing adds device work and should be kept consistent across measured
versions.

JSON statistics retain the status, elapsed nanoseconds, the measured,
unavailable and unresolved batch counts, the flush submissions, and the explicit
`decoded_device_compute_passes` scope. Human statistics show the corresponding
interval and counts. The maintained benchmark runner already requests these
statistics and preserves them in its raw native reports.

## GPU workgroup size

`FormulaExecutionStatistics::gpu_workgroup_size`, the JSON key
`gpu_workgroup_size` and the human "workgroup size" report the invocations that
cooperated on each candidate in the decoded device batches, as an integer. One
value describes a solve: its execution owns one oracle checking one theory, and
the retained graph fixes the size. Under the default fitted sizing this is the
size the theory's dependency schedule fits
(see [formula evaluation on the device](../architecture/execution.md#formula-evaluation-on-the-device));
it is absent before the first decoded batch. Device timings, sweep counts and
round- or work-limit residuals can differ between sizes, so compare them only
between runs that dispatched the same size, or report the sizes beside them.
Fixed-point residuals agree across sizes where every propagation transfer is
order-preserving: established for binary gates, and checked on the device but
not proved for native groups and numeric rows
([the obligation](../lean/correspondence.md#ownership-and-execution-correspondence)),
so report the size beside a comparison of verdict classes too.

## GPU candidate slates

A slate is the candidates a GPU formula session's stream proposes together for
one membership check and commit; `batch_size` is the size of the first slate
and of the slate bound feedback returns to. The device checks a slate in one or
more submissions: `gpu_submitted_batches` and `gpu_batches` count those
submissions, and `gpu_rounds` counts propagation sweeps, which are unrelated to
slates. Three statistics, in `FormulaExecutionStatistics`, the JSON and the
human "formula GPU slates" line, describe the slates:

- `gpu_slates` ("committed"): slates committed with at least one candidate.
- `gpu_largest_slate` ("largest"): the largest committed slate's candidate
  count, at most the batch size under `GpuBatching::Fixed` and possibly more
  under the adaptive default. It is absent before the first commit.
- `gpu_slate_resets` ("feedback resets"): the selections that returned adaptive
  slates to the batch size because the previous slate's answers tightened the
  objective bound. Every such selection counts, including one already at the
  batch size and the session's last selection, which follows its last
  non-empty slate; a program reports one reset per slate whose answers
  installed a bound. It is zero under fixed batching and for programs without
  objectives.

All three are zero or absent on CPU routes. Slate sizes change search and
device work, so compare runs with matching policies, or report these
statistics beside them.

## Thread and failure contracts

The measurement owner is `Send + Sync`. Short internal locks protect fixed-size
bookkeeping; application operations and observer callbacks run outside those
locks. A session can move between threads, while its borrowed input must remain
alive. This does not make concurrent pulls on one session an operation: pulling
requires an exclusive mutable borrow.

Stage guards must close in stack order on their creating thread across all clones.
Transferring a live guard to another thread makes attribution unavailable.
Overlapping stage scopes on different threads make the exclusive partition
unavailable. Independent phase attempts can run concurrently and their durations
are summed. Separate grounding observers have separate active callback tokens;
callbacks on one observer must pair without nesting. Reuse on a later thread
after the prior scopes close does not by itself invalidate attribution.

Disabled measurements read no clocks and snapshots remain absent. Invalid stage
nesting, overflow or poisoned bookkeeping cannot change a solver result into a
failure or a completed result. The affected measurement carries unavailable or
overflow evidence instead. Timing completeness and semantic completeness remain
separate questions.

Optional measurements are distinct from resource accounting. Work allowances,
candidate coverage and cancellation remain active when diagnostics are disabled.
Enabled measurements incur clock reads, recording and consumer output; their
cost must be measured for the selected workload and worker count. Comparing CLI
runs with and without `--stats` includes serialization and output, not only the
recorder's cost. Parallel phase snapshots sum worker intervals and are not atomic
snapshots of a single instant; joined totals are exact unless marked incomplete.

The generated API pages for
[`SolveMeasurements`](../../doc/zetesis_solve/struct.SolveMeasurements.html) and
[`PhaseTimings`](../../doc/zetesis_solve/struct.PhaseTimings.html) require the
[combined book/API build](../building.md). Durable source references are
[`SolveMeasurements`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-solve/src/measurements.rs),
[`PhaseTimings`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-solve/src/phase_timing.rs), and the
[measurement regressions](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-solve/tests/integration/measurements.rs).
