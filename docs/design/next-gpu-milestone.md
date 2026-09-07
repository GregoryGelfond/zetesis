# Next GPU milestone after physical formula qualification

Current checkpoint status: ordinary M4 Pro Metal solving passes all 94 original
corpus cases, and all four focused CLI/formula hardware tests pass. The matched
540-row scalar/Rayon/Metal campaign is complete: Rayon is fastest by warm median
in 20 shapes, scalar in 10, Metal in none. The functional integration is qualified
within these bounded tests. The per-phase, actual-stream performance acceptance
item below remains open; the synthetic membership run does not satisfy it.
The subsequent readiness review records the remaining assurance and measurement
obligations rather than claiming full GPU acceleration or deployment readiness.

The 2026-09-06 M4 Pro formula campaign establishes physical execution and exact
hybrid parity for the resident propagation primitive. The subsequent implementation
integrates that primitive into ordinary explicit GPU formula solving. Its joint
milestone is **batched reduct checking in the normal solver**, with a matched
parallel CPU reference and complete accounting of pending candidates.

## Why this step

The dated Metal campaign has a serial CPU reference and patterned synthetic
candidates. Metal was faster in two 256-candidate choice cases (paired median
ratios 2.44× and 2.61×), while CPU was faster in 28 of 30 cases. This points to
batch size and candidate structure as important, but is not enough to select a
production performance policy. Dispatching one candidate at a time would ignore
the measured batching behavior.

The new benchmark includes a reusable Rayon membership pool and records effective
resource limits. Its matched physical campaign is now complete; the older
two-backend results remain a separate historical measurement. Metal beats scalar
by paired median in three of the 30 new shapes (one is a near-tie), but does not
beat Rayon in any. This does not establish a useful automatic GPU crossover.

The ordinary formula solver already has a persistent native candidate cursor,
verified original interpretations, exact reduct checking, objective feedback and
bounded enumeration. Its actual membership boundary is before a candidate is
accepted as stable. Running GPU checks after the public stable-model iterator
returns would duplicate verification. Replacing the cursor with powerset
sampling would discard useful existing work and completeness structure.

## Deliverable

A normal invocation such as `zetesis problem.lp --backend metal --stats` should
use the general GPU propagation stage for supported formula programs. It must
initialize its adapter, queue and pipelines within the invocation, reuse the
immutable theory and transport where compatible, and report the actual split
between GPU propagation and CPU residual completion. No setup script, benchmark
command or installation-time pass marker is a prerequisite.

The first integration is explicitly hybrid. Exact GPU residual search, GPU outer
candidate generation, objective processing and relational grounding remain later
stages; this milestone must not be labeled full GPU acceleration.

## Bounded execution protocol

1. Separate bounded candidate proposal from membership checking and result
   commitment, retaining the current cursor and exact semantic blocking. Every
   proposed but uncommitted candidate remains explicitly accounted for.
2. Send a batch of original classical-model candidates to one resident GPU
   oracle. Freeze original truth for each candidate before reduct propagation;
   objective bounds never constrain the inner proper-subset query.
3. Accept only a checked no-proper-subset result. Complete every residual with
   the existing exact native membership routine. An original-model rejection
   from this model-only producer is an invariant failure to investigate, not
   an ordinary discarded candidate that hides a producer/oracle mismatch.
4. Commit results and objective feedback without losing candidates or optimal
   ties. Delayed objective feedback may cost work but cannot change the answer
   sets. Cancellation, budget exhaustion, device errors and failed blocking
   preserve explicit incomplete coverage; an empty device queue is not evidence
   of complete enumeration.
5. Keep backend policy outside the semantic candidate/checking API. The core
   search crate should not gain a wgpu dependency. Automatic selection may retain
   CPU when device initialization or measured workload economics favor it;
   explicit GPU requests report failures honestly.

A single-candidate hook may be useful as an internal correctness scaffold, but
batching and pending-candidate accounting are required to complete this milestone.
The first GPU stage should not rerun membership on already verified stable models.

## Acceptance evidence

- Complete native and hybrid parity for all 94 currently admitted original
  non-clingcon corpus cases, including full reported model counts, displayed
  model multisets, objective vectors and tied optima. Independent small formulas
  also check hidden semantic identities, which `#show` output cannot recover.
- Adversarial tests for partial batches, cancellation, budget boundaries, delayed
  failures, stale epochs, output errors and objective updates with pending work.
  No failed or unresolved batch can become complete coverage or an optimum.
- Lean contracts for proposal/check/commit accounting, completion-preserving
  feedback and the existing frozen-reduct acceptance boundary. Explicit encoding
  assumptions remain visible; semantic laws do not establish device refinement.
- Reproducible end-to-end and per-phase measurements from actual candidate streams
  against the retained scalar and Rayon CPU paths. Preserve all failures, initial
  samples and warm samples. Report residual rate, actual worker count, batch size,
  memory/transfer accounting and candidate/checking work with `--stats`.
- Physical Metal checks of the integrated ordinary CLI, including residual-heavy
  and irregular-size cases. The synthetic campaign does not stand in for these.
- Existing strict formatting, Clippy, rustdoc and independent 91% line-coverage
  gates remain in force. themelios stays pinned and unchanged.

Performance is an experimental result, not an acceptance assumption. The goal is
an honest, complete production path and evidence sufficient to decide where it
belongs. If it loses to parallel CPU, retain CPU selection while improving the
measured bottleneck; do not redefine the baseline to manufacture a win.

## Subsequent opportunity

The larger memory-scaling bet remains candidate-driven, persistent relational
construction with exact deltas and bounded GPU joins. That is the route toward
avoiding explosive eager grounding, informed by the GPU Datalog review and
program/domain analysis. It should follow a usable, measured candidate-to-oracle
pipeline so grounding changes can be assessed on complete solves with the same
acceptance and coverage contracts. General CPU improvements can continue where
profiling finds material costs; the recent sub-percent-to-low-percent timing
shifts alone do not establish them as the next major architectural step.

## Aggregate parity and qualification

Source admission has been extended from one to multiple independent
aggregate equality binders per rule. Each bound target must remain independent
of other aggregate generators' tuple/condition evaluation; safe references in
non-binding comparison aggregates preserve their previous behavior. The original
aggregate formulas and full-tuple coalescing remain the semantic foundation.
Dependent generator scopes and objective-relevant multi-assignment producers
remain separately tracked limitations.

The GPU integration, aggregate extension and scalar/Rayon/hybrid benchmark form
one integration checkpoint. After their regression, external-oracle, coverage,
formal and physical checks, a comprehensive audit will assess source compatibility,
semantic trust boundaries, candidate accounting, API/CLI behavior, actual CPU/GPU
performance, coverage quality and outstanding implementation/refinement gaps.
New implementation alone is not completion of that checkpoint. The
[production assurance audit contract](../verification/audit-contract.md) extends
the review beyond parity and coverage to predictable resource behavior, recovery,
human comprehension, API design, release assurance and a concrete path toward
mission-critical use.
