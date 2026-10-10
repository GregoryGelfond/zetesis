# Getting started with the library

For the single-dependency API, start with
[programs, answers and queries](agent.md). It combines program construction,
the native solver and themelios's agent/query APIs under `zetesis`.
The session example below uses the native session API.

The complete [solve example](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-solve/examples/solve.rs)
prepares an ASP program, streams typed answer sets on the CPU, displays them and
checks that enumeration completed. Run it from a zetesis checkout:

```sh
cargo run --locked -p zetesis-solve --example solve --no-default-features
```

The program chooses two of three tasks. Building and deploying together is
forbidden, so its complete family contains two answers: build with test, and
deploy with test. Each full interpretation also contains the three `task`
facts. `#show run/1` changes only the displayed channel, not those typed answers.

## Set up a Rust application

The packages are not published on crates.io. These examples require zetesis
0.5.0. Use paths in a pinned local checkout of that version. From the directory
containing your `zetesis` checkout, record its revision and create a sibling
application:

```sh
git -C zetesis rev-parse HEAD
rustup toolchain install 1.97.1 --profile minimal
cargo new answer-set-app
cd answer-set-app
rustup override set 1.97.1
```

Keep a clean checkout of the full revision printed by `rev-parse`; this fixes
the source used by all three path dependencies. To reproduce the setup
elsewhere, check out that same revision. The first Cargo build also fetches the
repository's pinned themelios dependency; no separate themelios checkout or
direct parser dependency is needed.

Replace the application's empty `[dependencies]` section with:

```toml
[dependencies]
zetesis-solve = { path = "../zetesis/crates/zetesis-solve", default-features = false }
zetesis-themelios = { path = "../zetesis/crates/zetesis-themelios" }
zetesis-cpu = { path = "../zetesis/crates/zetesis-cpu" }
```

This selects a CPU-only solver build. Without `default-features = false`, GPU
support is compiled but a GPU is not selected automatically. Cargo feature
unification allows another dependency to enable that support in the same build.
Keep the application's generated `Cargo.lock` to pin its resolved dependencies.

## Complete program

Put this code in `src/main.rs` and run `cargo run`. The book includes the actual
maintained example below; there is no separate abbreviated implementation.

```rust
# extern crate zetesis_solve;
# extern crate zetesis_cpu;
# extern crate zetesis_themelios;
{{#include ../../../crates/zetesis-solve/examples/solve.rs:example}}
```

For each answer, `Full answer` contains five atoms: the three task facts and two
chosen `run` atoms. The two `#show` lines are, in either enumeration order:

```text
#show: run(build) run(test)
#show: run(deploy) run(test)
```

Only after exhaustion does the example print `Complete family: 2 answer sets.`
Parsing, preparation, solving, observation and writer failures propagate through
the fallible `main`. A control or resource stop does not become a complete-family
claim; answers printed before it remain a partial prefix.

## What the calls do

1. **Prepare the program.** `zetesis_themelios::prepare_formula` parses, checks
   and prepares this finite choice program; `ground` admits its eager formula
   representation. Keep that owner alive while `PreparedInput::formula` borrows
   it. The application does not use themelios's parser directly.
2. **Configure execution.** `SolveConfig` selects CPU execution and eager
   grounding. `models: 0` requests every answer. `Cancellation` can supply cooperative
   cancellation and deadlines without changing the program's semantics.
3. **Consume typed answers.** `Session::enumerate` yields
   `Result<AnswerSet, SolveFailure>`. `AnswerSet::interpretation` exposes the full
   true-atom set. The observation API renders logical atom spellings, either all
   atoms or the original source's `#show` selection; it does not alter membership.
4. **Inspect completion.** `Completion::Exhausted` establishes complete search.
   Finding an answer, reaching a requested answer limit or dropping the iterator
   early does not. A complete empty family establishes inconsistency; an empty
   partial prefix does not.

If the application needs to retain a complete family, use bounded
`WorldView::collect` or `SessionBuilder::collect`. They preserve partial failure
evidence instead of returning a `WorldView` for an incomplete collection. See
[completion and output](outcomes.md).

For normal-rule or multi-file source admission, see [program preparation](source.md).
To construct a program as typed values, see the
[relational example](source.md#admit-an-existing-logical-program) and
[formula example](source.md#prepare-a-logical-formula-program).
For objectives, `Session::enumerate` returns all answer sets with scores, while
`Session::new` selects incumbents; inspect completion before calling an incumbent
optimal. Continue with [sessions](sessions.md), [costs and shown terms](costs-and-output.md)
or the [library reference index](libraries.md).
