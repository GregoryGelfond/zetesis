# Computing a frozen formula reduct

[ReductEvaluation](../Zetesis/ReductEvaluation.lean) supplies a finite executable
operation for testing a Ferraris formula reduct. It is a general mathematical
capability over any atom type, independent of grounding, a solver backend or a
machine-word representation. It connects an indexed computation to the existing
[formula semantics](../Zetesis/Ferraris.lean).

## Two interpretations, two passes

The **outer interpretation** determines which original subformulas survive the
reduct. The **tested interpretation** determines whether that frozen reduct is
satisfied. Each interpretation is supplied as a Boolean function on atoms. The
tested interpretation need not be a subset of the outer one; subset minimality
is a separate obligation when using satisfaction to decide answer-set membership.

The input is the existing `DagSharing.Node` table: atoms, falsum, conjunction,
disjunction and implication with indexed children. Its `meanings` fold unfolds
each node using the previously available formulas. Shared children can be read
more than once without duplicating their stored entry.

`ReductEvaluation.values outer tested table` performs two folds:

1. `TightEvaluation.values outer table` computes the original classical truth of
   every node. This completed table is the immutable mask for the second pass.
2. The second fold reads child values already computed in the tested
   interpretation. It combines them with the node's Boolean operation, then
   conjoins that result with the original truth at the current node's index.

A false mask bit makes the node false even if its children would make it true
in the tested interpretation. A true mask bit permits the ordinary operation on
the children's **already masked** values. In particular, implication uses the
reduct truths of its antecedent and consequent; it does not reevaluate the outer
interpretation or substitute ordinary truth in the tested interpretation.

## Why the folds compute the reduct

The main theorem, `values_correspond`, identifies the entire second-pass table
with the Boolean evaluation of the explicit Ferraris reduct of every unfolded
node. Its argument has three parts.

First, the existing original-evaluation theorem establishes each mask bit from
the first fold. Mask correctness is therefore a derived fact, not a premise of
the public result.

Second, the local layer argument combines exact child-reduct values with that
mask bit. If the original layer is false, the explicit reduct is falsum and the
masked result is false. If it is true, the reduct retains the connective over
its child reducts, exactly as the second fold computes.

Third, a prefix invariant carries these facts through the indexed fold. Appending
a node leaves every earlier unfolded formula unchanged. The old output therefore
remains exact, its length selects the current mask bit, and the layer argument
establishes the appended value. Reverse-list induction expresses this
construction-order argument; the evaluator itself does not reverse its input.

`values_length` establishes that the completed result has one entry per node.
`value_at` transports whole-table correspondence to one indexed read.
`roots_true_iff` then proves that checking all original root positions is exactly
satisfaction of `Ferraris.ReductTheory` for those roots. Repeated roots, root
order and unasserted shared entries do not change that conjunction.

## Scope of the result

The mathematical operations totalize an unavailable child or root as falsum,
matching the existing `DagSharing` decoder. Consequently these correspondence
theorems need no well-formedness premise. This total mathematical meaning does
not authorize a runtime array access: an implementation must establish its
admission, atom representation and index bounds separately.

The result proves frozen-reduct satisfaction for a completed finite computation.
It does not establish proper-subset enumeration, answer-set membership or a
complete family of answer sets by itself. Nor does it verify Rust extraction,
ownership, allocation, cancellation, work limits, shader synchronization or
readback. The list operations specify the algorithm's result, not its physical
array complexity. Those implementation correspondences belong in a distinct
refinement layer.
