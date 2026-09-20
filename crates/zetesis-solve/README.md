# zetesis-solve

Composed answer-set solving over coherent admitted programs. This library owns
execution policy, sessions, checked answer sets, complete world views and typed
failure evidence. It has no command-line parser, source-file loader, JSON encoder
or output sink. The `zetesis` command is a consumer through `zetesis-cli`.

## Solve an admitted program

`PreparedInput` borrows an admitted source owner, native relational program or
complete ground graph. `Session::builder` composes policy, optional device
resources and measurements before execution. `start` returns a streaming session;
`collect` retains the complete original answer-set family within explicit limits.

```rust
use zetesis_solve::{Backend, PreparedInput, Session, SolveConfig, WorldViewLimits};
use zetesis_cpu::Control;
use zetesis_themelios::{admit, AdmissionOptions};

let admitted = admit("a :- not b. b :- not a.".into(), AdmissionOptions::default())?;
let config = SolveConfig { backend: Backend::Cpu, models: 0, ..Default::default() };
let world_view = Session::builder(
    PreparedInput::admitted(&admitted), config, Control::default(),
).collect(WorldViewLimits::default())?;
assert_eq!(world_view.len(), 2);
# Ok::<(), Box<dyn std::error::Error>>(())
```

Collection consumes an unstarted request and selects all original answers,
including nonoptimal answers when objectives are present. A limit, interruption
or failed observer returns a retained prefix rather than a `WorldView`. A complete
empty family establishes inconsistency; a family containing one empty answer
does not. Streaming consumers choose when to retain or publish each answer.

## Execution and evidence

The reduct defines answer-set membership. Relational normal programs use exact
reduct closure; general formulas require original satisfaction and the absence
of a proper-subset model of the frozen reduct. CPU, Rayon and wgpu execution
preserve this contract. Class certificates justify specializations only under
their checked premises.

Automatic CPU formula execution checks those premises against the complete ground
theory. Positive atomic-head programs use least consequences and original
constraint satisfaction; tight programs can use supportedness. Full normalized
source analysis guides the order of these checks. It does not certify a ground
theory by itself. Explicit countermodel checking retains the general reduct path.

General formula execution prepares one candidate-parametric reduct encoding per
search owner and reuses it across residual queries. Rayon workers share the
immutable encoding and retain separate query state. Preparation, original
satisfaction and query work remain explicitly bounded and accounted; reuse does
not carry candidate truth into another check.

With GPU support, `ExecutionResources::with_gpu` shares a selected context and
`with_formula_profile` additionally shares one exact compiled formula primitive.
Each session retains its own program preparation, residency, candidate stream,
budgets, counters and incumbents. Shared infrastructure does not share truth or
resume a previous search. Adapter policy, device health and contention are
checked when execution uses the resource.

Independent relational CPU sessions retain one exact-program query preparation
and reuse empty workspaces across submitted batches. `max_source_work` bounds
preparation; candidate work remains separately bounded by `max_work`.
`SemanticOutcome::query_execution()` retains the CPU producer's actual ownership
receipt and any snapshot fault, and `SemanticOutcome::closure_execution()` sums
the counters of every completed check on the independent lazy and eager routes.
Preparation and candidate stops remain distinct, and previously checked answers
remain valid. Reused capacity is neither shared candidate truth nor a
performance guarantee.

`AnswerSet` retains its original subject and optional objective score.
`SemanticOutcome` records verified membership and search coverage independently
of consumer output. `Session::progress` snapshots current evidence without
stopping search. `SolveFailure` retains the cause, subject, available semantics
and optional timings; it contains no publication state.

Retained-answer limits count each actual shared atom-catalog owner once, each
answer's selection separately, and retained objective scores. Equal catalogs
with distinct owners remain distinct charges. These canonical payload limits
bound represented data, not allocator overhead or process memory.

`ExecutionObserver` receives typed synchronous execution facts. An observer
failure terminates its operation and preserves existing evidence; it cannot
trigger device fallback. `SolveMeasurements` lets admission, execution and
publication contribute to an explicit host-measurement scope. Measurements are
not semantic evidence or GPU kernel timings.

See the [library map](../../docs/book/rust/libraries.md),
[session guide](../../docs/book/rust/sessions.md),
[observation and measurement guide](../../docs/book/rust/measurements.md),
[outcome contracts](../../docs/book/rust/outcomes.md) and
[implementation correspondence](../../docs/book/lean/correspondence.md).
