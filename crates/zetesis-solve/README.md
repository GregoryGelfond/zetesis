# zetesis-solve

Solve admitted programs from Rust and receive typed answer sets. This crate owns
sessions, execution policy, complete-family collection and failure evidence.
Applications provide source loading and presentation; no command-line parser or
output stream is required.

## Start here

The [library quickstart](../../docs/book/rust/getting-started.md) shows dependency
setup and a complete runnable example. Packages are consumed from this repository
through path dependencies or a pinned Git revision, not from crates.io.

Run the quickstart example from the checkout root:

```sh
cargo run --locked -p zetesis-solve --example solve --no-default-features
```

The complete [example source](examples/solve.rs) chooses two compatible tasks
from three. It prepares the ASP source, starts a CPU session, renders each full
interpretation beside its `#show` output and checks exhaustion before reporting
the two-answer complete family. Errors propagate to its fallible `main`.

For an application, source admission usually comes from `zetesis-themelios`.
`PreparedInput::formula` borrows the example's formula owner; normal programs can
instead use `PreparedInput::admitted`. `Session::enumerate` yields
`Result<AnswerSet, SolveFailure>`, and `AnswerSet::interpretation` exposes the full
true-atom set. Inspect `Session::outcome` separately to determine what completed.

## Choose the result you need

- `Session::enumerate` streams every original answer set with its score, including
  nonoptimal answers when objectives exist.
- `Session::new` uses ordinary optimization selection when an objective exists.
  An interrupted incumbent is not a proved optimum; inspect the outcome. A
  request for one optimal answer bounds the search strictly by default, so it
  proves the optimum without counting its ties (`Optimization::ties_pruned`);
  `SolveConfig::optimum_search` selects another schedule for measurement.
- `Session::builder(...).collect(...)` returns a `WorldView` only after complete
  capture within `WorldViewLimits`. Failure retains a checked prefix instead.
- `Session::builder(...).counting().start()` requests exhaustive full-answer
  counting, or all optimal ties under optimal selection. Drain the session, then
  read `complete_answer_count()` from its outcome; incomplete coverage returns
  `None`. Counting ignores projection and retains at most one optimal model.
  See [complete counts](../../docs/book/rust/outcomes.md).

`models: 0` requests all answers. A positive answer limit or early stop does not
establish exhaustive search. A complete empty `WorldView` establishes
inconsistency; an empty partial result, or one answer with no true atoms, does not.

Keep the admitted theory, atom catalog, objectives and observations together.
A prepared input borrows one coherent owner; it cannot combine unrelated source
admissions. Returned answers retain their subject and full interpretation,
independently of displayed projections.

## Execution and cancellation

`SolveConfig` selects CPU or device execution and named resource limits.
`zetesis_algebra::Cancellation` supplies cooperative cancellation and deadlines. GPU
support is enabled by default; use `default-features = false` for a CPU-only
consumer. Cargo feature unification can enable it through another dependency.
Compiling GPU support does not select a device.

GPU formula checks size their candidate slates, the candidates checked and
committed together, adaptively by default (`GpuBatching::Adaptive`). Slates
start at `batch_size`, return to it after a slate whose answers tightened the
objective bound, and double after a filled slate that left the bound unchanged,
bounded by pending memory and, never below `batch_size`, by admitted device
prefixes. `GpuBatching::Fixed` keeps every slate at `batch_size` for controlled
comparisons. Larger slates may delay answers and objective feedback; they do not
alter membership or permit partial batch publication.

`SessionBuilder` can accept shared `ExecutionResources`, optional
`SolveMeasurements`, and an `ExecutionObserver`. Membership is always decided by
zetesis's own reduct check. Shared resources do not reuse candidate truth, search
coverage or budgets.

`SolveMeasurements::stages_only()` records exclusive grounding, solving and
other host stages without detailed search clocks or frontend work counters.
Its snapshots contain stage measurements and driver elapsed time; detailed
phase and grounding entries remain absent. `SolveMeasurements::new(true)`
retains full instrumentation, while `new(false)` reads no measurement clocks
and returns no snapshots. `is_enabled()` includes stage-only scopes;
`details_enabled()` identifies detailed instrumentation. An injected scope's
detailed setting replaces `SolveConfig::stats`, so requesting a stage summary
does not enable detailed solver statistics. Lazy grounding remains interleaved
with solving, rather than appearing as a zero-duration eager stage.

Failures and interrupted searches preserve available evidence without claiming
exhaustion, inconsistency or optimality. Lean laws do not yet certify the complete
Rust or device implementation.

Continue with [sessions](../../docs/book/rust/sessions.md),
[completion and output](../../docs/book/rust/outcomes.md), or the
[library reference index](../../docs/book/rust/libraries.md).
