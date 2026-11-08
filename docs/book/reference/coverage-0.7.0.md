# Coverage and Metal qualification, 0.7.0

<!-- A dated record: its commands keep the spellings of the binaries it records. -->

The 0.7.0 release candidate passed the portable and coverage/Metal gates on
7 November 2026 at commit `878f08509852654a27e3cb6158e746c9d39a1a1d`
(tree `d37a7b6d8cf33c2d13ccdc5513ce6ae80deb31ca`). The candidate uses root
arithmetic deductions and skips repeated outer parity and cardinality
inspections during candidate generation. The [coverage receipt](observations/coverage-0.7.0.json)
records the tool versions, report hashes, physical selection and gate outcomes.

| Population | Covered / instrumented Rust lines | Line coverage | Gate |
| --- | ---: | ---: | --- |
| Workspace, all features, after portable tests | 154,371 / 166,644 | 92.64% | Diagnostic snapshot used by the README badge |
| Same workspace after portable and Metal tests | 157,762 / 166,644 | **94.67%** | Passed the 91% floor |
| CPU-only solver, engine, facade and CLI | 10,666 / 11,603 | **91.92%** | Passed the independent 91% floor |

The Metal tests exercised 3,391 additional Rust lines in the same workspace
instrumentation. The CPU-only run built and measured its four packages
independently. Both floor checks exited successfully.

All **59 selected Metal tests passed in 11 groups** on an Apple M4 Pro. The
selection includes native aggregate reduction, formula and tight checking,
candidate slates, residual countermodel preparation, session reuse and
interruptions. The receipt lists the exact test names and per-group log
hashes. The device qualification applies to the recorded Metal adapter.

The gate used Rust 1.97.1, cargo-llvm-cov 0.8.7 and LLVM 22.1.6 on
`aarch64-apple-darwin`. The [source manifest](observations/coverage-0.7.0-sources.json)
records 2,573 files checked against the measured Git tree, including all
808 distinct source filenames in the LLVM reports.

To reproduce at the measured commit, prepare the
[verification tools](validation.md#prepare-verification-tools) and run these
commands on a host with an available Metal adapter.

```sh
sh scripts/check.sh portable
sh scripts/check.sh coverage --metal
```

The coverage command saves the workspace snapshot before adding physical
tests, checks the selected Metal results, then instruments the CPU-only
profile and evaluates the two floors. See the
[validation reference](validation.md#coverage) for the reports and
[the previous qualification](coverage-0.6.0.md) for the 0.6.0 measurements.
