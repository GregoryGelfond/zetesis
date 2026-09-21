# Getting started with the library

Use `zetesis-solve` to solve a program from Rust and receive answer sets as typed
values. Your application chooses how to load source and display results; a solve
does not require command-line arguments or captured text output.

This example finds the two answer sets `{a}` and `{b}` of:

```asp
{{#include ../examples/choices.lp}}
```

## Set up a project

The packages are not published on crates.io. Use a checkout of zetesis, or pin
its Git revision in your application's dependencies. The example below assumes
`zetesis` and your new application are sibling directories. Use Rust 1.97 or
newer; the repository's checked toolchain is 1.97.1.

```sh
cargo new answer-set-app
cd answer-set-app
```

Replace the new project's empty `[dependencies]` section in `Cargo.toml` with:

```toml
[dependencies]
zetesis-solve = { path = "../zetesis/crates/zetesis-solve", default-features = false }
zetesis-themelios = { path = "../zetesis/crates/zetesis-themelios" }
zetesis-cpu = { path = "../zetesis/crates/zetesis-cpu" }
```

This selects a CPU-only solver build. GPU support is enabled by default when
`default-features = false` is omitted; compiling that support does not itself
select a GPU. Cargo features are shared across dependencies, so another package
can enable that feature for the same build.

## Solve and check completion

Put the following in `src/main.rs`. This is the same source as the repository's
`book-getting-started` example.

```rust
# extern crate zetesis_solve;
# extern crate zetesis_cpu;
# extern crate zetesis_themelios;
{{#include ../examples/getting-started.rs:example}}
```

Run it with `cargo run`. It prints each full interpretation using Rust's debug
format and checks that enumeration completed. This is not the source's `#show`
projection. From the zetesis checkout, run the maintained example directly:

```sh
cargo run --locked -p zetesis-solve --no-default-features --example book-getting-started
```

The main steps are:

1. **Prepare the source.** `admit` parses and checks this normal-rule program,
   returning an owner for its admitted representation. Keep that owner alive
   while `PreparedInput::admitted` borrows it.
2. **Start a session.** `Session::enumerate` searches the original program.
   `Backend::Cpu` selects CPU execution; `models: 0` requests all answers.
   `Control` supplies cooperative cancellation and deadlines when needed.
3. **Read answer sets.** Each successful item is an `AnswerSet`.
   `interpretation()` gives its full true-atom set, including atoms that source
   display directives might hide. The `?` operator propagates a failed pull.
4. **Check what finished.** Reaching an answer is different from exhausting the
   search. The example requires `Completion::Exhausted` and returns an error if
   enumeration stopped early.

A resource limit, cancellation or failure can leave valid answers without a
complete family. If the application needs to retain that family, use bounded
`WorldView::collect` rather than an unbounded vector. A complete empty collection
establishes inconsistency; an empty partial result does not. See
[completion and output](outcomes.md) for this API and its failure evidence.

## Adapt the example

`admit` deliberately supports a normal-rule profile. Broader source programs may
need `admit_extended` or `admit_formula`; successful parsing alone does not mean
that a profile supports the program. See [source preparation](source.md) for
those choices and multi-file input.

For optimization, choose deliberately: `Session::enumerate` returns all original
answer sets with their scores, including nonoptimal ones. `Session::new` selects
incumbents when an objective exists. Inspect the final outcome before calling an
incumbent optimal. The [session guide](sessions.md) covers streaming, stopping,
execution policy and optimization; [completion and output](outcomes.md) explains
complete collection and publication.

Use [interpretations and atoms](models.md) to inspect results,
[costs and shown terms](costs-and-output.md) for source presentation, and the
[library reference index](libraries.md) when you need a lower-level operation.
