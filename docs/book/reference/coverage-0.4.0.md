# Coverage and Metal qualification, 0.4.0

<!-- A dated record: its commands keep the spellings of the binaries it records. -->

The canonical coverage command passed for zetesis 0.4.0 on 7 October 2026.
The [receipt](observations/coverage-0.4.0.json) identifies the reports, tools and
[source content](observations/coverage-0.4.0-sources.json). These are local
measurements, not a hosted-CI status or a claim that a release tag was measured.

| Population | Covered / instrumented lines | Coverage |
| --- | ---: | ---: |
| Workspace, all features, portable tests | 118,777 / 128,165 | **92.68%** |
| Same workspace instrumentation, portable and selected Metal tests | 120,985 / 128,165 | **94.40%** |
| CPU-only solver, public API and CLI, independent instrumentation | 10,232 / 11,167 | **91.63%** |

The combined workspace and independent CPU-only populations passed their
separate **91% floors**. The retained portable snapshot also exceeds 91% and
supplies the README badge. Metal added 2,208 covered Rust lines within the same
workspace denominator; CPU-only profiles were not merged into it. The maintained
test-support exclusions and cargo-llvm-cov's default filename filters apply.

All **59 selected Metal tests passed in 14 groups** on Apple M4 Pro. Each group
contains exactly its required named tests and one positive summary with its
required count. Zero-match summaries from unrelated workspace test binaries do
not count as qualification. The selection includes 8/16/32-bit relation column
transport, complete CPU/device answer agreement, shared resources and failure
behavior. It qualifies neither Vulkan nor unlisted device tests.

The run used Rust 1.97.1, cargo-llvm-cov 0.8.7 and LLVM 22.1.6 on
`aarch64-apple-darwin`. LLVM reports retain source filenames; the separate
manifest records their content and the workspace/test/build-control inputs.
No measured commit or release-tag mapping is inferred from those filenames.

To reproduce on the recorded source content, prepare the
[verification tools](validation.md#prepare-verification-tools) and run:

```sh
sh scripts/check.sh coverage --metal
```

An accessible Metal adapter is required. This command retains the portable
workspace snapshot before adding the physical tests, then checks the combined
workspace and independent CPU-only floors. `coverage` without a backend selects
portable tests only. The [validation reference](validation.md#coverage) explains
the populations and retained reports.

Line coverage does not measure assertion strength, WGSL instruction coverage,
solver performance or complete Rust/shader correctness. The
[proof correspondence](../lean/correspondence.md) states the verified scope.
The [0.3.0 record](coverage-0.3.0.md) retains its original source and measurements.
