# Refining formula evaluation

The separately built [implementation-refinement package](https://github.com/GregoryGelfond/zetesis/tree/main/refinement/evaluation)
connects actual generated evaluation and root scanning to original and Ferraris
reduct theory satisfaction under fixed observation tokens. The original evaluation
computes the frozen mask; its correctness is derived from that call.

## From evaluation to theory satisfaction

Let `M` be the original interpretation, `J` any tested interpretation, and `T`
the theory asserted by the stored root list. The central theorem establishes:

```text
successful generated evaluate call at M
    → originalValues
successful generated evaluate call at J masked by originalValues
    → reductValues
completed generated failed_root call over reductValues
    → answer

answer = None  iff  J models the Ferraris reduct of T frozen at M
```

The calls address the same stored nodes and roots; both evaluations clear their
old output. Premises require successful call results, enough packed words,
children referring to earlier nodes, and roots within the stored node count.
The proof derives mask truth, truth-table length and every root-read bound.
A corresponding theorem covers original-theory satisfaction. Neither result
requires `J` to be a subset of `M` or establishes answer-set membership.

The [formal statement, `TheorySatisfaction.completed_reduct`](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/evaluation/TheorySatisfaction.lean)
composes these checked boundaries:

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
exercise empty and repeated-root scans.

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

The calls share stored nodes, roots and numeric atom vocabulary. The `FrozenReduct`
struct and wrappers, owner checks and subset search remain unproved. Allocation,
reference counting, timers, concurrent memory, machine code and GPU execution also
remain outside these library models.

The [package guide](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/evaluation/README.md)
records the exact scope, source hashes and reproduction commands. Generated code
comes from production Rust. Two recorded local-name adjustments avoid generator
namespace collisions; exported destination metadata uses a portable path. These
changes preserve executable operations. Translation tools and model correspondence
remain trusted. The [correctness plan](correctness.md) keeps these limits separate
from whole-solver soundness and complete enumeration.
