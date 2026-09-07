# Aggregate bounds and lazy Metal: the next tranche

Status: proposed implementation sequence following the qualified `2c05ea7`
runtime checkpoint. This document does not claim that the new capabilities have
been implemented. The proposed scope excludes `#heuristic` and `#edge`, as
well as theory atoms/terms and Python/Lua scripting. Their explicit refusals
remain intentional; historical observations are preserved.

The reduct remains the acceptance foundation. Candidate pruning, specialized
membership checks and hardware execution must preserve original-program truth,
frozen-reduct stability and complete search accounting. Parsing, source identity
and I/O remain on the host through the existing themelios boundary.

## Why these priorities

The [current ordinary CPU comparison](../verification/dependency-performance-20260907/README.md)
records native/clingo medians of 41.021/12.779 ms for SEND, 105.307/120.946 ms
for queens02, and 131.262/194.286 ms for task04. All three select eager grounding
and tight-support membership. The separate single phase observations identify
SEND rule instantiation and candidate generation as substantial costs; queens
and task allocation are dominated by solving. These observations locate work,
not the cost of individual allocations or a general solver ranking.

The [ordered-window experiment](../verification/ordered-joins-20260907/README.md)
shows a separate lazy CPU oracle improvement. It is not exercised by those eager
cases. Choice bounds already become guarded count constraints and threshold
formulas used during candidate propagation. Wider language admission and stronger
bound exploitation are distinct contributions and require separate measurements.

## Deliverables

Each semantic slice includes its Lean obligations and explicit remaining
implementation correspondences. Validation-tool composition, integration,
documentation and independent review accompany the implementation.

### Language: aggregate consumers, choice bounds and count heads

Use a sequence of independently qualified frontend changes:

1. Admit aggregate-produced values in positive evaluated arguments through the
   existing capture-and-check path. For example, `p(1).q(N):-N=#count{},p(N+1).`
   should retain the original aggregate equality and supporting `p(1)` atom.
   No inverse arithmetic or new source join engine is required for this slice.
2. Admit independent aggregate assignments as integer choice bounds. A motivating
   source is `d(1).N{a;b}N:-N=#count{X:d(X)}.` Keep the original equality and rule
   activation condition. A proposed aggregate value neither establishes its
   actual value nor activates the bound unconditionally.
3. Broaden positive `#count` head eligibility to positive ordinary atoms, while
   retaining the current per-instantiated-group tuple/atom bijection. For example,
   `q(1).1#count{X:p(X):q(X)}1.` needs local binding and retained eligibility.
   Reuse the supported conditional-choice machinery only with a preservation
   argument. Do not simply remove the static-condition refusal.

For count heads, preserve complete tuple keys, atom identity, original eligibility
formulas, duplicate coalescing and support conditions. Possible-support membership
is not truth. Validate the complete generated group before publication; a partial
enumeration cannot establish its alias or coverage properties. Include recursive
eligibility, source-order permutations, empty groups, repeated rows and exact
resource stops in the independent original/frozen `M/J` comparisons and clingo
regressions.

General `#sum`, `#sum+`, `#min` and `#max` heads remain high-priority subsequent
slices. Weighted/extremal semantics, relaxed tuple aliases and negative head or
eligibility forms must receive their own semantic design and evidence. This
tranche does not promise all general head aggregates at once.

### CPU: test reusable eager-evaluation scratch

The current flat expression evaluator creates a new value vector for each
evaluation. SEND's separate diagnostic records 29,344 expression evaluations and
137,344 expression nodes during rule instantiation. This identifies repeated
scratch construction; it does not establish that allocation dominates the phase.
Atom and formula-node lookup costs are competing hypotheses.

First collect bounded scratch-growth/allocation evidence without per-node clocks.
Then test a private scratch owner local to an admission or worker, preserving
the existing flat expression plan and checked scalar operations. Preserve operand
order, readiness, deferred versus final-row errors, source locations, live-value
cleanup, logical charges and ordered emitted subjects. Bound retained capacity
and distinguish it from existing copied-payload accounting.

Use sealed, identically configured before/after builds for unobserved admission,
boundary-only profiling and detailed attribution. Compare complete models,
ordered-subject fingerprints, counters, typed failures and exact-limit behavior.
Include SEND, queens and small constructor/error controls; the objective-free
grounding profiler does not silently expand to cover task-allocation objectives.
Keep task allocation in the ordinary process comparator. Retain the optimization
only if repeatable measurements justify it without a material control regression.
Candidate-search optimization is a separate follow-on, guided by its own profile.

### GPU: useful ordinary lazy Metal execution

Implement the admitted relational profile through three ordered milestones from
the [lazy-Metal design](lazy-metal-integration.md):

1. A reusable bounded source-instance stream and an exact portable round executor.
2. Per-candidate device enablement, consequences, constraints and closure growth
   over immutable round snapshots, using the shared wgpu runtime.
3. Ordinary `--grounder lazy --backend metal` integration, with explicit effective
   routing and measured device work.

The source stream must retain program/snapshot identity, atom identity and
exhausted versus stopped traversal. A union of different candidate closures can
offer instances, but the device must still check every positive antecedent in
the individual candidate's snapshot. Commit a round only after all required
instances and chunks complete. Retain frozen seeds, exact candidate accounting,
bounded catalog/chunk/transport storage and explicit failures.

The first honest route keeps source joins on the host and performs useful
per-world inference on Metal. Sending already completed CPU closures to the
device, or switching to complete static grounding, does not meet this milestone.
Keep automatic-selection policy unchanged until measurements justify a change.
General lazy Ferraris formula grounding remains a separate capability.

Portable protocol checks precede physical qualification. Compare complete results
with CPU and the applicable external oracle; exercise cross-world contamination,
irregular batches, catalog growth, source exhaustion, resource stops and device
failure. Record actual dispatches, transfers, host waits and device identity.
No physical performance conclusion follows from compilation or portable tests.

### Tooling and clarity: finish the selected Rust campaign

Compose the new bounded process and native-JSON/report libraries into the
selected-upstream comparison campaign. Preserve all 24 exact source/helper
contracts and 73 full-model occurrences, original bytes, source/binary seals,
report-publication protections and typed stopped/failure evidence. Exercise the
campaign's failure paths, not only the successful collection.

Switch a legacy consumer only after the Rust replacement has equivalent evidence.
Retire only scripts/import scaffolding made unused by that switch; the rest of
the Python/C++ migration remains explicit. Keep tooling under the same API,
naming, resource and review standards as the solver. Update the implementation
map, compatibility matrix and README to reflect integrated capabilities.

## Integration and formal obligations

Aggregate assignment and head IR changes share frontend interfaces with
evaluator/scratch internals, especially `formula_support.rs`. These interfaces
must remain consistent. The GPU source-stream work lives at the relational
core/CPU/wgpu boundary and must preserve the existing CPU oracle as a checked
consumer.

Lean work is limited to foundations needed by these changes: aggregate proposal
and eligibility composition, guarded bound-pruning soundness, evaluator prefix
invariants, source coverage and per-world frozen-reduct rounds. State complete
carrier/extraction assumptions and the unproved Rust/WGSL bridges explicitly.
Maintain reusable logical definitions, the pinned build and full axiom audit.
Theorem count and proof-library size are not acceptance criteria for the tranche.

## Checkpoint criteria

- Complete all three bounded language slices with original/frozen semantic
  comparisons, independent clingo regressions and unchanged neighboring refusals.
- Reach an evidence-based decision on the CPU scratch experiment; a negative
  result is retained without shipping unjustified complexity.
- Qualify the GPU milestones separately; ordinary lazy Metal requires physical
  execution evidence before being described as supported. Report any unfinished
  stage explicitly rather than claiming the whole route from the source stream.
- Complete the selected Rust campaign without weakening existing contracts.
- Retain strict formatting, pedantic Clippy, strict docs, both independent 91%
  coverage floors, the 94-case corpus, selected upstream suite and Lean audit.
- Refresh matched performance data and phase observations after integration.
  Preserve failures, unsupported combinations and raw samples. Keep the required
  [full eager/lazy CPU/Metal matrix](corpus-performance.md) distinct from limited
  comparisons. Source-parameter overrides and queens size sweeps remain queued.

At the checkpoint, take stock of supported language, measured costs, actual
device use, reusable library boundaries and proof correspondence before choosing
further general head-aggregate or optimization work.
