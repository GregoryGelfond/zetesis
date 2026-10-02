# Refining formula evaluation

The separately built [implementation-refinement package](https://github.com/GregoryGelfond/zetesis/tree/main/refinement/evaluation)
connects the actual generated private `FrozenReduct::satisfied_by` query to
Ferraris reduct satisfaction under fixed observation tokens. It composes the
proved evaluator and root scan, retaining an explicit stored-mask invariant.

## From evaluation to theory satisfaction

Let `M` be the stored candidate, `J` any tested interpretation, and `T` the theory
asserted by the stored roots. The central result is:

```text
Represents frozen: its stored mask is original node truth at M
completed actual private satisfied_by query at J → Boolean answer

answer = true  iff  J models the Ferraris reduct of T frozen at M
```

[`FrozenQuery.completed_satisfaction`](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/evaluation/FrozenQuery.lean)
requires this mask agreement, represented packed input, ordered children and
bounded roots. It derives the two internal calls from the actual query result,
then establishes their meaning and read bounds. `represents_from_evaluation`
derives the mask agreement for the corresponding record from completed original
evaluation; it does not verify `FrozenReduct::freeze` or its allocation.
The existing `TheorySatisfaction` results cover separate original and reduct
calls. No candidate modelhood or tested-subset premise is required; satisfaction
does not establish answer-set membership.

The query retains one work record across evaluation and root checking. For `N`
nodes and `R` root occurrences, every typed return charges at most `N + R`,
without resetting prior work. Limits and subset statistics are preserved.
An evaluation stop skips the root scan; a root-scan stop remains the same error.
The argument composes these boundaries:

| Boundary | Argument |
| --- | --- |
| Setup | The generated entry operations produce a zero cursor and empty output |
| Packed membership | The extracted atom query returns the stored bit |
| One node | Actual reads and Boolean branches append exactly the masked truth |
| Work and stopping | Each continuation charges one unit; a stop preserves the prior prefix |
| Loop | Actual returned-control threading gives a terminating, trace-correct outcome under fixed tokens |
| Reduct | Two successful generated evaluations compute explicit Ferraris reduct truth |
| Roots | The actual scan preserves root order, duplicates, first-failure identity and exact work |
| Theory | A completed scan returns no failed root exactly when the tested interpretation models the asserted theory |
| Stored query | The actual method uses the represented mask and shares evaluation's returned work with the root scan |

An exhausted iterator completes without polling or charging a node. With a node
present, the iterator fetches it before polling; cancellation, deadline and work
refusal precede its evaluation and append. A typed stop leaves a strictly shorter
correct prefix. `FixedLoop` proves that the generated function terminates under
its input invariants and fixed-token model; termination may be such a refusal,
not a successful evaluation.

The root scan charges every tested occurrence, including duplicates and the
first false root. It returns that root's node identifier, not its list position
or the smallest false identifier. Empty roots complete without polling or work.
A typed stop certifies only the preceding true occurrences, not satisfaction
or rejection of the whole theory. Checked
[examples](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/evaluation/RootScanExample.lean)
exercise empty and repeated-root scans. [Query examples](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/evaluation/FrozenQueryExample.lean)
cover cumulative work refusal, a nonmodel candidate with a valid mask, and a
successful tested non-subset.

## Reusing the ASP library

The package imports the existing `ReductEvaluation` sources and their dependency
closure directly from the general library. Those eleven modules compile unchanged
under both the library's Lean 4.33.1 and the extraction package's Lean 4.31.0.
The bridge is checked in the latter toolchain with its own audit; it does not mix
object files from different versions or duplicate the semantic definitions.

A structural conversion maps extracted node constructors and machine indices to
the library's formula DAG. Fold correspondence then connects completed generated
calls and their asserted roots to the existing reduct theorem. The result is a
checked connection to the same mathematical theory used elsewhere in the library.

## Remaining implementation boundary

`FixedLoop` follows the generated loop's control threading: repeated token reads
return the same value. The broader authored `Trace` relation permits separately
supplied controls between body calls. Fixed-loop calls form a trace, but the
[checked cancellation example](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/evaluation/FixedLoopExample.lean)
shows that a refreshed trace can stop after one node while the fixed-clear loop
completes two. The pinned extraction effect cannot express changing atomic read
results, so correspondence with concurrent Rust histories remains open.

The calls share stored nodes, roots and numeric atom vocabulary. `FrozenReduct`
construction and its public allocation and owner-checking wrappers remain
unproved, as does subset search. Allocation, reference counting, timers,
concurrent memory, machine code and GPU execution remain outside these models.

The [package guide](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/evaluation/README.md)
records the exact scope, source hashes and reproduction commands. Generated code
comes from production Rust. Two recorded local-name adjustments avoid generator
namespace collisions; exported destination metadata uses a portable path. An
unreferenced derived `Debug` implementation and its ordered registration are
also removed after an explicit reference check. This reversible, audited
selection leaves executable bodies unchanged. Translation tools, preprocessing
and model correspondence remain trusted. The [correctness plan](correctness.md) keeps these limits separate
from whole-solver soundness and complete enumeration.
