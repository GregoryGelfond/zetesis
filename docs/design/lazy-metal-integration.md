# Lazy source grounding with physical Metal

This is an implementation proposal, based on source `8ce3191` and a read-only
review of the dependency tranche's ordered lazy joins. Nothing below is a
qualified route or a performance result. The first useful deliverable should
support the existing admitted relational `Program` profile with host source
joins and device-owned per-candidate consequence steps. General lazy Ferraris
formulas require a separate design. No themelios changes are needed for the
relational slice.

## Existing boundaries

| Source seam | Implemented behavior and consequence |
| --- | --- |
| `zetesis-cpu/src/oracle.rs::check`, `visit`, `guards` | Each seed independently rebuilds relations from its current closure, joins positive source patterns, checks filters and frozen gates, derives a delta, and repeats through a complete no-delta round. A CPU-completed closure sent to the device would duplicate the already decided membership work. |
| `zetesis-cpu/src/batch.rs::BatchOracle::check_batch` | An owned Rayon pool checks a fixed seed slice in input order. Work/derived-atom limits are per candidate; cancellation/deadline are shared. There is no reusable source-instance cursor or shared-world closure protocol yet. |
| `zetesis-core/src/candidate.rs::Seed`; `zetesis-cpu/src/candidates.rs` | Seeds retain exact `Program` identity and sparse true gate atoms; absence means false. The candidate cursor yields the empty seed before requesting a carrier tuple and retains exhausted versus stopped termination. Preserve that property. |
| `zetesis-core/src/ground.rs::GroundProgram::compile` | Materializes the complete carrier and all substitutions, then dense rules. Calling it before device dispatch is eager grounding even if earlier seeds used lazy CPU joins. |
| `zetesis-wgpu/src/lib.rs::GpuOracle::check_batch`; `packing.rs::GraphPlan` | Requires a complete `GroundProgram`; graph identity and all atom/rule dimensions are sealed before packing. Its 4,096-atom workgroup closure limit is a static-profile bound, not an existing dynamic-catalog API. |
| `zetesis-wgpu/src/oracle.wgsl` | A workgroup owns one world's closure. It evaluates exact frozen true/false gates and positive antecedents, atomically derives heads, checks constraints and compares the gate projection with the seed. These are useful primitives, but the kernel assumes an immutable complete rule array and resets closure on every call. |
| `zetesis-wgpu/src/runtime.rs` | Reusable adapter selection, capability checks, validated pipeline, error scopes, device-health invalidation, submission and bounded readback. Reuse this lifecycle; do not create a second transport implementation. |
| `zetesis-cli/src/engine.rs::validate_combination`, `Engine`, `Executor` | Explicit lazy GPU requests are refused; lazy Auto stays CPU without discovery. Other Auto requests start on CPU and may replace it after a batch of 32 with a GPU executor that calls `compile_static`. This is an eager switch, not lazy GPU operation. |
| `zetesis-cli/src/closure_session.rs`; `session.rs` | Candidate generation, complete checked results, ready publication and pending stops are separate. A new executor must return results for the same submitted seed occurrences, without manufacturing exhausted search status. |
| `zetesis-cli/src/formula_execution.rs`; `zetesis-wgpu/src/formula/*` | Eager Ferraris DAG propagation plus exact CPU residual completion is a different oracle. Neither a partly emitted formula DAG nor this normal-rule closure kernel establishes its general reduct semantics. |

## Minimum honest device path

Use synchronized snapshots and bounded instance chunks. Initially every world's
closure is empty; its frozen seed is separate and is never copied into the
derived relation. For round r, form the union U of the complete per-world
closures C[w,r]. Host joins enumerate filter-valid source instances over the
immutable U, retaining their whole positive, true-gate, false-gate and head atoms.
No host per-world gate test or head derivation decides these instances.

The union can combine tuples from different worlds. Therefore the device must
check **every positive atom against that world's C[w,r]**, as well as its gates
against the immutable seed. Union membership is not per-world positive truth.
An enabled instance derives its head into a separate next-delta bitset; an
enabled constraint sets a per-world violation flag. All chunks read the same
round snapshot, so neither chunk order nor lane scheduling changes this step.
Facts and constraints without positive antecedents are included on every scan.

Only after the host cursor reports exhaustive traversal of every source
template for this snapshot, every chunk completes, and the complete readback is
validated may the next delta be committed. Newly true atoms enter the next host
union; catalogued but underived atoms do not. Continue while any world grows.
A completed no-delta scan supplies final headed and constraint coverage for
every world. Compare each final closure's gate-carrier projection with its exact
seed before returning a membership result. A seed atom never reached by any
source instance must still participate in this comparison.

This performs genuine per-world device work: enablement, positive membership,
head insertion, constraints and closure growth. Host work is parsing/admission,
symbolic joins, deterministic filters, packing and snapshot management. It is
neither all-device grounding nor a device validation of a CPU-finished answer.
Reading deltas or complete closure bits each round is acceptable for the first
slice; record that transfer explicitly and make no residence or speed claim.

## Three implementation steps

1. **A reusable bounded source-instance stream and portable round specification.**
   Extract the positive join/filter cursor from `zetesis-cpu/src/oracle.rs` into
   a capability module used by the existing CPU oracle and the new coordinator.
   Retain the storage-order prefix windows, complete binding checks, rollback,
   literal order and exact value identity. Keep frozen gate evaluation outside
   this stream; the CPU consumer applies its existing gates, while the device
   consumer receives grounded gate lists. Existing CPU early-gate pruning may
   remain an explicit consumer policy, rather than disappearing accidentally.
   Proposed public concepts are `SourceInstances`, `SourceSnapshot`, bounded
   instance chunks and retained `Exhausted`/`Stopped` completion. Constructors
   must bind the program, snapshot and atom catalog, not accept an arbitrary
   caller-supplied “complete” flag. A portable executor over these chunks checks
   the protocol before adding a device implementation.

2. **An exact incremental device executor.** Add `zetesis-wgpu/src/lazy/` and a
   distinct shader profile through the shared `Runtime`. Proposed `GpuLazyOracle`
   accepts immutable source-program identity, the fixed seed batch, explicit
   limits and control; it does not accept `GroundProgram`. Preserve the static
   oracle and its shader as the baseline. Share small checked ABI and boolean
   primitives where useful, but do not mislabel the existing reset-and-complete
   static kernel as incremental. The host coordinator can use the CPU source
   capability through a normal library dependency; `zetesis-cpu` depends only on
   core/Rayon, so this adds no dependency cycle. Keep new instance data types in
   that capability or core only when both consumers need them, without a new
   general-purpose framework or crate solely for packaging.

3. **Explicit ordinary lazy Metal, then a measured Auto policy.** Add a `LazyGpu`
   executor in `engine.rs`, preserve `ClosureSession`'s fixed candidate order and
   pending-stop accounting, and admit `--backend metal --grounder lazy` for the
   relational profile. Replace the blanket backend restriction with a truthful
   profile capability check; formula admission's explicit lazy refusal stays.
   Preserve the current Auto heuristic until actual lazy-device measurements
   justify changing it. If Auto later retries an unpublished failed batch, it
   must retain consumed work and the same seeds; an explicit Metal request must
   not silently become CPU or complete static lowering. Report effective lazy
   grounding, host joins, actual Metal execution, dispatches and round readbacks.

The first step is independently useful for testing source exhaustion and sharing
the current join algorithm. The second and third together are the minimum route
that can satisfy the requested physical lazy-Metal milestone.

## Identity, bounds and interruption

Use exact `Atom`/`Value` identity: predicate name, arity and strong sign plus the
whole ordered tuple, including constructor signs. Local source variable IDs and
tuple positions are not global atom IDs. Assign dense IDs only as seed or source
instances demand them, keep them stable within a batch, and include all supplied
true seed atoms. Retain the owning `Program` handle to prevent foreign-instance
reuse. A changed seed batch starts empty closures; it must not inherit another
batch's closed worlds. Reuse immutable buffers only under explicit compatible
program/catalog-generation and dimension checks.

Admit checked u32 addresses and products before packing. Bound the discovered
catalog, source cursor visits, completed bindings, round count, rules and
antecedent IDs per chunk, worlds, and both host/device bytes. Account at once for
the symbolic catalog/union, old closure, pending delta, seeds, chunk packing,
device transport, readback and decoded results. Preallocate only bounded bitset
capacity if convenient; do not enumerate the complete atom carrier to fill it.
The 4,096 active-ID ceiling is a possible explicit initial capacity, not an
assumption that every lazy source fits. Refuse on growth before publication.

Source work shared across worlds needs a separate typed batch allowance from
device work per world. Do not give each chunk a fresh quota or imply that CPU's
per-candidate operation counts equal GPU lane work. The current `max_work` help
is explicitly about CPU oracle operations; any new mapping must document its
unit. Checked aggregation must retain completed work on source/device failure.
Semantic results require completion; exhausted resources never mean rejection.

Poll control during host enumeration and before/after bounded device work.
Keep shader loops bounded by chunk dimensions and source-round progress; avoid
an uninterruptible unbounded dispatch. The shared `runtime::read` presently waits
for one submission up to its timeout, so responsive cancellation needs bounded
polling or a bounded wait quantum, without unmapping or reusing in-flight buffers.
Device/timeout/readback failure invalidates the affected lifecycle and returns
no partial batch result. Distinguish a complete world closure, exhausted source
snapshot, exhausted candidate stream, requested-model stop and writer failure.

## Acceptance evidence before promotion

Portable tests should compare exact closures, constraints, gate projection and
candidate outcomes with existing lazy CPU and independently compiled static
closure on the same complete small seed population. Exhaust chunk sizes 1,
boundary-minus-one, boundary and larger-than-stream. Include cross-world tuple
combinations that satisfy no world, missing seed atoms, positive cycles without
facts, duplicate rules/heads, nullary and empty relations, both predicate signs,
structured values, repeated variables, failed joins and newly derived rows that
become eligible only in the next snapshot. Exercise every source/packing/readback
stop boundary, foreign/stale identities, exact ceilings and tail-bit/status
corruption. No result may depend on chunk split or dispatch order. Deliberate
controls should drop the last source chunk, use union truth as world truth, or
mistake a pending queue for final source exhaustion; the semantic tests must fail.

Add a scoped Lean module for union-snapshot coverage, world-isolated consequence
steps, chunk concatenation and final-snapshot completion. Existing
`Lifted.SourceCoverage`, `Lifted.materialized_exact`,
`LiftedBridge.completed_lazy_stage_exact`, `Events` and `BatchAccounting` provide
the vocabulary. The new obligation is that enumeration over U covers each
world's enabled bindings and device checks remove cross-world combinations.
Coverage must be re-established after growth; existing snapshot counterexamples
already show why an earlier exhausted stream does not prove later closedness.
Kernel tests and packing audits remain separate from these mathematical laws.

Validate WGSL with Naga and prepare a bounded engineering command before asking
for physical qualification. On a real physical Metal adapter, run the same fixed
seed occurrences and full original sources through CPU lazy, complete static
CPU (where it fits) and lazy Metal. Require actual device dispatches with nonzero
per-world checks/derivations and exact completed results, not adapter discovery
alone. Include a source whose complete static carrier exceeds its configured
lowering cap while lazy active storage fits; retain actual active/catalog/rule
counts and show that static lowering was never called. Seal source and binary
hashes, adapter/backend, configuration, completion, bytes, rounds, transfer and
raw full-model records. Existing physical static/formula Metal qualification
does not qualify this new protocol.

Only after correctness qualification compare uninstrumented end-to-end time on
sparse and dense joins, varying candidate batch and instance-chunk size. A union
can create expensive cross-world joins, and round transfers may dominate; both
are reasons to measure, not evidence of a speedup. Device-side source joins,
semi-naive storage redesign, cross-batch source caches, dynamic Ferraris DAGs,
general aggregate/conditional lazy evaluation and automatic crossover changes
remain separate follow-ons.
