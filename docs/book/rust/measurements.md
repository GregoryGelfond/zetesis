# Observations and host measurements

An execution observation describes a selected route or attempted operation.
A measurement records host work. Neither establishes that an interpretation is
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

`SolveMeasurements::new(true)` creates one optional host-measurement scope.
Clones share the recorder and its original start time. Pass `&owner` through
`SessionBuilder::measurements`, which clones the handle; its enabled setting replaces
`SolveConfig::stats`. Independently created owners measure independent scopes.

| Operation | Recorded scope |
| --- | --- |
| `measure(phase, operation)` or `enter(phase)` | A completed attempted interval, including returned errors |
| `stage(stage)` | An exclusive host stage, with properly nested children removed from its parent |
| `grounding_observer()` | Closed frontend grounding attempts and their work counters |
| `snapshot()` | Available measurements without resetting the recorder |

Source preparation and consumer output can use the same owner as the session.
The application supplies those boundaries explicitly. A label cannot attest
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

The grounding profile also exposes `support_producer_visits` and
`support_snapshot_preparations`. These count entered operations, including those
followed by failure. They distinguish affected-producer scheduling from a full
round scan without asserting a timing benefit. Fields missing from older
reports remain unavailable.

## Compare independent reduct checks

The [`lazy_measurement` library](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-experiments/src/lazy_measurement.rs)
compares the same ordered candidate occurrences across scalar, Rayon and shared
CPU/device schedules. It checks complete closures and rejection reasons after
each timed call. This measures membership checking, not outer candidate search.

Independent scalar/Rayon samples sum actual completed checks' work, rounds,
bindings and tuple probes. A tuple probe is a row offered to whole-row matching,
including a rejected row; prefix comparisons and catalog lookup are separate.
Total work also includes preparation, so fewer probes alone do not establish a
cheaper operation. The maximum named per-candidate closure envelope is distinct
from the Rayon pool's reservation and process RSS. The pool reports its shared
preparation separately; repeated snapshots are not new preparation events.

JSON-lines schema 3 adds the tuple-probe observation. A missing field in an older
report means unavailable, not zero. Shared routes retain their own source/device
receipts. The [measurement contract](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-experiments/README.md)
specifies setup, reuse, timing and failed-report boundaries.

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
[measurement regressions](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-solve/tests/measurements.rs).
