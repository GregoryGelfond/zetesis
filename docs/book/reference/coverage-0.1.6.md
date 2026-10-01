# Version 0.1.6 coverage

<!-- A dated record: its commands keep the spellings of the binaries it records. -->

The README badge reports this local coverage measurement at
[`0f787751`](https://github.com/GregoryGelfond/zetesis/tree/0f787751888f2fbdc7e166c5862b6d8cdd71bfd8).
It is not a hosted-CI status. Later documentation changes do not alter the
measured implementation or tests.

| Population | Covered / instrumented lines | Coverage |
| --- | ---: | ---: |
| Workspace, all features, portable tests | 104,748 / 113,674 | 92.15% |
| CPU-only solver library and CLI, separate instrumentation | 9,193 / 9,903 | 92.83% |

Both populations passed their independent 91% floor using Rust 1.97.1,
cargo-llvm-cov 0.8.7 and LLVM 22.1.6. Test-support crates are excluded by the
maintained coverage policy. Neither population includes physical-device tests.
The [verification receipt](observations/coverage-0f787751.json) records exact
counts, report hashes, commands and the separate hardware qualification.

All 58 selected Metal tests passed in 14 groups on Apple M4 Pro. These
uninstrumented tests check CPU/device answer agreement, actual device work,
shared resources, interruption and failure boundaries. They do not increase the
coverage percentages above or qualify Vulkan and other devices.

The portable, external-clingo, coverage and manual gates also passed. The oracle
gate checked its 15 campaigns against their maintained selections. Proof sources
did not change in this update; the Lean gate was not rerun. Line coverage and
device tests do not establish formal correctness of Rust or shaders.

To reproduce, check out the linked revision, install the
[verification tools](validation.md#prepare-verification-tools), then run:

```sh
sh scripts/check.sh portable
sh scripts/check.sh oracle
sh scripts/check.sh coverage
sh scripts/check.sh book
sh scripts/check.sh hardware --metal
```

The last command requires an accessible Metal adapter. Retain coverage reports
and hardware logs separately. Adding `--metal` to the coverage command produces
a different test population from the portable measurement reported here.
The [performance comparison](instantiation-lazy.md) identifies its measured
executables separately and includes the combined grounding and closure changes.
