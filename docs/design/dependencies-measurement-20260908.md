# Aggregate dependencies and execution measurements

This tranche starts from `974de9395d1fe0f4a4d4aa30f3268198a5c7b1a1`, after
the eight scoped physical Metal tests passed. The original theory and its
frozen reduct remain the solver's semantic foundation. The following work is
planned; this document does not establish implementation or measured gains.

## Independent slices

- Admit bounded acyclic dependencies between aggregate assignment producers,
  including their existing scalar and range descendants. Keep original equality
  guards in every generated row. Qualify the shared ordinary, choice and checked
  count-head planner without claiming general head aggregates or dependent
  objective consumers.
- Identify repeated CPU candidate, propagation or objective work, then evaluate
  one general reduction. Preserve candidate order, exact reduct acceptance and
  explicit resource accounting. A smaller representation alone is insufficient
  reason to accept an optimization.
- Add a reusable Rust performance matrix over the clean example corpus, with
  explicit eager/lazy and CPU/Metal configurations. Preserve the established
  uninstrumented CPU protocol and its results. The new matrix uses instrumented
  native samples so each sample records its actual execution route, device work
  and available grounding/solving phases. Its wall interval includes that
  instrumentation and JSON output; it must not be presented as an uninstrumented
  comparison.
- Measure lazy source selection on identical relational programs and ordered
  frozen candidate batches, using scalar/Rayon reference checking and portable
  and physical Metal round execution. Sparse and dense cases are both required.
  Source work, transfer bytes and mask payload are separate quantities; none is
  process RSS or a device-only kernel duration.

## Measurement and release boundaries

Every requested matrix cell retains its disposition. Unsupported lazy formula
inputs, unavailable adapters, limits and failures are not zero-duration successes.
Display/count/cost agreement is named separately from complete hidden-model
agreement when the reference capture does not expose hidden atoms. No eager
fallback can qualify a requested lazy route. A Metal request with no submitted
device work does not establish GPU acceleration.

Benchmark populations run only after qualification, with competing builds and
compute workloads stopped. Retain all launched samples, fixed schedules, source/executable
identities, limits and raw captures. Process peak RSS remains explicitly
unavailable unless the capture boundary can supply a qualified measurement;
unified-memory accounting cannot be summed as independent host/device memory.
The development context used for this record exposed no adapter. Physical runs
require an environment exposing the device and a frozen command prepared after
the portable checks.

Integrate each accepted slice with focused tests, clingo comparisons, applicable
Lean preservation laws and independent review. Qualification requires the existing
local macOS portable, oracle, proof and two independent 91% coverage gates,
plus the 94 clean corpus and 24 selected upstream cases. Record declined
experiments as such and keep user-facing claims tied to qualified results.
