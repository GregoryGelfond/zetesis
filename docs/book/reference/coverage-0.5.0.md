# Coverage and Metal qualification, 0.5.0

<!-- A dated record: its commands keep the spellings of the binaries it records. -->

The maintained coverage command passed on 10 October 2026 at
[`df5b3d51`](https://github.com/GregoryGelfond/zetesis/commit/df5b3d515aca42621fac6e7fb3ffc334b4ee949d),
whose workspace version is 0.5.0. The [receipt](observations/coverage-0.5.0.json)
identifies the reports, tools and [source content](observations/coverage-0.5.0-sources.json).
The release is finalized by a later documentation-only commit; the measured
source remains the commit above. These are local measurements, separate from
hosted CI status.

| Population | Covered / instrumented lines | Coverage |
| --- | ---: | ---: |
| Workspace, all features, portable tests | 125,081 / 134,668 | **92.88%** |
| Same workspace instrumentation, portable and selected Metal tests | 127,324 / 134,668 | **94.55%** |
| CPU-only solver, public API and CLI, independent instrumentation | 10,401 / 11,349 | **91.65%** |

The combined workspace and independent CPU-only populations passed their
separate **91% floors**. The portable snapshot also exceeds 91% and supplies
the README badge. Metal added 2,243 covered Rust lines within the same workspace
denominator; CPU-only profiles were not merged into it. The maintained
test-support exclusions and cargo-llvm-cov's default filename filters apply.

All **63 selected Metal tests passed in 14 groups** on Apple M4 Pro. Each group
contains exactly its required named tests and one positive summary with its
required count. Zero-match summaries from unrelated workspace test binaries do
not count as qualification. The selection covers complete CPU/device answer
agreement, compact relation transport, shared resources and failure behavior.
It also includes hybrid sessions with streamed constraints, optimal ties,
queued interruption, source faults and terminal reconstruction over device-checked
cores. Vulkan and unlisted device tests remain outside this record.

The run used Rust 1.97.1, cargo-llvm-cov 0.8.7 and LLVM 22.1.6 on
`aarch64-apple-darwin`. The source manifest records 1,695 tracked source and
build-control files, including all 670 source filenames in the coverage reports;
their contents match the measured commit. LLVM reports record filenames rather
than source-content hashes. The separate manifest establishes the checked source
correspondence without asserting reproducible-build equivalence.

To reproduce at the measured commit, prepare the
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
The [0.4.0 record](coverage-0.4.0.md) retains its original source and measurements.
