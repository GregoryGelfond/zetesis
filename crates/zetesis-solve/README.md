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
cargo run --locked -p zetesis-solve --no-default-features --example book-getting-started
```

The [example source](../../docs/book/examples/getting-started.rs) admits a two-rule
ASP program, starts a CPU session, prints each full interpretation in Rust's
debug format and checks that search is exhausted. Errors propagate to its
fallible `main`.

For an application, source admission usually comes from `zetesis-themelios`.
`PreparedInput::admitted` borrows that owner, `Session::enumerate` yields
`Result<AnswerSet, SolveFailure>`, and `AnswerSet::interpretation` exposes the full
true-atom set. Inspect `Session::outcome` separately to determine what completed.

## Choose the result you need

- `Session::enumerate` streams every original answer set with its score, including
  nonoptimal answers when objectives exist.
- `Session::new` uses ordinary optimization selection when an objective exists.
  An interrupted incumbent is not a proved optimum; inspect the outcome.
- `Session::builder(...).collect(...)` returns a `WorldView` only after complete
  capture within `WorldViewLimits`. Failure retains a checked prefix instead.

`models: 0` requests all answers. A positive answer limit or early stop does not
establish exhaustive search. A complete empty `WorldView` establishes
inconsistency; an empty partial result, or one answer with no true atoms, does not.

Keep the admitted theory, atom catalog, objectives and observations together.
A prepared input borrows one coherent owner; it cannot combine unrelated source
admissions. Returned answers retain their subject and full interpretation,
independently of displayed projections.

## Execution and control

`SolveConfig` selects CPU or device execution and named resource limits.
`zetesis_cpu::Control` supplies cooperative cancellation and deadlines. GPU
support is enabled by default; use `default-features = false` for a CPU-only
consumer. Cargo feature unification can enable it through another dependency.
Compiling GPU support does not select a device.

`SessionBuilder` can accept shared `ExecutionResources`, optional
`SolveMeasurements`, and an `ExecutionObserver`. A caller-supplied `BatchExecutor`
is supported for formula membership under its explicit soundness contract.
Shared resources do not reuse candidate truth, search coverage or budgets.
Failures and interrupted searches preserve available evidence without claiming
exhaustion, inconsistency or optimality. Lean laws do not yet certify the complete
Rust or device implementation.

Continue with [sessions](../../docs/book/rust/sessions.md),
[completion and output](../../docs/book/rust/outcomes.md),
[custom executors](../../docs/book/rust/executors.md), or the
[library reference index](../../docs/book/rust/libraries.md).
