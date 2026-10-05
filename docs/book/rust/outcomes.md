# Completion, resources, and output

Keep three questions separate: what has been proved about the program, how much
search has completed, and what an external consumer received.

This chapter uses the native `zetesis_solve::WorldView`: a complete original
family, possibly empty when the program is inconsistent. The facade's upstream
`zetesis::query::WorldView` is a nonempty live stream. Its materialized
`Snapshot` guarantees a complete, nonempty family. These types have distinct
contracts, as described in
the [library reference](libraries.md#api-reference).

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
bypasses staging to preserve prompt answers. `HumanRenderer` also flushes its
sink after the terminal summary so appended statistics cannot overtake buffered
answers. A failed human summary flush preserves semantic evidence but leaves
that summary unacknowledged. JSON and custom-renderer callers retain ownership
of flushing their supplied writers. Flushing does not establish durability.

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

`Interruption` implements `std::error::Error`. Error wrappers can retain it as a
typed cause; callers can downcast to `Interruption` and follow `source()` to the
underlying stop or resource error. Model-construction work and byte limits carry
their evidence directly and have no nested source. Display text is for people,
not a substitute for these typed distinctions.

JSON interruption `kind` and `code` classify the outcome for machine consumers.
`detail` is its human-readable explanation and may change as diagnostics improve;
consumers should not parse Rust debug spellings or infer completion from that text.
The public lazy source API reports exhausted rounds as `Stop::RoundLimit`,
separately from charged-work exhaustion. A stopped round sequence has not
established its final least consequence set.

Formula enumeration bounds retained projection history separately from authored
encoding. `max_projection_entries` counts distinct excluded keys;
`max_projection_nodes` counts logical trie nodes; `max_projection_bytes` admits
named capacity and its conservative growth overlap. The same fields are available
as advanced CLI options. `statistics.search.projection_history` reports entries,
nodes, retained/peak bytes and its subtotal of cumulative search work. A history
refusal retains any already checked answer prefix and reports incomplete coverage;
it does not turn a completed answer into a rejected candidate. Under
`--search regions` the history stays empty and
`statistics.search.candidate_regions` reports the regions visited, refuted and reached as
leaves, the propagations, atoms held and cut, whether the support cut
applied, and the reading work, a subtotal of cumulative search work, and
`statistics.search.reduct_query_regions` the regions of the reduct queries; under
`--search clauses` the former is `null` and the latter zero.

Under `--workers` above one the answer sets of the formula route
arrive in the schedule's order: the family is exact and each answer appears
once, but no order is promised, nor the same order between runs. Consumers
compare answer sets as sets, as the oracle comparison does.

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

`SemanticOutcome::objective_work()` records cumulative accepted objective
preparation and scoring work, including refused attempts, even before any
incumbent exists and after an observer failure. It is independent of answer
selection and optional timing collection. An incumbent's `Optimization::work`
mirrors this account. Candidate-bound construction has a separate work budget.
With statistics enabled, JSON reports this receipt as
`statistics.objective_work`; it is `null` when no typed semantic outcome survives.
A preparation attempt can consume a small objective-work allowance, so detailed
scoring fallback receives only the remaining allowance.

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
The example uses `WorldViewFailure::into_parts()` to move that prefix and its
typed cause, subject and outcome together. This transfers ownership without
cloning answers or strengthening the incomplete evidence. `into_answer_sets()`
instead deliberately discards the cause and coverage context.

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
The [world-view regressions](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-solve/tests/integration/world_views.rs)
cover additional search, scoring and storage failures. The
[source preparation example](source.md) separately checks empty and inconsistent
programs; neither an empty display nor a retained prefix decides inconsistency.

## Replace answer presentation

`AnswerRenderer` receives `AnswerView` callbacks one answer at a time, then a
borrowed `PublicationView`. Its optional `configuration(ConfigurationView)`
callback reports the selected backend, configured worker capacity and effective
grounding mode. The mode distinguishes eager, lazy, hybrid and an eager base
with host answer reconstruction. Selection does not establish completed work or
worker utilization. A later backend change can produce another callback. The
default emits nothing; a custom view decides whether to present these facts.
The controller evaluates `#show` in the themelios observation layer once per
yielded answer. Both `HumanRenderer` and
`JsonRenderer` consume that same typed `ModelView`: the complete interpretation,
selected original atoms, evaluated terms and optional priority/cost pairs remain
separate. An empty displayed projection never changes answer identity.

Library consumers can borrow evaluated `#show` terms with
`observation::Evaluation::symbols()` or take ownership with `into_symbols()`.
The latter moves the existing vector without copying terms or allocating new
storage. Read `statistics()` first if the evaluation's work receipt is needed.

`publish_prepared` accepts an admitted owner and `PublicationConfig`, containing
ordinary `SolveConfig` and observation limits. It needs no argument parser,
global stream or complete `WorldView` buffer. `run_with_renderer` and
`run_bundle_with_renderer` are source conveniences using the existing CLI options;
their `json` flag does not replace the injected renderer. The prepared entry
suppresses routine execution messages; warning diagnostics and typed failures
remain available.

`needs_stage_timings()` defaults to `false`. A custom renderer that needs coarse
host stages overrides it; a wrapper delegates the preference of the view it
wraps. The controller fixes this choice after `begin` succeeds. `HumanRenderer`
requests coarse stages for its default timing line without enabling detailed
phase clocks. `JsonRenderer` requests none; its optional statistics object may
be `null`, and any represented timing fields remain `null`, unless detailed
measurements are enabled through `SolveConfig::stats` or source options.
Renderer selection, including this preference, is independent of `Options::json`.
Available snapshots reach `PublicationView::phase_timings()`.
Configuration callbacks are measured as observation/output work, not solving.

The controller acknowledges a complete answer only after its renderer returns
success. A renderer can write into a supplied sink or accept typed data directly.
Failure after a partial write acknowledges no answer. Cooperative cancellation
remains a publication stop, independently of any established exhaustion or
optimum; a later writer failure remains a failure. Neither an accepted callback
nor `SummaryDelivery::Accepted` implies durability. The human renderer flushes
its terminal summary before returning `Accepted`; a flush error remains a typed
output failure. Other renderers leave flushing to their callers unless their
own contract states otherwise. The process still performs its final explicit
flush, and a successful later flush cannot clear an earlier publication failure.

The human renderer preflights one complete record under its byte ceiling. The
JSON renderer additionally retains a bounded document atom table; each invocation
starts fresh indices. Structural equality decides each atom's index, so equal
atoms of independent owners share one; once a canonical atom has been found, the
table answers its later occurrences from the same owner by owner-scoped identity
instead of hashing the atom's structure. Identities are kept for the catalog of
the record being encoded: a record from another catalog, such as each answer
with reconstructed terminal definitions, starts the identity cache afresh, so
its cost does not grow with the number of answers. A refused record withdraws
the atoms it entered together with their identities. Custom renderers own their encoding
limits and any copies they retain. The controller retains no complete family
for presentation, although objective selection still uses the solver's
separately bounded incumbent store.

`SummaryStage::SearchFinished` preserves the human result and compact timing
summary before statistics are written. The command appends optional human tables
on stderr. That view cannot claim later reporting succeeded, and the callback is
not reached after a source or execution failure. The default `Finalized` stage,
used by JSON and custom renderers, includes attempted timings and later reporting
failures. The controller invokes the chosen terminal callback at most once; a
failed footer never replaces an earlier original cause.

This consumer inspects full and shown channels while retaining only two scalar
fields. It deliberately accepts typed records without producing text:

```rust
# extern crate zetesis_cli;
# extern crate zetesis_cpu;
# extern crate zetesis_themelios;
{{#include ../examples/answer-renderer.rs:example}}
```

Run the checked example with:

```sh
cargo run --locked -p zetesis-cli --no-default-features --example book-answer-renderer
```

See the [view contract](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-cli/src/view.rs),
[publication controller](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-cli/src/publication.rs)
and [bounded model views](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/observation/view.rs).

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

The [streaming regression](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-solve/tests/integration/streaming_defaults.rs)
checks every full answer for independent selections on a 24-vertex path against
an independently generated bitmask family, using both automatic specialization
and the general countermodel oracle. The former already completes within the
previous search-work allowance; the latter needs more work and still interrupts
when explicitly given that earlier ceiling. The test releases each answer after
checking its identity; it does not collect a `WorldView` or bypass that type's
retention limits. This demonstrates completed enumeration within the ordinary
allowances, not a running-time guarantee or a change to answer-set semantics.

### Cancellation and deadlines

[`zetesis_cpu::Cancellation`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-cpu/src/cancellation.rs)
carries shared cancellation and an optional absolute deadline. `Session` owns
solving; callers pass this value to the session and can cancel it through a clone.
`Cancellation::default()` sets no deadline. `with_deadline(Instant)` can fail if
the timer thread cannot be started; an already expired deadline starts no thread.
Clones observe the same flags, and `poll()` reports `Stop::Cancelled` before
`Stop::Deadline` when both apply.

Cancellation is observed at cooperative polling boundaries, not by forcibly
terminating arbitrary work. A future deadline owns a timer that sets an expiry
flag and retires then or when the last clone drops. Polling reads shared flags
without reading the clock. An already-started bounded static compilation is not
preemptible; session setup polls before it and subsequent work polls again. A
deadline is therefore not a hard process-kill guarantee.

For a reusable interrupt handle across separate runs, `CancellationSlot::open`
returns a `CancellationRun` guard. Keep the guard until that run ends and clone
its `cancellation()` token for preparation or workers. Each opening gets a fresh
identity. Opening again, clearing the slot or dropping the guard stops the old
tokens; an old guard cannot stop a newer run. Cancelling an idle slot has no
effect on a later run.

A slot cancellation makes at most one atomic update, without allocating or
waiting. Tokens from a slot add one atomic membership read to ordinary polling.
Deadlines retain their existing timer and retirement costs. Slot identities never
wrap: exhausting them returns `CancellationSlotError::GenerationExhausted`
without changing the current run.

For Rust consumers migrating from the previous API, `Cancellation` replaces
`Control`, and operations named `with_control` or ending in `_with_control` now
use `with_cancellation` or `_with_cancellation`. Update imports, named fields and
calls; no compatibility alias is retained. The cancellation/deadline behavior is
unchanged.
Typed interruption variants named `Control` that wrap broader stop reasons,
and existing serialized control codes, keep their own meaning.

The installed CLI maps `--time-limit DURATION` to this `Cancellation`.
The duration accepts nonnegative whole seconds, or a whole number with `s`, `m`
or `h`, measured from completion of input loading. Zero requests an immediate
stop; omission sets no deadline.
`--stats` reports the requested process duration. A deadline during search leaves
its coverage incomplete. A later deadline during publication preserves already
established coverage. Either stop exits with code 3, in both human and JSON output.
Input, device and output errors remain distinct failures. Source loading,
frontend operations, blocking output and a running device kernel are not
preempted. Rust library consumers construct and supply their own `Cancellation`;
the process option does not replace an explicitly supplied library value.

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
Observation stops also retain their located observation evidence. A cooperative
stop does not itself invalidate the writer; the adapter attempts an incomplete
footer, whose write can independently fail. JSON keeps semantic
`completion` and `optimization.optimal` unchanged and adds `publication_stop`;
its overall `status` is `incomplete`. For example, cancellation between proved
optimal ties can leave `completion: "exhausted"`, `optimal: true`, and fewer
published ties. It does not establish complete delivery.

The process returns exit 3 for a cooperative publication stop and exit 2 for a
writer failure. The finalized API retains a preceding publication stop separately.
A subsequent process flush failure returns exit 2 and emits a flush diagnostic;
that process boundary has already mapped the publication outcome to an exit code.
A partial record is never counted. Encoding/resource refusals remain actual
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

The CLI's `--json` output is a versioned view. The document spells each atom
once, in the record that first holds it; a record refers to its full model and
its shown atoms by index into the document's atom table, and shown terms and
costs stay per record. Human output and JSON do not
define different solving modes. Applications should consume typed library
values or the JSON contract rather than parse styled answer lines.

The default human summary reports the selected configuration and coarse elapsed
time. Eager grounding and solving have separate intervals; lazy source work
occurs within membership and is reported together with solving. Human `--stats`
adds nonitalic tables after this summary, reporting requested policy separately
from observed execution and distinguishing unavailable measurements from zero.
Host intervals around device calls include transport, waits and readback.
Enabled session elapsed time can include the consumer's delay between pulls;
active solving spans do not include that delay. These scopes matter when using
the same library in a server or comparing it with a command-line run.

The compact human work table projects retained accounting without collecting
extra counters. Its formula region reading work is a search-work subtotal;
closure work covers completed checks and omits stopped-check partial work.
GPU submissions do not imply a decoded result, and only decoded primitive work
is shown. Eager rule/table rows cover rule instantiation. The typed statistics,
JSON and compatibility record views retain the complete counter catalog.

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

Under `--search clauses`, formula search records its necessary
disjunctive support attempt separately from user or objective refinements.
`statistics.search.necessary_support` identifies application, an inapplicable
head grammar, or a configured construction/encoding shape limit; under
`--search regions` it is `null`, and `statistics.search.regions` says
whether the support cut applied. Its construction and encoding work are included in search work,
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
