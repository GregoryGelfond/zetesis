# Supplying a membership executor

`SessionBuilder::executor` supplies one implementation of `BatchExecutor` for an
already admitted formula input. This is an execution boundary within the ordinary
session. Candidate generation, the original theory, its checked class certificate,
exact residual completion, objectives and publication retain their existing owners.
The executor neither emits `AnswerSet` values nor establishes search exhaustion.

This interface accepts `PreparedInput::formula` and `formula_bundle`.
Relational and prepared-ground inputs return a typed refusal; the library does
not replay source or materialize a different profile. The explicit executor
requires `Backend::Auto`. A simultaneous explicit builtin backend request is a
conflict, and `ExecutionResources` remains an input to builtin execution only.
The executor can own shared infrastructure handles inside its own configuration.

## Select the operation before executing it

`ExecutorCapabilities` declares general reduct checking, tight support, or both.
With `Oracle::Auto`, a tight-capable executor can receive the same complete
`TightPlan` prepared and accounted by the candidate enumeration. Its shared
`Arc` can be retained without constructing another certificate. If tight
preparation declines, general capability is required. `Oracle::Countermodel`
requests the general operation explicitly. Missing capability refuses the
request without selecting another executor.

`prepare` receives the immutable `MembershipPlan` once. A general plan contains
the exact original `Theory`; a tight plan authenticates complete original-root
coverage and its applicability conditions. Objective restrictions never enter
that theory. `check` receives a borrowed `CandidateBatch` of original models in
proposal order. Each candidate fixes its own reduct. Preparing a plan establishes
no candidate's membership.

CPU residual and wgpu batch adapters use the same request/receipt boundary.
The scalar CPU region schedule remains available independently; batching is an
execution choice, not a new definition of membership.

## State the trusted boundary

The host independently validates original satisfaction before an executor sees
a candidate. `CandidateBatch::finish` checks the result count and associates the
receipt with that exact subject and candidate slice. The host checks this
association before consuming the receipt. These checks do not prove the executor's
verdicts or detect verdicts exchanged between positions in the same batch.

| Verdict | Required evidence |
| --- | --- |
| `NoProperSubset` | Original satisfaction and no proper-subset model of the frozen reduct, by a completed exact check or an applicable complete certificate. |
| `Refuted` | A sound nonminimality argument, such as a checked proper-subset reduct model or failed support under the complete tight plan. |
| `Residual` | No decision; the host must finish exact membership under its cumulative limits. |
| `NotModel` | Contradicts the original-model producer; the host refuses this as an invariant failure. |

An adapter must preserve order, exact theory identity and candidate identity.
A changed epoch, lost completion message, work limit or device fault cannot be
converted to a decisive verdict. Existing Lean laws compose sound membership
and complete candidate coverage conditionally. They do not certify an arbitrary
Rust adapter, transport, shader or device.

## A bounded reference adapter

This implementation uses the public exhaustive Ferraris reference checker. Its
cost is exponential in candidate size, so its per-candidate work and subset
limits are explicit. It illustrates the contract and is not a faster backend.

The ordinary builder then collects its complete answer-set family:

```rust
# extern crate zetesis_cpu;
# extern crate zetesis_ferraris;
# extern crate zetesis_solve;
# extern crate zetesis_themelios;
{{#include ../examples/batch-executor.rs:example}}
```

Run the maintained example and its assertions with:

```sh
cargo run --locked -p zetesis-solve --no-default-features --example book-batch-executor
```

## Ownership, limits and failure

The builder moves an executor into one boxed session owner. Methods run
synchronously on the thread starting or pulling the session; executor state is
not cloned between sessions. The host joins parallel candidate producers before
checking a batch. Their pending candidates remain owned by `StableModels` until
membership completes. Exact residual jobs likewise join before batch commit.

The host's candidate, batch, search and completion ceilings retain their existing
units. An executor must separately configure and bound its compilation, callback
work, result-buffer capacity, device memory and driver allocations. These are
not bounded by `SolveConfig` or measured as process RSS. Returned verdict storage
has one logical slot per candidate in the host's pending allowance; extra
capacity and other executor-private allocations remain the executor's responsibility.
The example's reference limits apply to each candidate, not the whole solve.

`ExecutorFailure::Interrupted` retains a typed resource or control stop and
cannot establish exhaustion. `Unsupported` is an explicit execution refusal.
`Failed` retains the original typed external cause for downcasting through
`SolveError::Executor`. Malformed result counts retain the existing
`SolveError::FormulaBatchShape` boundary. None triggers backend replacement.
Panics are not caught, and cancellation cannot undo effects already performed.

An executor setup fault returns from `start` with the original subject and
available attempted timing. A setup interruption returns a stopped session.
After checking begins, a fault leaves already verified answers valid and retains
pending-candidate accounting without claiming coverage. Ordinary sessions
terminate on that fault; they do not retry the callback. `Cancellation` must be polled
cooperatively by the executor as well as by the host.

`SemanticOutcome::batch_execution()` records declared capabilities, the selected
operation, checker/commit/pending counts, exact completion attempts and queued
verified models. It does not invent device work or identify hardware. Existing
GPU receipts remain separate. The public conformance tests compare complete
families and objectives, exercise general and tight plans, and check refusal,
malformed output, cancellation and preserved failure evidence.

The [executor API](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-solve/src/batch_executor.rs),
[batch orchestration](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-solve/src/formula_queue.rs)
and [public conformance tests](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-solve/tests/batch_executors.rs)
show the contract and its integration with ordinary sessions.

This API is a composition point for a future device adapter. It is not an
implementation or qualification of Loihi or another neuromorphic backend; the
[neuromorphic appendix](../appendices/neuromorphic.md) states the additional
operation, transport and completion obligations.
