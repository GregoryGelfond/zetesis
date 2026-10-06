# Programs, answers and queries

Use the `zetesis` crate to construct ASP programs, solve them and ask questions
about their answer sets. Its `Solver` implements the themelios backend contract;
`program`, `solve` and `query` expose the original themelios types under the
zetesis namespace. No rendered ASP or ASPIF crosses this boundary.

This API is available from `v0.3.0`. The [native session API](sessions.md)
remains available for GPU execution,
objectives, statistics and direct control of prepared input.

## One dependency

From an application beside a zetesis source checkout:

```toml
[dependencies]
zetesis = { path = "../zetesis/crates/zetesis" }
```

Record the checkout revision and keep the application's `Cargo.lock`. No separate
themelios dependency or local checkout is required.

## Construct, solve and read

The program chooses two tasks and rules out building and deploying together.
Its two answers therefore both include `run(test)`. The macro writes this
selection problem directly in ASP:

```rust
# extern crate zetesis;
{{#include ../examples/agent.rs:example}}
# mod execution {
{{#rustdoc_include ../examples/shared/agent.rs:hidden}}
# }
```

The constructors compose the same program from typed Rust values. This form
suits larger applications that generate facts, rules and directives from data:

```rust
# extern crate zetesis;
{{#include ../examples/agent-constructors.rs:example}}
# mod execution {
{{#rustdoc_include ../examples/shared/agent.rs:hidden}}
# }
```

Both construction forms call the following shared operation. It creates an
`Agent` with the native `Solver`, retains one complete snapshot, prints the
shown terms and asks whether every answer includes `run(test)`:

```rust
# extern crate zetesis;
mod execution {
{{#include ../examples/shared/agent.rs:example}}
}
```

Run either complete example with:

```sh
cargo run --locked -p zetesis --example book-agent
cargo run --locked -p zetesis --example book-agent-constructors
```

The shown sets contain `run(build), run(test)` and `run(deploy), run(test)`;
the example prints their Rust debug representation. Their order is unspecified.
Each full answer also contains the three `task`
facts. `Model::atoms()` returns those complete atoms; `Model::shown()` applies
`#show`. Queries always read the full answer set.

Both forms create the canonical `Program` accepted by the same engine. The
[preparation examples](source.md) also pair macros and constructors for direct
control of the native session API.

## Queries: Yes, No and Unknown

The query API deliberately adopts the **Gelfond–Kahl three-valued reading**:
Definition 2.2.2 with the authors' errata, as specified in the
[pinned themelios query design, §2.2](https://github.com/GregoryGelfond/themelios/blob/3339a8abed1e00f21e42c2b3ed2677c16248eda5/docs/design/query.md).

`Query::of` constructs a ground literal query. Within one answer set, a literal
is true if present, false if its strong contrary is present, and unknown
otherwise. `Query::all` forms a conjunction and `Query::any` a disjunction;
each is evaluated within that same answer set, taking respectively the minimum
or maximum under `false < unknown < true`. The complete family then determines
the result:

| Result | Meaning |
| --- | --- |
| `Yes` | The query is true in every answer set |
| `No` | The query is false in every answer set |
| `Unknown` | Neither condition holds |

For the family `{ {a}, {b} }`, `a ∨ b` is `Yes`, although neither disjunct holds
in every answer. `a ∧ b` is `Unknown`: neither answer contains a strong contrary.
Absence alone does not establish falsity.

`Agent::answer` solves and reads the complete family. `snapshot()` retains that
family for repeated queries without further solving. Both refuse an inconsistent
program or unfinished search with a typed fault; neither returns logical
`Unknown` to mean execution was incomplete. `Snapshot::entails` provides the
two-valued cautious reading: true exactly when the query answer is `Yes`.

## Brave and cautious consequences

`Agent::cautious()` returns the atoms present in **every** answer set, their
intersection. `Agent::brave()` returns those present in **some** answer set,
their union. Both use the full answer sets, including atoms hidden by `#show`.
In the example above, the cautious consequences are the three `task` facts and
`run(test)`; the brave consequences also contain `run(build)` and `run(deploy)`.

Each agent call enumerates to completion and folds the answers as they arrive.
The fold retains the accumulated consequence set and the current model. The
declared capability is `DerivedByEnumeration`. A program with no answer sets or an
unfinished search yields a typed fault. `Snapshot::cautious()` and
`Snapshot::brave()` reuse an already retained complete family without solving
again.

## Change the knowledge base

An agent retains the logical program between questions. Assertion returns a
handle that identifies the statement to retract:

```rust
# extern crate zetesis;
{{#include ../examples/reasoning.rs:example}}
```

This prints `unknown`, `yes`, then `unknown`. Each question is a new solve of
the updated knowledge base. An earlier snapshot remains a reading of its own
program; later assertions do not change it. The adapter currently rebuilds for
each question. It does not claim incremental ground-state reuse.

## Execution and limits

`Solver::new(Config)` selects grounding, worker count and resource allowances.
The default worker count comes from the host. This adapter currently executes on
the CPU; enabling GPU support elsewhere in a Cargo build does not change that.

| Grounder | Preparation |
| --- | --- |
| `Auto` | Existing adaptive formula preparation, including terminal reconstruction where applicable |
| `Eager` | Complete finite formula grounding |
| `Lazy` | Relational templates joined during candidate checking; outside that profile, formula producers materialized, eligible constraints instantiated during search and terminal definitions reconstructed per answer |

These are the CLI's `--grounder` values with the same mapping. Lazy grounding
joins a program inside the
[relational profile](source.md#admit-an-existing-logical-program) from source;
a program that profile refuses only for a construct it lacks takes the lazy
formula route instead. Every other refusal, such as a resource ceiling, is
returned; it is not retried under another grounding mode.

Use `Agent::solve_with` and `SolveOptions` for a time budget. The deadline covers
grounding, search and delivery, including time between reads. Enforcement is
cooperative and depends on timer scheduling. `Agent::interrupt()` provides cancellation for an in-flight
question; pulls before or after that question do not affect its successor.
Configured resource ceilings produce typed resource faults. They are distinct
from the request's time budget and from exhaustive search.

## The backend boundary

Direct backend consumers call `lower(Door::Program(&program))`, or construct
`Admitted` from a parse and use `Door::Parsed(&admitted)`. Both retain canonical
values and real provenance. `lower` validates and retains without grounding; a
refused replacement leaves the previous program available. Grounding belongs to
`solve`, and a failed grounding leaves the retained program available for
another request.

Only the parameterless `base` part is admitted. A named or parameterized part is
refused at `lower` with `Refused::Part`, retaining its exact key. This fault has
no diagnostic span because canonical parts have no source provenance. Statement
faults retain their statement and any real parsed location. `#script` is refused
as a program statement; zetesis executes no embedded script.

The solve contract enumerates full, unscored answer sets. It ignores objectives
and `#project`, while preserving `#show` as display selection. The CLI and native
session API retain their existing optimization and projection behavior.

Optimization, assumptions, registered Rust functions, theory propagators,
externals, a ground-program observer and incremental `#program`
operations are not yet declared by this adapter. Requests for them fail
explicitly. Program construction may represent more than a selected backend
can execute.

The contract's live solve handle stays on its calling thread. Native `Session`
transfer and the query layer's nonempty live `WorldView` are separate
contracts; see [completion and ownership](outcomes.md).
