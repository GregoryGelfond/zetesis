# Evaluating an admitted formula DAG

[IndexedEvaluation](../Zetesis/IndexedEvaluation.lean) connects checked indexed
reads to the original and frozen-reduct computations. It belongs to the
representation layer. The independent Ferraris definition remains unchanged.

The earlier mathematical evaluator assigns falsum to an unavailable reference.
That makes its semantics total, but does not justify indexing a Rust vector.
This module makes the missing step explicit: admitted child indices refer to
initialized values, the frozen mask covers the table, and asserted roots exist.

## Operations and invariant

`node` reads children through optional lookup. `applyMask` similarly reads the
current frozen bit. `step` appends a value only after these reads succeed.
`evaluate` folds that step over the complete table. A missing entry returns
`none`, which is distinct from a completed false value.

`evaluate_exact` proves the fold against its total reference using the existing
`DagSharing.WellFormed` admission predicate. After each prefix:

1. The output has one value per processed node.
2. Every child of the next admitted node lies in that output.
3. A covering frozen mask contains the next node's bit.

The checked reads therefore succeed. Both computations append the same value,
preserving the prefix invariant. Neither output correctness nor index safety
is assumed. The structural input conditions are DAG admission and mask length.

`original_exact` specializes the result to the ordinary evaluator.
`reduct_exact` uses a computed original mask. The original evaluator's length
theorem supplies mask coverage, so no separate mask-correctness assumption is
introduced.

## Satisfaction

`roots` checks asserted positions in order. It stops at the first false root;
an absent root that is actually read returns `none`. `roots_exact` proves that
an admitted root list yields the same conjunction as the mathematical scan.

`satisfiesReduct` composes two checked passes and that root scan. The first pass
computes the outer interpretation's mask; the second evaluates the tested
interpretation against that immutable mask. `satisfies_reduct_exact` establishes
completion and the Boolean result. `satisfies_reduct_iff` identifies a true
result with satisfaction of the Ferraris reduct. No subset relation between
the interpretations is required; minimality is a separate membership condition.

## Implementation boundary

The corresponding loop is `evaluate` in
`crates/zetesis-ferraris/src/oracle.rs`. This proof uses authored Lean operations,
not a new extraction of that Rust loop. It removes the missing-index default
from successful admitted evaluation; it does not prove source admission,
machine-index conversion, atom membership, allocation, cancellation, work
limits or theory-owner identity.

The model reads both operands of a binary node before combining their values.
Its theorem concerns admitted tables, where both operands exist. It does not
claim Rust's precise short-circuit read trace, work count or malformed-input
error precedence. The root scan does model early false termination explicitly.
