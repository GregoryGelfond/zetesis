# Completion, resources, and output

Keep three questions separate: what has been proved about the program, how much
search has completed, and what an external consumer received.

| Evidence | What it establishes | What it does not establish |
| --- | --- | --- |
| Verified model | Completed membership for the original subject | Exhaustive enumeration or optimality |
| Evaluated score | Cost of that verified model | A globally best cost |
| Exhausted search | Relevant candidate coverage completed | Successful external publication |
| Proved optimum | Complete relevant search establishes the incumbent | Every optimal tie was delivered |
| `WorldView` | All full answers of the original subject were captured after unrestricted exhaustion | Successful external publication |
| Published record | The sink accepted one whole record | Flush, durability or complete search |

The CLI owns its standard-output buffer and explicitly flushes it before
returning an exit code, including for `devices`. A failed flush returns exit 2;
any original failure and later output failures remain separate diagnostics.
Redirected output has a fixed 8 KiB staging buffer, while terminal output
bypasses staging to preserve prompt answers. Library callers retain ownership
of flushing and durability for their supplied writers.

`SemanticOutcome::unsatisfiable()` requires exhausted coverage and zero verified
models. A zero display count, an empty consumer vector, or `completion() == None`
cannot establish inconsistency. A requested model limit and an interrupted
search are distinct completion states. If membership execution ends without
an exhausted candidate stream, `SolveError::CandidateStreamNotExhausted` retains
the checked prefix and leaves coverage unestablished. This check is also active
in release builds.

`SemanticOutcome::search_state()` carries the status and its reason together.
`SearchState::PendingInterruption(reason)` records a known batch stop while
previously checked answers remain to be consumed. After that prefix is drained,
`Interrupted(reason)` is final. The compatibility `completion()` and
`interruption()` accessors derive from this state; a pending interruption has
no final completion classification. An absent state makes no claim about why
the caller has not obtained a stopping classification.

`Interruption::Preparation` identifies cancellation or an expired deadline
observed before candidate checking starts. It does not attribute work to a
closure oracle, countermodel search or device that was never entered. JSON
uses the interruption kind `preparation`, with `cancelled` or `deadline` as
its control code.

`SemanticOutcome::selection()` identifies the family requested by the session.
`All` ranges over the original program; `Optimal` permits sound exclusion of
worse candidates. `Completion::Exhausted` therefore does not by itself identify
the unrestricted family, and an optimum proof does not prove that a consumer
retained every tie. `WorldView` owns both unrestricted enumeration and complete
capture; callers cannot attach an arbitrary vector to this evidence.

Objective presence is also semantic data. An absent objective is not an active
objective whose cost happens to be zero. Preserve the priority structure and
optional score in typed results and comparisons; flattening both cases to an
empty or zero vector can conceal a disagreement.

Unrestricted enumeration evaluates scores without retaining incumbents. Its
`scored_models()` can be positive while `incumbent()` is absent and
`retained_models()` is zero. Those fields describe objective-search retention,
not the number of answers in a `WorldView` or a consumer's own collection.

## Check selection and complete capture

The next example uses `1{a;b}1.` with costs `1` for `{a}` and `2` for `{b}` at
priority `2`. Its `#show.` declaration hides both atoms from display. Every
library answer still contains its complete interpretation: two visually empty
answers are not one answer set.

| Operation | Requested family | Completed result in this example |
| --- | --- | --- |
| `Session::new` | Objective-selected answers | `{a}` at cost `1`; optimum proved |
| `Session::enumerate` | All original answers | `{a}` at cost `1` and `{b}` at cost `2` |
| `WorldView::collect` | All original answers, completely retained | The same two scored answers |
| Collection limited to one answer | All original answers | Typed failure retaining one checked answer |

Each operation starts a fresh session over the same admitted owner. The example
checks exact full interpretations and priority/cost pairs without depending on
enumeration order. It pins CPU execution and one-at-a-time batches to make the
small failure-accounting example easy to inspect.

```rust
# extern crate zetesis_solve;
# extern crate zetesis_core;
# extern crate zetesis_cpu;
# extern crate zetesis_themelios;
{{#include ../examples/selection.rs:example}}
```

Run it from the checkout root with:

```sh
cargo run --locked -p zetesis-solve --no-default-features --example book-selection
```

The failed collection has checked two models but retained only one. The second
model was already verified when the collection refused its storage, and the
iterator has not completed the final exhaustion step. Thus the failure carries
`verified_models() == 2` and `completion() == None`; its checked prefix cannot be
relabelled a complete one-answer world view. Larger batches can also retain
queued membership results in verification accounting before those answers are
scored or yielded. The retained answer's subject remains the original owner.

`Session::new` with an objective must finish its search phase before yielding
retained incumbents. Streaming all answers can release each answer after use;
complete collection retains the full family. With `K` answers and full payload
sizes `s₁, …, sₖ`, collection requires space proportional to their sum in addition
to the search engine and shared subject. `K` can be exponential in the atom
carrier. Objective selection retains incumbent ties under its separate limits;
the one optimum in this fixture does not bound the number of ties in general.

See the implementation contracts in
[`Session`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-solve/src/session.rs),
[`SemanticOutcome`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-solve/src/semantic_outcome.rs)
and [`WorldView::collect`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-solve/src/world_view.rs).
The [world-view regressions](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-solve/tests/world_views.rs)
cover additional search, scoring and storage failures. The
[source preparation example](source.md) separately checks empty and inconsistent
programs; neither an empty display nor a retained prefix decides inconsistency.

## Resource contracts

Each operation names the resources it bounds: source bytes, syntax depth, atoms,
substitutions, nodes, work, candidate occurrences, retained results or transport
storage. Zero is a real ceiling where those limits apply; it does not mean
unlimited. Work and storage limits are independent.

Ordinary sessions and CLI flags share `SolveConfig::DEFAULT`: ten billion
cumulative search-work units, ten million branch decisions, ten million
candidates and one hundred million CPU oracle-work units under that field's
per-candidate or shared-batch contract. These are finite logical allowances,
not wall-clock deadlines or performance claims. Standalone primitive limit
types keep their own operation-specific defaults. Source admission, derived
atoms, carrier storage, batch bytes, objective retention and complete `WorldView`
capture remain independently bounded; requesting every answer does not make
those resources unlimited. A smaller explicit work, decision or candidate
allowance still yields an interrupted prefix when exhausted.

The [streaming regression](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-solve/tests/streaming_defaults.rs)
checks every full answer for independent selections on a 24-vertex path against
an independently generated bitmask family, using both automatic specialization
and the general countermodel oracle. The former already completes within the
previous search-work allowance; the latter needs more work and still interrupts
when explicitly given that earlier ceiling. The test releases each answer after
checking its identity; it does not collect a `WorldView` or bypass that type's
retention limits. This demonstrates completed enumeration within the ordinary
allowances, not a running-time guarantee or a change to answer-set semantics.

`Control` is cloneable shared cancellation with an optional absolute deadline.
Cancellation is observed at cooperative polling boundaries, not by forcibly
terminating arbitrary work. In particular, an already-started bounded static
compilation is not preemptible; session setup polls before it and subsequent
work polls again. A deadline is therefore not a hard process-kill guarantee.

The installed CLI maps `--time-limit SECONDS` to this existing `Control`.
The duration is a nonnegative whole number of seconds, measured from completion
of input loading. Zero requests an immediate stop; omission sets no deadline.
`--stats` reports the requested process duration. A deadline during search leaves
its coverage incomplete. A later deadline during publication preserves already
established coverage. Either stop exits with code 3, in both human and JSON output.
Input, device and output errors remain distinct failures. Source loading,
frontend operations, blocking output and a running device kernel are not
preempted. Rust library consumers construct and supply their own `Control`;
the process option does not replace an explicitly supplied library control.

Admission limits also do not retroactively bound vectors constructed by the
caller. A documented logical payload budget excludes what its contract names,
such as allocator bookkeeping or worker threads. It must not be reported as
process RSS. Read the operation's own limit type rather than assuming one global
memory bound covers the entire solver.

## Views and failures

Use `Session` when the consumer wants semantic values and owns its presentation.
Use `zetesis_cli::run_finalized` when it wants the ordinary source driver and an
injected output sink. Its finalized result is `Result<PublicationOutcome,
PublicationFailure>`. `PublicationOutcome::Completed` carries a
`PublicationReport`, separating semantic evidence from
`Publication`. A writer failure can coexist with already established exhaustion;
it cannot retract that proof, and it cannot claim a partially written record as
fully published. `PublicationFailure` preserves the original cause and available
semantic, publication and timing evidence. The solver library's `SolveFailure`
retains only execution cause, subject, semantics and timings. Its owned
decomposition allows a consumer to preserve those values in its own failure
representation without constructing new semantic evidence.

`PublicationOutcome::Stopped` carries the original control reason, its
publication phase, the semantic snapshot, and complete record/summary counts.
Observation stops also retain their located observation evidence. The writer
remains usable, so the adapter writes an incomplete footer. JSON keeps semantic
`completion` and `optimization.optimal` unchanged and adds `publication_stop`;
its overall `status` is `incomplete`. For example, cancellation between proved
optimal ties can leave `completion: "exhausted"`, `optimal: true`, and fewer
published ties. It does not establish complete delivery.

The process returns exit 3 for a cooperative publication stop. A writer or flush
failure returns exit 2, retaining a preceding publication stop separately. A
partial record is never counted. Encoding/resource refusals remain actual
failures, distinct from a cancellation or deadline.

The finalized API now returns the outcome enum. Match `Completed` or `Stopped`,
or call `semantic()` and `publication()` for their common evidence. `report()`
returns an optional legacy report. `PublicationOutcome::into_legacy()` is the
explicit adapter used by `run` and `run_detailed`: it maps a cooperative stop to
`RunError::PublicationStopped` while preserving semantic and partial-report
metadata. Those convenience APIs retain their original return shapes.

The driver retains one semantic snapshot and separate publication and timing
state. Legacy reports, statistics and JSON are derived views; their fields are
not copied back into the session's evidence. A successful legacy `Report`
requires an established completion classification. Requesting it before that
point is a typed driver protocol failure. Partial views retain absent completion
without substituting exhaustion or a logical interruption. Mutating a detached
compatibility report cannot change the retained semantic outcome.

The CLI's `--json` output is a versioned view. Full semantic atoms, shown atom
indices, shown terms and costs remain separate. Human output and JSON do not
define different solving modes. Applications should consume typed library
values or the JSON contract rather than parse styled answer lines.

`--stats` reports requested policy separately from observed execution and
distinguishes unavailable measurements from zero. Eager grounding and solving
can be measured as separate stages; lazy source work occurs within membership.
Host intervals around device calls include transport, waits and readback.
Enabled session elapsed time can include the consumer's delay between pulls;
active solving spans do not include that delay. These scopes matter when using
the same library in a server or comparing it with a command-line run.

Closure sessions expose `SemanticOutcome::candidate_statistics()` separately
from membership results. These counters report necessary source restrictions,
their preparation and traversal work, and skipped impossible binary intervals.
An interval can contain several seeds; it is not an examined-candidate count.
Prepared and peak copied payload bytes exclude allocator and index overhead and
are not process memory. The peak includes temporary restriction templates.
Failed preparation retains its work but has no completed preparation footprint.
The CLI exposes the same fields as `statistics.candidate_restrictions` in JSON.
`--max-candidate-bytes` bounds copied payload; `--max-search-work` bounds cumulative
restriction work on this route. Both advanced controls appear in `--help-all`.

Formula search records its necessary disjunctive support attempt separately from
user or objective refinements. `statistics.search.necessary_support` identifies
application, an inapplicable head grammar, or a configured construction/encoding
shape limit. Its construction and encoding work are included in search work,
including a rolled-back encoding. A shape refusal retains general search;
cancellation, work exhaustion and allocation failure remain explicit stops.
Every proposed interpretation still needs membership checking against the
original theory and its frozen reduct.

`CompletionAccounting` reports requested workers and the greatest concurrency
selected at preflight. This selection alone does not mean a candidate entered
execution. `requested_scratch_bytes` records the greatest minimum envelope;
`peak_scratch_bytes` records retained query-vector and map entry capacities plus
the conservative transient/result allowance. Reservation can exceed the minimum request.
If the resulting capacity exceeds `max_completion_scratch_bytes`, the attempt
refuses before candidate work and can report a peak above the ceiling with zero
entered slots. Both byte counters exclude allocator and hash-table control
overhead, thread stacks, the original theory, the candidate cursor and GPU
storage. Neither counter measures RSS. These fields are preserved in failed
attempts and in the CLI's JSON completion statistics.
