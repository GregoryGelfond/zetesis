# Coverage and Metal qualification, 0.8.0

<!-- A dated record: its commands keep the spellings of the binaries it records. -->

The implementation integrated for 0.8.0 passed the portable and coverage/Metal
gates on 15 November 2026 at commit
`3d395e57f14ca39c8ab436dfc1dbb7439982e4b7`
(tree `2a750bdf015499f925c13235fd958e178074b994`).
[PR #18](https://github.com/GregoryGelfond/zetesis/pull/18) preserves that tree
in the development branch. The subsequent release preparation changes version
metadata and documentation; the recorded executable still identifies itself as
0.7.0. This page identifies the measured source rather than claiming another
hardware run of the release metadata.

The [coverage receipt](observations/coverage-0.8.0.json) records exact report
hashes, tool versions, floor results and named physical tests.

| Population | Covered / instrumented Rust lines | Line coverage | Gate |
| --- | ---: | ---: | --- |
| Workspace, all features, after portable tests | 161,689 / 174,228 | **92.80%** | Diagnostic snapshot used by the README badge |
| Same workspace after portable and Metal tests | 165,077 / 174,228 | **94.75%** | Passed the 91% floor |
| CPU-only solver, engine, facade and CLI | 10,718 / 11,654 | **91.97%** | Passed the independent 91% floor |

The selected Metal tests exercised 3,388 additional Rust lines in the same
workspace instrumentation. CPU-only coverage uses its own instrumented build;
the two floor profiles are not merged.

All **59 selected Metal tests passed in 11 groups** on an Apple M4 Pro. They
cover native aggregate reduction, relation masks, tight and formula checking,
overlapping candidate batches, exact CPU completion, shared contexts, ordinary
sessions, interruption and failure boundaries. The selection includes completed
support-table joins and quantified families with actual device execution.
This is Metal qualification; it does not qualify Vulkan or every possible
device path.

The gate used Rust 1.97.1, cargo-llvm-cov 0.8.7 and LLVM 22.1.6 on
`aarch64-apple-darwin`. Fresh hosted [PR checks](https://github.com/GregoryGelfond/zetesis/actions/runs/43719467727)
also passed Linux and macOS portable checks, Linux coverage and its committed
floor check, the book, and the Lean proof checks. Hosted Linux coverage is
separate from the local Metal observations above.

To reproduce at the measured revision, prepare the
[verification tools](validation.md#prepare-verification-tools) and run on a
host exposing a Metal adapter:

```sh
sh scripts/check.sh portable
sh scripts/check.sh coverage --metal
```

The command retains the portable snapshot before the physical tests, then
instruments the CPU-only profile and checks both floors. See the
[validation reference](validation.md#coverage) for the maintained populations
and the [0.7.0 record](coverage-0.7.0.md) for the preceding release's evidence.
