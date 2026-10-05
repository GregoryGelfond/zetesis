# Coverage and Metal qualification

<!-- A dated record: its commands keep the spellings of the binaries it records. -->

These local checks cover the implementation released in
[`v0.3.0`](https://github.com/GregoryGelfond/zetesis/releases/tag/v0.3.0).
Portable coverage was collected before the version change. The final source
inventory confirms that every authored Rust and WGSL file is unchanged; only
the workspace version and corresponding lockfile entries differ. Metal checks
ran after that version change. Neither result is a hosted-CI status.

| Population | Covered / instrumented lines | Coverage |
| --- | ---: | ---: |
| Workspace, all features, portable tests | 109,813 / 119,145 | 92.17% |
| CPU-only solver, public API and CLI, independent instrumentation | 9,831 / 10,729 | 91.63% |

Both populations passed their separate 91% floors with Rust 1.97.1,
cargo-llvm-cov 0.8.7 and LLVM 22.1.6. The maintained test-support exclusions
apply. Neither population includes physical-device tests.

All **58 selected Metal tests passed in 14 groups** on Apple M4 Pro. The
maintained reader rechecked each group's exact named results. These tests check
CPU/device answer agreement, device work, shared resources and failure behavior.
They do not increase portable coverage or qualify Vulkan.

The portable Rust checks, all 15 maintained clingo campaigns, the book and the
stated Lean checks also passed. The initial portable run stopped at a test-helper
lint; that helper was factored, and the lint, affected test and remaining checks
then passed. This was not one uninterrupted portable run.

The [verification receipt](observations/coverage-0.3.0.json) records report
hashes, source correspondence and the physical test selection. Coverage and
regression tests do not establish end-to-end correctness of Rust or shaders.
The [proof correspondence](../lean/correspondence.md) states the verified scope.

To reproduce, check out `v0.3.0`, prepare the
[verification tools](validation.md#prepare-verification-tools), then run:

```sh
sh scripts/check.sh portable
sh scripts/check.sh oracle
sh scripts/check.sh coverage
sh scripts/check.sh proofs
sh scripts/check.sh book
sh scripts/check.sh hardware --metal
```

The last command requires an accessible Metal adapter. `coverage --metal`
produces a different test population. The [previous snapshot](coverage-120fadfb.md)
retains its own source identities and measurements.
