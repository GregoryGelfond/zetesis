# Coverage and Metal qualification

<!-- A dated record: its commands keep the spellings of the binaries it records. -->

The README badge reports portable line coverage for implementation
[`120fadfb`](https://github.com/GregoryGelfond/zetesis/commit/120fadfb3744c00760bcefd575ce3641075341dc).
Separate Metal tests qualified
[`f9e1506c`](https://github.com/GregoryGelfond/zetesis/commit/f9e1506cf31e77cf7d58a40aa7d048d6d1b45747),
which adds measurement documentation without changing the implementation or
tests. These are recorded local checks, not a hosted-CI status.

| Population | Covered / instrumented lines | Coverage |
| --- | ---: | ---: |
| Workspace, all features, portable tests | 107,496 / 116,546 | 92.23% |
| CPU-only solver library and CLI, independent instrumentation | 9,307 / 10,020 | 92.88% |

Both populations passed their separate 91% floors with Rust 1.97.1,
cargo-llvm-cov 0.8.7 and LLVM 22.1.6. The maintained test-support exclusions
apply. Neither population includes physical-device tests.

All **58 selected Metal tests passed in 14 groups** on Apple M4 Pro. They check
CPU/device answer agreement, actual device work, shared execution resources and
failure boundaries. The maintained reader rechecked each group's exact named
test results. These uninstrumented checks do not increase portable coverage or
qualify Vulkan and other devices.

The portable, external-clingo, Lean and book gates also passed. The oracle gate
checked all 15 maintained campaigns. The [verification receipt](observations/coverage-120fadfb.json)
retains report hashes, commands, exact coverage counts and the complete physical
test selection. Line coverage, regression tests and semantic Lean laws do not
establish end-to-end correctness of Rust or shaders.

To reproduce at the named source, prepare the
[verification tools](validation.md#prepare-verification-tools), then run:

```sh
sh scripts/check.sh portable
sh scripts/check.sh oracle
sh scripts/check.sh coverage
sh scripts/check.sh proofs
sh scripts/check.sh book
sh scripts/check.sh hardware --metal
```

The last command requires an accessible Metal adapter. Adding `--metal` to the
coverage command produces a different population from the portable reports
above. The [previous coverage snapshot](coverage-0.1.6.md) and
[performance comparison](foundation-reuse.md) keep their own source identities.
