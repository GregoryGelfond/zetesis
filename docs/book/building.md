# Building the documentation

The manual uses mdBook 0.5.4. Run the following from the repository root:

```sh
scripts/check.sh book
```

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

The session and reduct examples are included from Rust files under
`docs/book/examples`; the code displayed in the chapters is the code tested by
mdBook. Build their dependencies without requiring a physical GPU, then run:

```sh
cargo build --locked -p zetesis-cli --lib --no-default-features --target-dir target/book-tests
mdbook test --library-path target/book-tests/debug/deps
```

Reserve `target/book-tests` for this example configuration. Separate storage
keeps Cargo check/rustdoc metadata and other feature configurations from making
mdBook's crate lookup ambiguous. The ordinary application and API documentation
can continue using `target`.

Each example checks a semantic result and propagates typed failures. No GPU
support or performance claim follows from these portable example tests.
Lean blocks are excerpts or consumer examples, not Rust doctests. Build their
own package with:

```sh
cd proofs
lake build
```

The Lean toolchain is declared by `proofs/lean-toolchain`. Building the book
does not replace checking the proof package, source/API links, workspace Rust
gates or the physical execution paths described by a backend claim.
