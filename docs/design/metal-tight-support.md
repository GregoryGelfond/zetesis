# Certified tight support on Metal

Status: standalone Rust/WGSL primitive and matched measurement harness implemented,
2026-09-08. Four instrumented physical tests pass on Apple M4 Pro. Matched
uninstrumented measurements and local coverage qualification are complete.
Workspace coverage includes actual Metal execution; its portable stage is
reported separately. Ordinary solve dispatch
and the previously measured `7690475` binaries are unchanged. The
[physical matrix](../verification/dependencies-measurement-tranche-20260908/physical/README.md)
is the motivation, not evidence of this new kernel's performance.

## Implemented experiment

[`GpuTightOracle`](../../crates/zetesis-wgpu/src/tight/mod.rs) accepts an existing
complete `TightPlan` and ordered candidate occurrences. One workgroup evaluates
the original formula DAG, checks roots in original order and reduces producer
support before scanning present atoms. The API preserves exact scalar witnesses,
checked subject identity, finite storage/work limits and separate submitted versus
completed device activity. Failed or cancelled calls do not commit membership.
Decoded device activity alone is not a successful acceptance receipt.

[`zetesis-bench tight`](../../crates/zetesis-experiments/README.md#complete-tight-certificates)
compares scalar, Rayon, fresh Metal and resident Metal classification, followed
by the same serial exact CPU residual completion. Two separately owned GPU
instances keep fresh clearing from invalidating the resident route. The full
DAG, ranks, producers, ordered candidates and independent complete references are
emitted before a rotated initial/warmup/timed schedule. Host whole-call,
classification and residual intervals are measured directly; nested intervals
are not additive. Device setup, reference preparation, witness validation,
publication and fresh cache clearing are explicitly outside these intervals.
This is not an ordinary solve, shader-only timer or process-memory measurement.

Portable tests cover admission, packing, malformed receipts, cache accounting,
independent small-theory semantics and benchmark failure boundaries. Four
compiled physical tests cover small theories, duplicate producers, nonmonotone
root order, resizing, bit-word boundaries, limits and cancellation. The [physical record](../verification/tight-metal-experiment-20260908/README.md)
retains all four passes and separate host execution coverage. These instrumented
tests establish no GPU speedup or ordinary solver benefit.

The [matched physical measurements](../verification/tight-metal-experiment-20260908/measurements/README.md)
retain 18 cases and 1,080 observations. Resident Metal is faster than fresh Metal
in every case median, but both remain slower than scalar and Rayon checking.
There is therefore no measured range here for selecting this primitive
automatically during ordinary solving. The qualified primitive and its evidence
remain useful for larger, more varied workloads and further execution work.

## Why the current comparison uses different membership algorithms

The physical matrix records 53 corpus cases using CPU `tight_support`; explicit Metal uses the general countermodel route. This follows directly from [`prepare_certificate`](../../crates/zetesis-cli/src/countermodel.rs#L130): certificate preparation requires `oracle=auto` and backend CPU or Auto. An explicit Metal request skips it. [`Execution::gpu`](../../crates/zetesis-cli/src/formula_execution.rs#L116) always constructs the general GPU formula oracle with exact CPU residual completion. There is no evidence that these theories cease to qualify on Metal: the policy does not attempt that check.

Removing the policy guard alone would not provide device support checking. [`batch_step`](../../crates/zetesis-sat/src/batch.rs#L175) calls the injected GPU checker before `certify_pending`, which applies the CPU certificate only to residuals. General GPU propagation would still be paid first. There is also an accounting hazard: committed nonresidual verdicts feed `batch.propagated`, which the CLI exposes as [`gpu_decided`](../../crates/zetesis-cli/src/formula_execution.rs#L166). CPU certificate successes after GPU residuals would need separate provenance before that combination could be reported honestly.

## The semantic certificate already has a reusable library boundary

[`TightPlan`](../../crates/zetesis-ferraris/src/tight.rs) owns a checked complete ground `Theory`, producers and positive ranks, with read-only accessors. Its compiler checks every original root and the accepted producer grammar; a source classification can suggest eligibility but cannot replace this check. The certificate supports ordinary atomic producers and atomic choices with admitted bodies and constraints. Unsupported shapes or positive cycles have typed refusal paths.

[`check_accounted`](../../crates/zetesis-ferraris/src/tight/evaluate.rs) performs four operations for a candidate: evaluate the original DAG; check all original roots; mark every head whose producer body is true; check that each present atom is supported. Passing these checks returns `Stable`. A false original root returns `NotModel`; failed support returns `Residual`, preserving exact reduct completion rather than inventing a countermodel. The plan is tied to the immutable theory identity, and interruption or exhausted work cannot produce acceptance.

These conditions are independent of CPU versus GPU execution. They also preserve the reduct as the semantic foundation: ranked support discharges the proper-subset reduct obligation for a certified class. This is not yet a certificate for an incomplete lazy source registry; such use requires an additional source-completion/coverage argument.

## Smallest useful experiment

The implemented first slice is a bounded library primitive for batched checking of an existing `TightPlan`, exercised outside ordinary backend selection. Compile the plan once on the host and feed the same ordered candidate occurrences to scalar `TightPlan::check_accounted`, a Rayon execution of that operation, and a Metal implementation. Retain exact CPU completion for every residual. Use candidates before membership filtering, including nonmodels and unsupported candidates, not only already accepted answer sets.

The existing [`formula.wgsl`](../../crates/zetesis-wgpu/src/formula.wgsl#L64) provides a useful starting point: one workgroup per candidate, packed candidate input, topological original-DAG evaluation by lane zero, and cooperative root checks. A dedicated support kernel could reuse those operations and add producer-head support reduction plus a present-atom support scan. It would omit the general proper-subset domain construction and repeated propagation sweeps. Reuse the surrounding resident buffers, identity checks, bounded transport and ordered readback where their contracts fit. Account for new producer/support storage explicitly.

The current [`bitwise.wgsl`](../../crates/zetesis-wgpu/src/formula/bitwise.wgsl) packs eight truth-table relation rows; its bits are explicitly not candidate lanes. It is not an existing tight-support kernel. Nor should the static closure kernel be assumed to implement arbitrary admitted formula producer bodies without a separate exact lowering argument.

First establish verdict and witness parity, then measure fresh and resident batches separately. Record certificate setup, original evaluation, support checking, transfers, device submissions, exact residuals, elapsed time and logical memory. Tiny batches may still favor CPU. A subsequent end-to-end comparison should keep the grounder, outer candidate generator, objective handling, output, batch policy and semantic specialization matched. The current physical CPU/Metal ratio does not isolate this proposed comparison.

## Integration and correctness obligations

- Select the semantic membership strategy independently of its executor in the library. Keep the CLI as composition, with explicit device selection, fallback and failure behavior. Record certificate, device propagation and exact CPU completion separately; never derive device decisions from undifferentiated accepted verdicts.
- Preserve every original root, equality and choice activation. Device support must mean truth in this candidate, not possible support. Compile against the complete immutable theory and reject foreign identities.
- Preserve occurrence ordering, retries and exactly one accounted result per input. Exercise repeated candidates, empty and irregular batches, tail sizes 31/32/33 and 63/64/65, stale epochs, exact/one-short capacities, cancellation and interrupted commits. Partial work never yields a stable receipt.
- CPU early exits and parallel device scans can have different work charges. Define and test both bounded contracts instead of silently reinterpreting existing limits. Keep optional certificate refusal distinct from terminal shared work exhaustion.
- Preserve deterministic witnesses where the API promises them: the CPU chooses the first false root in original root-list order, not necessarily the smallest root node identifier, and the first unsupported atom in ascending order.
- Compare with independent whole-formula and frozen M/J semantics on tiny theories, in addition to scalar/Metal agreement. Existing [`tight.rs` tests](../../crates/zetesis-ferraris/tests/tight.rs), certificate accounting and batch interruption tests supply relevant controls. Matching implementations alone cannot exclude a shared semantic error.

## Lean scope

[`TightPlans`](../../proofs/Zetesis/TightPlans.lean#L187) already proves ranked-support stability; its equivalence theorem states the additional coverage hypotheses. [`CertifiedExecution`](../../proofs/Zetesis/CertifiedExecution.lean) handles sound certificate verdicts, exact residual completion and interruption. [`BatchAccounting`](../../proofs/Zetesis/BatchAccounting.lean#L175) supplies the composition boundary for complete occurrence accounting.

[`TightEvaluation`](../../proofs/Zetesis/TightEvaluation.lean) now connects ordered Boolean DAG evaluation to unfolded original-formula truth, indexed producer reduction to support, and ranked classification to exact residual completion. Its eleven laws compose with the existing stability and execution results. Rank extraction, complete source coverage, Rust/WGSL representation, barriers, readback and resource accounting still require explicit implementation correspondence; the current proofs do not constitute verification of a Metal kernel or the entire solver.

**Next step:** physically qualify and measure the implemented same-certificate experiment before changing ordinary Metal dispatch. It isolates the missing primitive and prevents both a misleading algorithm comparison and misattributed GPU work. Lazy certificate execution remains a separately justified extension.
