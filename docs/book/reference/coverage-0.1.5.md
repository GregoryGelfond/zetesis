# Version 0.1.5 coverage

<!-- A dated record: its commands keep the spellings of the binaries it records. -->

This record retains the version 0.1.5 coverage snapshot that the README badge
reports: a recorded local measurement, not a live hosted-CI status. Its
commands keep the spellings of the revisions they measured.

| Population | Covered / instrumented lines | Coverage |
| --- | ---: | ---: |
| Workspace, all features, portable tests | 108,128 / 117,927 | 91.69% |
| CPU-only solver library and CLI, separate instrumentation | 9,666 / 10,549 | 91.63% |

This version `0.1.5` snapshot measures compiled source
[`318c8238`](https://github.com/GregoryGelfond/zetesis/tree/318c8238ad72719deee63f9b5250d3e1bbce565b)
using Rust 1.97.1, cargo-llvm-cov 0.8.7 and LLVM 22.1.6 on macOS 26.6.2.
The [verification receipt](observations/coverage-318c8238.json) retains exact
counts, source identities, report hashes and qualification scope. Documentation
updates do not change the measured source. The
[canonical-storage comparison](canonical-storage.md) records performance
separately; its measured executable is byte-identical to the qualified release.

Both populations passed their independent 91% floor. The workspace contains
2,433 portable profiles; the CPU-only population contains 342 profiles.
No physical profiles were imported into either population.

Sixty tests in 16 groups passed separately on Apple M4 Pro Metal at
[`3b06e479`](https://github.com/GregoryGelfond/zetesis/tree/3b06e4795b3a00cd0fbf11d3e0f4f2c32750cf2c).
That revision changes only an ignored table-join test to request eager grounding
explicitly; automatic grounding can instead reconstruct terminal definitions.
All assertions remain, and production sources and the release executable are
unchanged. The ordinary `zetesis test backend --device metal` command also
passed its three complete-family checks with actual device work. These runs
were not instrumented and do not contribute to the coverage badge.

The portable, external-oracle, manual and coverage gates passed at `318c8238`.
The corrected test also passed formatting, its related CPU tests and strict
Clippy before device qualification. The Lean 4.33.1 build, axiom audit and
source-record checks cover 147 semantic modules and 1,348 audited theorems.
Their source hashes are recorded in the
[verification record](https://github.com/GregoryGelfond/zetesis/blob/3b06e4795b3a00cd0fbf11d3e0f4f2c32750cf2c/proofs/verification.json).
These counts describe the checked
mathematical library, not verification of the Rust grounder, masks or GPU
execution. Historical corpus and performance results retain their original
source identities in the [grounding comparison](grounding-measurements.md).

The static closure comparison checked 53 candidate executions against an
independent ordered-set reference. Owned seeds, indexed selections and manual
selections share that reference, including reused epochs and the 4,096-atom
boundary. Each of the four tight-oracle physical tests exercises both Atomic
and Grouped support construction. The relation tests cover typed equality masks and
prepared-view refusals.
The shared-context tests cover formula execution while relation columns remain
prepared, non-destructive contention refusal and failure propagation to peers.
They check reuse after healthy, settled preparation cancellation, as well as
caller control during static and formula execution. Ordinary-session tests
check exact context identity across explicit GPU eager/lazy closure and formula
setup. Automatic execution remains on CPU, including when the caller supplies
GPU resources. Repeated sessions preserve
independent subjects, budgets, costs and outcomes while sharing the device;
policy and observer refusals preserve later reuse.
The formula-session tests distinguish tight support from general reduct checking
using checked plan preconditions. They verify automatic tight dispatch, general
checking for a non-tight positive cycle, and a finite tight-work refusal before
device submission. An unseeded positive cycle grounds to the empty theory and
therefore exercises tight checking; source recursion alone does not determine
the ground theory's plan.
Compiled-profile tests check exact pipeline identity across fresh formula oracles
and ordinary library sessions, including device health and contention boundaries.
Builder collection tests use caller-owned resources and retain incomplete results
without claiming a complete `WorldView`. The independent CPU population covers
both the composed solver and its CLI consumer after their crate separation.
Explicit lazy execution checks preserve source grounding, immutable-upload reuse
and complete candidate accounting. Formula checks exercise dependency-level
original truth, packed auxiliary domains, strict-subset reduction and actual
submission receipts under finite device limits.
Combined language-consumer tests preserve complete answer-set families, scored
observations and all optimum ties across aggregate heads, objectives and output
queries. They require actual GPU work and exact accounting of CPU residuals.
The ordinary table-join case requires positive table preparation, probe and row
counts, actual GPU candidates, exact decided/residual accounting and no pending
results. Its complete Metal family equals the independent CPU family. It checks
host table grounding composed with GPU reduct checking, not a GPU table kernel.
Other devices are outside this measurement; the hardware gate qualifies the
Vulkan selection on a host exposing a Vulkan adapter.

Reproduce the portable snapshot with `scripts/check.sh coverage` at `318c8238`
using the [verification tools](validation.md#prepare-verification-tools). At `3b06e479`, run
`scripts/check.sh hardware --metal` for the 60 device tests and
`zetesis test backend --device metal --json` for the three CLI checks.
Retain the coverage JSON and HTML reports and the hardware logs separately.
Running `scripts/check.sh coverage --metal` or `--vulkan` creates a different,
combined measurement; it is not the population reported above.
