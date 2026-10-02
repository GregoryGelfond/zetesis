# Refining formula evaluation

The optional [evaluator package](https://github.com/GregoryGelfond/zetesis/tree/main/refinement/evaluation)
connects traces of actual generated evaluator steps to the definition of Ferraris
reduct satisfaction. The original trace computes the frozen mask; its correctness
is proved, rather than assumed of a supplied table.

## The two-pass argument

Let `M` be the original interpretation, `J` any tested interpretation, and `Fᵢ`
the formula denoted by node `i`. The central theorem establishes:

```text
completed original trace at M
    → originalValues
completed trace at J masked by originalValues
    → reductValues

reductValues[i] = true  iff  J satisfies the reduct of Fᵢ frozen at M
```

Both traces start with empty output and address the same stored formula table.
The theorem assumes enough packed words for each interpretation and child indices
that refer to earlier nodes. It derives the original mask's truth and length.
It does not require `J` to be a subset of `M`; that restriction belongs to the
later minimality search. This result proves satisfaction, not answer-set
membership by itself.

The [formal statement](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/evaluation/ReductTrace.lean)
composes these checked boundaries:

| Boundary | Argument |
| --- | --- |
| Setup | The generated entry operations produce a zero cursor and empty output |
| Packed membership | The extracted atom query returns the stored bit |
| One node | Actual reads and Boolean branches append exactly the masked truth |
| Work and stopping | Each continuation charges one unit; a stop preserves the prior prefix |
| Trace | Induction preserves the complete truth prefix, source table and work counts |
| Reduct | Existing general ASP theorems identify the second pass with explicit Ferraris reduct truth |

An exhausted iterator completes without polling or charging a node. With a node
present, the iterator fetches it before polling; cancellation, deadline and work
refusal precede its evaluation and append. A typed stop leaves a strictly shorter
correct prefix. It does not supply a completed evaluation. The package also
constructs a finite trace from admitted state, which may end with such a stop.

## Reusing the ASP library

The package imports the existing `ReductEvaluation` sources and their dependency
closure directly from the general library. Those eleven modules compile unchanged
under both the library's Lean 4.33.1 and the extraction package's Lean 4.31.0.
The bridge is checked in the latter toolchain with its own audit; it does not mix
object files from different versions or duplicate the semantic definitions.

A structural conversion maps extracted node constructors and machine indices to
the library's formula DAG. Fold correspondence then connects generated-step
traces to the existing reduct theorem. The result is a checked connection to the
same mathematical theory used elsewhere in the library.

## Remaining implementation boundary

A trace records calls to the unchanged generated loop body, each with a separately
supplied control value. It is an authored execution relation. It has **not yet
been proved equivalent to the generated whole loop or a Rust execution history**.
The current extraction effect cannot express changing atomic read results.
Reusing a fixed token would silently freeze those observations.

The theorem shares a stored table and numeric atom vocabulary between its passes.
Rust pointer-owner checks remain unproved. Allocation, reference counting,
timers, concurrent memory, machine code and GPU execution also remain outside
the library models. Constructor admission and subset-search refinement are further
steps toward full membership verification.

The [package guide](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/evaluation/README.md)
records the exact scope, source hashes and reproduction commands. Generated code
comes from production Rust. A recorded local-name adjustment avoids a generator
namespace collision; exported destination metadata uses a portable path. Neither
changes executable operations. Translation tools and model correspondence remain
trusted. The [correctness plan](correctness.md) keeps these limits separate from
whole-solver soundness and complete enumeration.
