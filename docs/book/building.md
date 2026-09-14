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

The session, resource-sharing, measurement, source-preparation, objective-selection,
reduct and derived-workload examples are
included from Rust files under `docs/book/examples`; the code displayed in the
chapters is the code tested by mdBook. Build their dependencies without requiring
a physical GPU through the maintained command:

```sh
scripts/check.sh book
```

mdBook invokes rustdoc from a temporary directory. The command exports the
checkout's selected toolchain so examples use the same compiler as their
dependencies, even when the machine's default toolchain differs.

`scripts/check.sh book` explicitly reuses the checkout's ordinary `target`
directory and passes rustdoc a fresh view containing only libraries named by that
successful Cargo build's JSON messages. This excludes stale competing rlibs from
earlier source revisions without deleting the shared build cache. The maintained
`book::select` operation checks required crates and complete build output;
`Libraries::publish` confines the regular files to that build directory and
hard-links them into the new view. Do not modify build artifacts concurrently
with the check. The view is removed afterward; its Cargo messages remain under
`target/book-views/run.*`. Application, API documentation and book checks can
reuse this checkout's ordinary target. Other source checkouts and the independent
instrumented coverage populations keep separate build directories, as described
in the [build storage policy](https://github.com/GregoryGelfond/zetesis/blob/main/CONTRIBUTING.md#build-storage).

The examples check their stated contracts and propagate typed failures. No GPU
support or performance claim follows from these portable example tests.
The derived-workload acquisition block uses `no_run`: mdBook checks its types
without launching solvers. Its Cargo example test separately checks N=4 source
derivation and identity against the admitted corpus, with no child processes.
The shared-device example also uses `no_run`: mdBook checks compilation without
device discovery. Device behavior is checked by the separate physical test
population. Running the example requires an accessible GPU. GPU support is compiled
for this dependency set; the other examples select CPU execution explicitly.
The same files are registered as Cargo examples, so workspace formatting and
all-target Clippy checks apply to them. The authored-lint inventory includes
`docs/book/examples` as a maintained source root.

To run one example as an ordinary consumer from the checkout root:

```sh
cargo run --locked -p zetesis-solve --no-default-features --example book-session
cargo run --locked -p zetesis-solve --no-default-features --example book-source
cargo run --locked -p zetesis-solve --no-default-features --example book-selection
cargo run --locked -p zetesis-solve --no-default-features --example book-measurements
```

On a machine with an accessible physical GPU, run the shared-resource example:

```sh
cargo run --locked -p zetesis-solve --all-features --example book-resources
cargo run --locked -p zetesis-solve --all-features --example book-profiles
```

The tour's ASP fixture is included directly in the chapter. A Cargo example test
checks that its bytes match the session example's source literal; the literal
keeps the displayed Rust independently runnable without a working-directory
assumption. That example enables its test harness in Cargo, so the ordinary
workspace test gate runs the correspondence assertion. Run it alone with:

```sh
cargo test --locked -p zetesis-solve --no-default-features --example book-session
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
