# Checking constraints without retaining their whole ground theory

[`StreamedConstraints`](../Zetesis/StreamedConstraints.lean) connects a bounded
finite scan to the existing constraint-filtering law. The retained theory may
contain arbitrary formulas. The constraints being checked separately must be
original bodies implying falsum; they cannot supply support for an atom.

## What the scan knows

The mathematical source is a finite list of admitted ground occurrences. A
function `body` gives each occurrence's formula; `test` evaluates its original
truth in one fixed candidate. The list describes the complete family without
requiring the Rust implementation to allocate that whole list. Establishing
that the source cursor covers it is a separate obligation.

`scan` consumes bounded fuel. A true body returns its occurrence as a violation.
An observed empty suffix returns `complete`. Zero fuel returns `pending` with
the unread suffix, even if that suffix is empty. Consequently the caller cannot
mistake lack of remaining fuel for observed completion.

`scan_preserves` proves the accounting invariant by induction on fuel:

1. A violation names an occurrence in the supplied source whose test is true.
2. Completion establishes that every test in the source is false.
3. A pending suffix follows a checked prefix whose tests were all false.

Each false head extends the tail's certificate or checked prefix. A true head
already supplies the required witness. The bound `source.length < fuel`
excludes a pending result: each occurrence consumes one step and observing the
empty suffix needs one more. This is a mathematical inspection bound, not the
work charged for constructing or evaluating a Rust source binding.

`scan_complete_iff` and `scan_violation_iff` then give both directions of the
completed verdict. The example `pending_can_hide_violation` checks one false
body and leaves a true one unread. Useful partial work is not satisfaction.

## Partitions and stable models

A partition covers the original occurrence list by permutation of its flattened
parts. This retains all occurrences, including multiplicity. `completed_partition`
proves that every part completes exactly when the original family is clear.
`partition_invariance` permits different chunk sizes and orders with that same
coverage. It preserves the satisfaction verdict, not the first violating
occurrence or diagnostic order. Parallel acceptance still needs every part's
completion; no law turns an unfinished partition into a false body.

The semantic connection has a pointwise premise: for every original occurrence,
its Boolean test is true exactly when the candidate satisfies its body formula.
`clear_iff_models` derives satisfaction of all original constraints from those
individual evaluations. It does not assume the desired whole-family result.

Finally, `stable_iff_completed_partition` applies
[`ConstrainedPositive.stable_append_constraints`](../Zetesis/ConstrainedPositive.lean):
the candidate is stable for the retained theory plus these constraints exactly
when it is stable for the retained theory and every covered part completes.
The retained theory need not be positive, and constraint bodies need not be
monotone. A violating constraint rejects a candidate; it does not establish
that no other retained-theory candidate is an answer set.

## Concrete obligations

Source lowering must establish the finite family, its original occurrence and
outer-binding scopes, complete aggregate witnesses and correct formula bodies.
Arithmetic admission remains independent of candidate truth: completed-family
warnings and refusals cannot depend on chunk size or on which candidate happens
to be checked first. The [arithmetic-family laws](../Zetesis/ArithmeticFamilies.lean)
state that separate outcome contract.

Rust must also establish cursor coverage, candidate and prepared-owner identity,
correct original evaluation, cancellation handling and completion publication.
Replaying a family charges its work again; bounded retained storage does not
bound total replay work. Cache validity, machine arithmetic, allocation, CPU/GPU
scheduling and full candidate enumeration are not proved by this module. The
finite scanner is a formal algorithm over supplied occurrences, not a verified
implementation of the source grounder or the whole solver.
