# Refining formula evaluation

The optional [evaluator package](https://github.com/GregoryGelfond/zetesis/tree/main/refinement/evaluation)
connects the actual generated evaluator to per-node Ferraris reduct satisfaction
under fixed observation tokens. The original evaluation computes the frozen mask;
its correctness is derived from that call.

## The two-pass argument

Let `M` be the original interpretation, `J` any tested interpretation, and `Fᵢ`
the formula denoted by node `i`. The central theorem establishes:

```text
successful generated evaluate call at M
    → originalValues
successful generated evaluate call at J masked by originalValues
    → reductValues

reductValues[i] = true  iff  J satisfies the reduct of Fᵢ frozen at M
```

Both calls clear their old output and address the same stored formula table.
The theorem assumes successful call results, enough packed words for each
interpretation and child indices that refer to earlier nodes. It derives the
original mask's truth and length. It does not require `J` to be a subset of `M`;
that restriction belongs to later minimality search. This is per-node
satisfaction, not a proof of the Rust root scan or answer-set membership.

The [formal statement](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/evaluation/FixedReduct.lean)
composes these checked boundaries:

| Boundary | Argument |
| --- | --- |
| Setup | The generated entry operations produce a zero cursor and empty output |
| Packed membership | The extracted atom query returns the stored bit |
| One node | Actual reads and Boolean branches append exactly the masked truth |
| Work and stopping | Each continuation charges one unit; a stop preserves the prior prefix |
| Loop | Actual returned-control threading gives a terminating, trace-correct outcome under fixed tokens |
| Reduct | Two successful generated calls compose with the existing general Ferraris theorem |

An exhausted iterator completes without polling or charging a node. With a node
present, the iterator fetches it before polling; cancellation, deadline and work
refusal precede its evaluation and append. A typed stop leaves a strictly shorter
correct prefix. `FixedLoop` proves that the generated function terminates under
its input invariants and fixed-token model; termination may be such a refusal,
not a successful evaluation.

## Reusing the ASP library

The package imports the existing `ReductEvaluation` sources and their dependency
closure directly from the general library. Those eleven modules compile unchanged
under both the library's Lean 4.33.1 and the extraction package's Lean 4.31.0.
The bridge is checked in the latter toolchain with its own audit; it does not mix
object files from different versions or duplicate the semantic definitions.

A structural conversion maps extracted node constructors and machine indices to
the library's formula DAG. Fold correspondence then connects completed generated
calls to the existing reduct theorem. The result is a checked connection to the
same mathematical theory used elsewhere in the library.

## Remaining implementation boundary

`FixedLoop` follows the generated loop's control threading: repeated token reads
return the same value. The broader authored `Trace` relation permits separately
supplied controls between body calls. Fixed-loop calls form a trace, but the
[checked cancellation example](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/evaluation/FixedLoopExample.lean)
shows that a refreshed trace can stop after one node while the fixed-clear loop
completes two. The pinned extraction effect cannot express changing atomic read
results, so correspondence with concurrent Rust histories remains open.

Both passes share a stored table and numeric atom vocabulary. The `FrozenReduct`
struct and wrappers, Rust root scan, owner checks and subset search remain
unproved. Allocation, reference counting, timers, concurrent memory, machine code
and GPU execution also remain outside these library models.

The [package guide](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/evaluation/README.md)
records the exact scope, source hashes and reproduction commands. Generated code
comes from production Rust. A recorded local-name adjustment avoids a generator
namespace collision; exported destination metadata uses a portable path. Neither
changes executable operations. Translation tools and model correspondence remain
trusted. The [correctness plan](correctness.md) keeps these limits separate from
whole-solver soundness and complete enumeration.
