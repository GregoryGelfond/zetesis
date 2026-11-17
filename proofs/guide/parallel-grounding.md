# Ordered parallel grounding and witness replay

The mathematical unit is an ordered occurrence stream over a fixed immutable
source. Concurrency changes who traverses each block, while the coordinator
retains the original publication order. These laws do not select an execution
default, a worker count or a resource allowance. The corresponding CPU routes
are experiments; conditional equations are not a proof of the Rust executor.

## Partition first, publish in order

[ParallelGrounding](../Zetesis/ParallelGrounding.lean) defines `rowTasks` by
successive take/drop operations and a final remainder. `row_tasks_reconstruct`
shows that flattening the blocks returns every original occurrence in order.
Zero-width blocks, duplicate rows and rows that produce no output are retained.
`ordered_task_outputs_exact` lifts that equality through a total emitter;
`completed_task_results_exact` permits a task implementation only under its
explicit completed-result equality premise. This premise is an obligation,
not a proof that a concrete task implements the emitter.

`task_count_includes_empty` separates task completion from output count.
The fixtures demonstrate preserved duplicate outputs and the failure of
completion-order publication. A worker's successful empty result is not a
missing result. Typed failures, cancellation, panic handling, grants and receipts
are outside these list equations and require the concrete ordered-result
protocol. The list model also does not prove that snapshot-based pending-head
skips and serial per-row publication visit identical internal witness streams.

## Transport the witness, keep its owner

[WitnessReplay](../Zetesis/WitnessReplay.lean) retains an unchanged prefix and
transports the changed suffix. `unchanged_prefix_reconstructs` requires the
actual prefix equality; `unchanged_projection_exact` then presents the same
complete witness to the same mathematical projection. Numeric row coordinates
from unrelated relation owners do not meet that premise.

`task_boundary_reset` discards dependence on a previous task's last row.
`unchanged_completion_retained` preserves a completion with no changed suffix,
and `unchecked_prefix_is_unsound` shows why length checks alone are insufficient.
Rust must retain the original source, plan, relation and canonical identities,
reconstruct the exact binding, and use the existing canonical writer once in
ordered publication. The theorem does not model that writer's effects.

## A fully bound singleton prefix

[SingletonPrefix](../Zetesis/SingletonPrefix.lean) composes the partition law with
one unchanged prefix binding. `singleton_prefix_outputs` preserves ordered
outputs; `rejected_prefix_outputs` covers a failed prefix. The counterexample
`multiple_prefixes_do_not_commute_with_blocks` excludes exchanging an arbitrary
multi-binding prefix with the block loop.

The concrete classifier must establish a fully bound test and confirm the
single offered row through the existing matcher. Offered-row cardinality alone
does not show that the pattern matches. An unbound one-row generator is outside
this classifier. Each worker replays the original prefix with the same matching
and charging operations; the law grants no permission to skip validation.

## Independent producer deltas are a separate question

The remaining ParallelGrounding laws concern predicate sets, not occurrence
lists. `ReadsOnly` must contain every state dependence, `WritesOnly` every
possible output, and both cross read/write intersections must be excluded for
`independent_deltas_merge`. Shared writes may coalesce as a set; this does not
preserve source origins or multiplicity. `covered_delta_merge_exact` additionally
requires the existing DeltaRounds coverage and published-history premises.
These laws are groundwork, not a certificate for the current producer ready
queue or an implemented component-wave scheduler.

`unfinished_dependency_blocks` and `starting_cannot_create_readiness` require
completed prerequisite publication. Queue insertion or a running worker is not
completion. These equations establish neither visibility nor liveness, and
prove no fixed-point termination, reduct membership or GPU execution.

[Selected comparisons and retained rows](producer-row-reuse.md) concern separate
redundant work inside source Activity. Warm retained rows must still be consumed
against the current Activity; parallel transport supplies no truth-cache license.
The [manual correspondence](../../docs/book/lean/correspondence.md#ordered-parallel-grounding-and-witness-replay)
lists the concrete implementation obligations and evidence boundary.
