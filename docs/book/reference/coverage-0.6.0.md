# Coverage and Metal qualification, 0.6.0

<!-- A dated record: its commands keep the spellings of the binaries it records. -->

The maintained coverage command passed on 14 October 2026 for the 0.6.0
implementation. The [receipt](observations/coverage-0.6.0.json) identifies the
reports, tools and [source contents](observations/coverage-0.6.0-sources.json).
The measured tree includes the checked lookup-priority correction following
`715dabb2`; final release edits after this run change documentation only.
These are local measurements, separate from hosted CI status.

| Population | Covered / instrumented lines | Coverage |
| --- | ---: | ---: |
| Workspace, all features, portable tests | 146,143 / 156,838 | **93.18%** |
| Same workspace instrumentation, portable and selected Metal tests | 148,394 / 156,838 | **94.62%** |
| CPU-only solver, public API and CLI, independent instrumentation | 11,171 / 12,161 | **91.86%** |

The combined workspace and independent CPU-only populations passed their
separate **91% floors**. The portable snapshot supplies the README badge.
Metal added 2,251 covered Rust lines within the same workspace denominator;
CPU-only profiles were not merged into it. The maintained test-support
exclusions and cargo-llvm-cov's default filename filters apply.

All **63 selected Metal tests passed in 14 groups** on Apple M4 Pro. Each group
ran exactly its required named tests. The selection covers complete CPU/device
answer agreement, aggregate reductions, compact relation transport, shared
resources and failure handling. Hybrid sessions check streamed constraints,
optimal ties, interruption and terminal reconstruction over device-checked
cores. Vulkan and unlisted device tests are outside this record.

The run used Rust 1.97.1, cargo-llvm-cov 0.8.7 and LLVM 22.1.6 on
`aarch64-apple-darwin`. The manifest records 2,391 source, fixture and
build-control files, including all 766 filenames in the coverage reports.
LLVM reports contain filenames rather than source-content hashes; the separate
manifest records the checked source correspondence, without asserting
reproducible-build equivalence.

To reproduce from the 0.6.0 source, prepare the
[verification tools](validation.md#prepare-verification-tools) and run:

```sh
sh scripts/check.sh coverage --metal
```

An accessible Metal adapter is required. The command retains the portable
workspace snapshot before adding physical tests, then checks both independent
floors. The [validation reference](validation.md#coverage) explains the
populations and reports. The [0.5.0 record](coverage-0.5.0.md) retains its
original measurements.

Line coverage does not measure assertion strength, WGSL instruction coverage,
solver performance or complete Rust/shader correctness. The
[proof correspondence](../lean/correspondence.md) states the verified scope.
