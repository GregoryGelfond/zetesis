# Selected comparisons and retained producer rows

These are conditional mathematical laws for source-activity preparation. They
preserve the original occurrence sequence and consumer; they do not replace
reduct membership, prove complete grounding, or identify possible activity with
truth in an answer set.

## A comparison already selected this binding

[`SelectedProducerComparisons.selected_fold_exact`](../Zetesis/SelectedProducerComparisons.lean)
uses the three-valued `ObjectiveEligibility.Activity` algebra. Its hypothesis says
that every marked ordinary comparison in this body already reads `required` at
this exact binding. Replacing only those reads by `required` preserves the full
ordered conjunction. The induction keeps the head activity equal and applies the
same premise to the remaining occurrences; it does not short-circuit other
literals. This does not authorize reuse for unselected bindings or imply equal
canonical-value allocation histories.

## Exact rows, fresh current-round activity

[`CyclicRows`](../Zetesis/CyclicRows.lean) retains occurrences, including duplicates:

1. `selected_occurrences_exact` derives list equality from equal source lists and
   pointwise-equal scalar selection. Immutability and scalar agreement are premises.
2. `replay_current_consumer_exact` substitutes that exact list into the same
   ordered fold, starting with the same current state. It does not reuse any old
   evaluated activity.
3. `whole_pass_exact` lifts exact rows for each producer to a complete pass in the
   same producer order.
4. `refinement_trace_exact` lifts pointwise-equal passes to every fixed finite
   sequence of states. It is not a termination or completed-fixed-point theorem.
5. `rows_do_not_determine_activity` supplies the counterexample: even the same
   one-element list can produce required or optional activity under different
   current-state consumers.

`CyclicRows` is the module name; its declarations retain the namespace
`Zetesis.SourceActivity.CyclicRows`. These equations do not prove that a concrete
cache obtains exact rows, seals only after successful exhaustion, or carries the
right owner. Rust must also preserve consumer error order and authenticate each
retained source/rule/relation and canonical value identity. Resource charges,
extra retained bytes, refusal and cancellation are separate contracts.

## Uniformly absent nested arithmetic scopes

[`ArithmeticFamilies.absent_nested_outcomes`](../Zetesis/ArithmeticFamilies.lean)
requires a sound classifier for every construct capable of producing a nested
scope. If no occurrence can produce nested outcomes for any binding, their
ordered concatenation is empty. `absent_nested_preserves_family` then preserves
family admission and zero-divisor warnings when that empty family is omitted.
One observed empty binding alone is insufficient. Enclosing-row validation and
independent scalar fields remain mandatory; no work or polling equivalence is
claimed.

The [manual correspondence section](../../docs/book/lean/correspondence.md#selected-comparisons-and-retained-producer-rows)
identifies the executable obligations. The umbrella and strict axiom audit cover
these declarations only after the maintained proof capture is regenerated for
the integrated source; an older capture does not qualify a later edit.

[Ordered parallel grounding](parallel-grounding.md) addresses row partition and
witness transport separately. These mechanisms do not make selected comparison
reuse, retained rows or any worker count a default; each experimental composition
requires its own source-bound Rust and qualification evidence.
