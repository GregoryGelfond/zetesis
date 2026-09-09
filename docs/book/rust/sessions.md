# Embedding an ordinary solve

A session owns search state and borrows a coherent admitted input. It accepts
semantic configuration and control, with no argument parsing, standard streams
or answer rendering. This example enumerates the guided tour's complete family:

```rust
# extern crate zetesis_cli;
# extern crate zetesis_core;
# extern crate zetesis_cpu;
# extern crate zetesis_themelios;
{{#include ../examples/session.rs:example}}
```

`admit` is the strict normal-profile door used by this example. Use
`admit_extended` for its supported scalar extensions, or `admit_formula` for the
broader finite formula profile. These are explicit library choices. The ordinary
source driver provides automatic profile selection; `Session` receives an
already prepared representation and does not retry admission itself.

## Pulling and stopping

`Session` is a fused iterator of `Result<AnswerSet, SolveFailure>`. Each
privately constructed `AnswerSet` retains its original `Subject`, complete interpretation and optional
fully evaluated score. Hidden atoms remain present even when `#show` would omit
them from human output. `SessionModel` remains a compatibility alias. These are
completed native membership results, not Lean proof objects.

`Session::enumerate` streams all answers of the original program, including
nonoptimal answers. It evaluates objective scores but disables both candidate
pruning and incumbent selection. `Session::new` retains the ordinary behavior:
an active objective selects incumbent ties; without an objective it enumerates
the original family. `SemanticOutcome::selection()` records `All` or `Optimal`
independently of whether that search finished.

`models: 0` requests exhaustive enumeration. In unrestricted enumeration, a positive model
limit can terminate successfully at that limit without exhausting the search.
Dropping the iterator or calling `stop()` early does not imply that unseen
candidates have been checked. Inspect `SemanticOutcome::completion()`; it is
optional precisely because a consumer may stop before a terminal classification
exists.

For `Session::new` with an objective, the search phase ends before retained incumbents are yielded.
Exhausted search establishes optimum ties; an interrupted phase may yield
unproved incumbents. A score alone is not an optimum certificate. Use the
outcome's `optimum_proved()` and interruption information to distinguish proved
ties from an interrupted incumbent. Yielded tie limits also affect how much of
the selected family a consumer receives.

## Collecting the original world view

`WorldView::collect(input, config, limits, control)` owns a fresh unrestricted
session and returns its complete original answer-set family only after
exhaustion. Use `models: 0`; a positive model limit is honored and can prevent
completion. Every member retains its full interpretation and original subject.
Objectives annotate the members with scores without removing nonoptimal answers.

An inconsistent program has an empty `WorldView`. An empty program has a world
view containing one empty `AnswerSet`. Thus `WorldView::is_empty()` establishes
inconsistency, while an empty vector collected from a stopped stream does not.
An arbitrary vector and a detached outcome cannot construct a `WorldView`.

`WorldViewLimits` independently bounds retained answers, summed full atoms and
canonical payload bytes, including score priorities. The byte bound excludes
shared subjects, allocator overhead, engine state and the one answer being
considered for collection. It is not a process-memory limit. Members are moved
into the collection without a repeated membership check or full-model clone;
retained space can still be exponential in program size.

`WorldViewFailure` preserves the checked prefix, original subject, typed cause
and available semantic outcome. Search interruption, scoring failure and
collection limits cannot produce a complete value. A model whose score or
storage admission failed can remain in verified accounting without appearing in
the retained prefix. No partial result is treated as a smaller program.

This contract deliberately differs from the September 2026 themelios solve/query
design drafts, which describe a nonempty lazy `WorldView` and select optimal
answers under objectives. Zetesis uses the original unrestricted family and
an opt-in complete collection, including the empty inconsistent family. The
streaming `Session` remains the surface for partial observations. These types
do not implement a themelios-solve backend or imply compatibility with a future
themelios-query runtime.

## Reuse and identity

The admitted owner can outlive several sessions. Each session starts fresh
search budgets, worker pools, pending queues and incumbent storage. Reusing an
owner does not resume a previous search.

`AnswerSet::subject()` retains the checked `Program` or `Theory`.
`Subject::same_instance` distinguishes shared owners from independently admitted but
structurally equal inputs. `into_interpretation()` explicitly discards the
subject association for raw-model interoperability; use it only when your own
boundary can maintain the required context.

The example uses a tiny family to show streaming and bounded complete collection.
A production consumer can process each answer as it arrives; full materialization
is an explicit choice.
Neither collection nor successful iteration acknowledges delivery to an external
sink; [publication is a separate boundary](outcomes.md).
