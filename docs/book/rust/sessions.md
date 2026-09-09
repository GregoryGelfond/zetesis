# Embedding an ordinary solve

A session owns search state and borrows a coherent admitted input. It accepts
semantic configuration and control, with no argument parsing, standard streams
or answer rendering. This example enumerates the guided tour's complete family:

```rust
{{#include ../examples/session.rs:example}}
```

`admit` is the strict normal-profile door used by this example. Use
`admit_extended` for its supported scalar extensions, or `admit_formula` for the
broader finite formula profile. These are explicit library choices. The ordinary
source driver provides automatic profile selection; `Session` receives an
already prepared representation and does not retry admission itself.

## Pulling and stopping

`Session` is a fused iterator of `Result<SessionModel, SolveFailure>`. Each
`SessionModel` retains its original `Subject`, complete interpretation and optional
fully evaluated score. Hidden atoms remain present even when `#show` would omit
them from human output.

`models: 0` requests exhaustive enumeration. With no objective, a positive model
limit can terminate successfully at that limit without exhausting the search.
Dropping the iterator or calling `stop()` early does not imply that unseen
candidates have been checked. Inspect `SemanticOutcome::completion()`; it is
optional precisely because a consumer may stop before a terminal classification
exists.

With an objective, the session completes the relevant search before yielding
retained optimum ties. A score alone is not an optimum certificate. Use the
outcome's `optimum_proved()` and interruption information to distinguish proved
ties from an interrupted incumbent. Yielded tie limits also affect how much of
the selected family a consumer receives.

## Reuse and identity

The admitted owner can outlive several sessions. Each session starts fresh
search budgets, worker pools, pending queues and incumbent storage. Reusing an
owner does not resume a previous search.

`SessionModel::subject()` retains the checked `Program` or `Theory`.
`same_instance` distinguishes shared owners from independently admitted but
structurally equal inputs. `into_interpretation()` explicitly discards the
subject association for raw-model interoperability; use it only when your own
boundary can maintain the required context.

The example deliberately collects just two tiny results. A production consumer
can process each model as it arrives rather than retain an unbounded vector.
Neither collection nor successful iteration acknowledges delivery to an external
sink; [publication is a separate boundary](outcomes.md).

