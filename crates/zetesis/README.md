# zetesis

The public facade exposes the canonical ASP values and construction macros,
engine-independent solving contracts, and query readings under one dependency.
`program`, `solve`, `query`, `syntax`, `base` and `analysis` re-export the pinned
themelios crates. `Program` and `Symbol` are also available at the crate root;
`prelude` combines the canonical working vocabularies. `Solver` supplies the
native CPU backend; `Agent` manages the knowledge base and the query API reads
its answer sets. GPU execution and native objective/statistics controls remain
available through `zetesis_solve::Session`.

```rust
use zetesis::{Solver, program, query::AgentReading, solve::agent::Agent};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut agent = Agent::new(program! { p(1). q(X) :- p(X). }, Solver::default());
    let snapshot = agent.snapshot()?;
    for answer in snapshot.members() {
        println!("{:?}", answer.atoms());
    }
    Ok(())
}
```

The nine macros build the same owned values as the canonical constructors and
work when this is the consumer's only application dependency, including under a
Cargo rename. Parsing occurs during compilation; generated constructors retain
their allocation and canonicalization costs at runtime. The facade adds no
program representation. The backend validates canonical input before grounding.

The [library guide](https://gregorygelfond.github.io/zetesis/book/rust/agent.html)
contains runnable examples for enumeration, queries, assertion and retraction.
The adapter enumerates full answer sets, ignoring objectives and `#project`;
`#show` selects only the display. Resource failures, cancellation and time limits
remain distinct from complete search. The first adapter supports CPU execution
and the existing grounding profiles; advanced upstream capabilities remain
explicitly undeclared.

`program!` supports rules, including choices and aggregates, optimization,
`#show` and `#external`. Other directive families use typed constructors or the
source parser. Successful construction does not establish engine support.
rust-analyzer may produce a different reading or an error for forwarded ASP
punctuation such as `:-`; rustc preserves the macro tokens. The
[canonical macro documentation](https://github.com/GregoryGelfond/themelios/blob/3339a8abed1e00f21e42c2b3ed2677c16248eda5/docs/design/macros.md)
records the editor limitation and remaining token-dialect limits.

The nested consumer fixture tests all nine macros through the renamed dependency
`z`, canonical value identity, counted elements, and consumer diagnostic spans.
Its only application dependency is this facade; `trybuild` is a test instrument.
