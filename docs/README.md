# Documentation

The [zetesis manual](book/index.md) is the main guide to the solver. Its three
parts cover the architecture and reduct semantics, the Rust library interfaces,
and the Lean proof library. Start with the [guided tour](book/architecture/tour.md)
or the [library map](book/rust/libraries.md).

The mdBook sources live in `book/`; the repository's `book.toml` configures the
build. [Building the documentation](book/building.md) explains the toolchain and
checks. Rust examples in `book/examples/` are shared with executable checks.
The formal definitions and proofs live in the independent
[Lean package](../proofs/README.md).

The [admitted language](book/reference/language.md) describes supported source
constructs and their boundaries. Crate READMEs and rustdoc provide the detailed
API contracts referenced by the manual. Curated examples and regression fixtures
remain beside the code that consumes them, with their provenance and licenses.
