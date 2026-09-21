# The zetesis Book

zetesis is an answer-set solver built around the reduct, with lazy grounding,
parallel CPU execution and optional GPU computation. This manual covers using
the command, embedding the libraries and understanding the solver.

## Start here

| I want to… | Read |
| --- | --- |
| Install zetesis and solve a program | [Installation and first solve](../../README.md#install-and-run), then the [command guide](reference/commands.md) |
| Check whether my program is supported | [Admitted language](reference/language.md) |
| Use zetesis from Rust | [Getting started with the library](rust/getting-started.md), then [solving sessions](rust/sessions.md) |
| Understand how the solver works | [A guided tour](architecture/tour.md), then [programs, answer sets and the reduct](architecture/semantics.md) |
| Read or use the proofs | [Lean definitions and imports](lean/foundations.md) |
| Compare performance or run benchmarks | [Performance results](reference/performance.md), then the [benchmark commands](reference/commands.md#measure-a-corpus) |

## Three parts

**[Part I: Solver design and architecture](architecture/tour.md)** follows a
program from source to answer sets. It explains grounding, reduct checking and
execution choices, assuming basic familiarity with ASP.

**[Part II: The Rust library programmer's manual](rust/getting-started.md)** starts
with a complete solve, then covers results, source preparation and lower-level
operations. Examples use public APIs and are checked with the code. The
[library map](rust/libraries.md) helps locate a particular capability.

**[Part III: The Lean proof library](lean/foundations.md)** introduces the
definitions and laws behind the solver. It includes a worked proof and explains
the connection to the implementation. Reading the argument does not require
Lean tactic fluency; extending the proofs does.

## Results and limits

An *answer set* has passed membership checking. A *world view* is the complete
family of a program's answer sets. A stopped stream, a displayed projection and
a selected optimum have different guarantees; the
[results chapter](rust/outcomes.md) explains how to distinguish them.

The solver is experimental. The [language reference](reference/language.md)
describes supported inputs, and the [validation guide](reference/validation.md)
explains the checks used to assess the implementation. Lean theorems establish
their stated mathematical claims; they do not yet verify the complete Rust
implementation, source compiler or GPU shaders. See the
[proof correspondence](lean/correspondence.md) for that boundary.

The [vocabulary](vocabulary.md) defines recurring terms. For local documentation
builds and checked examples, see [Building the documentation](building.md).
