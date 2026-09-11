# Building the documentation

The manual uses mdBook 0.5.4. Install the
[verification prerequisites](reference/validation.md#prepare-verification-tools)
first, then run the following from the repository root:

```sh
scripts/check.sh book
```

For semantic regressions, proof records and execution measurements, see
[Validating an implementation change](reference/validation.md).

This checks the book build and the displayed Rust examples. To build only the
HTML:

```sh
mdbook build
```

The HTML is written to `target/book`. To include the adjacent local Rust API
reference linked from the manual, generate rustdoc into the same target root:

```sh
cargo doc --locked --workspace --all-features --no-deps --target-dir target
```

Serve the `target` directory when browsing both outputs through HTTP, or open
`target/book/index.html` directly. The API reference is under `target/doc`.
`mdbook serve` serves only its book output, so it does not by itself expose the
sibling rustdoc tree. Source links lead to the maintained repository paths.

## Check the Rust examples

The session, source-preparation, objective-selection, reduct and derived-workload examples are
included from Rust files under `docs/book/examples`; the code displayed in the
chapters is the code tested by mdBook. Build their dependencies without requiring
a physical GPU, then run:

```sh
cargo build --locked -p zetesis-cli -p zetesis-validation --lib --no-default-features --target-dir target/book-tests
mdbook test --library-path target/book-tests/debug/deps
```

Reserve `target/book-tests` for this example configuration. Separate storage
keeps Cargo check/rustdoc metadata and other feature configurations from making
mdBook's crate lookup ambiguous. The ordinary application and API documentation
can continue using `target`.

The examples check their stated contracts and propagate typed failures. No GPU
support or performance claim follows from these portable example tests.
The derived-workload acquisition block uses `no_run`: mdBook checks its types
without launching solvers. Its Cargo example test separately checks N=4 source
derivation and identity against the admitted corpus, with no child processes.
The same files are registered as Cargo examples, so workspace formatting and
all-target Clippy checks apply to them. The authored-lint inventory includes
`docs/book/examples` as a maintained source root.

To run one example as an ordinary consumer from the checkout root:

```sh
cargo run --locked -p zetesis-cli --no-default-features --example book-session
cargo run --locked -p zetesis-cli --no-default-features --example book-source
cargo run --locked -p zetesis-cli --no-default-features --example book-selection
```

The tour's ASP fixture is included directly in the chapter. A Cargo example test
checks that its bytes match the session example's source literal; the literal
keeps the displayed Rust independently runnable without a working-directory
assumption. That example enables its test harness in Cargo, so the ordinary
workspace test gate runs the correspondence assertion. Run it alone with:

```sh
cargo test --locked -p zetesis-cli --no-default-features --example book-session
```

Lean blocks are excerpts or consumer examples, not Rust doctests. The normal-rule
chapter includes named anchors from the maintained `Zetesis.Examples.Choices`
module; its declarations are imported by the proof package's checked umbrella.
Build that package with:

```sh
cd proofs
lake build
```

The Lean toolchain is declared by `proofs/lean-toolchain`. Building the book
does not replace checking the proof package, source/API links, workspace Rust
gates or the physical execution paths described by a backend claim.
