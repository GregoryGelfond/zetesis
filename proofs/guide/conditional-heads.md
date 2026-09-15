# Conditional disjuncts and scoped aggregate values

A finite conditional head `H:C` contributes

\[
  (C \to H) \land \neg\neg C.
\]

All its completed local instances belong to one disjunction. The enclosing
rule remains `B → head`. An empty local family contributes false; a universal
body conditional instead has a vacuously true empty conjunction. This is the
conditional-disjunction translation in Hansen and Lierler,
[Semantics for Conditional Literals via the SM Operator](https://par.nsf.gov/servlets/purl/10379357),
§§3–4. A finite source profile supplies the ground instances; these laws begin
with their completed list.

## Original truth and the frozen reduct

[ConditionalHeads.lean](../Zetesis/ConditionalHeads.lean) reuses the condition and
consequent pair from `UniversalConditionals`, with an existential connective.
`instance_original` says that an instance holds precisely when its condition and
head both hold. `head_original` selects one such complete pair from the finite
list. Selecting a condition from one binding and a head from another is invalid.

For a candidate `M` and tested interpretation `J`, `instance_frozen` retains:

1. `C` and `H` hold in the original candidate `M`;
2. the reduct of `C` implies the reduct of `H` in `J`.

The double negation freezes eligibility at `M`, while the implication still
tests its original subformulas under the reduct. Even when `M` satisfies `C`,
replacing `C → H` by `H` is not justified: `J` may falsify `C`. No subset premise
is needed for these equivalences. Stable-model checking subsequently quantifies
over the proper subsets of the same candidate and the same original theory.

`true_condition_equivalent` erases a condition only with original truth in every
interpretation and reduct truth for every frozen pair. `empty_is_false` states
the completed empty-head boundary. `complete_family_equivalent` permits repeated
or reordered instances with exact membership coverage of whole pairs. It does
not justify omitting an unexplored binding after a resource refusal.

## Positive producer permissions

[ConditionalHeadSupport.lean](../Zetesis/ConditionalHeadSupport.lean) allows signed
atom heads and normalized Boolean heads. A positive atom's producer permission
is the conjunction of the outer body and that occurrence's local condition.
Neither the condition's atoms nor a default-negated head acquire producer status.

`stable_has_positive_producer` removes an allegedly unsupported atom. Every
originally active rule has a true eligible head occurrence. A positive head in
that occurrence must survive the removal, while negative and Boolean heads keep
their frozen truth. The retained implication therefore still holds. The smaller
interpretation would model the whole reduct, contradicting stability. This is a
necessary-support result, not a disjunctive shift or an answer-set certificate.

## Aggregate proposals enter existing local scopes

A completed aggregate-assignment carrier proposes outer values. Local head,
choice, aggregate-element, and universal-conditional consumers use the selected
value in their existing binding frames. They retain the original aggregate
equality in the enclosing rule; a successful arithmetic or range operation does
not establish that equality.

`covered_assignment` is the direct application of `AggregateConsumers`' covering
carrier law: the actual accepted value remains paired with its complete local
head family. `unrealized_assignment` states that a false original equality makes
the whole rule vacuous in the original and every frozen interpretation. Existing
`ConditionalConsumers` laws give the corresponding universal-body contract, and
`BindingScopes.readAll_restrict` explains why narrowing a frame to its declared
outer prefix preserves exactly the values available to that local consumer.

## Implementation correspondence and remaining premises

The source compiler clones the same outer variable frame independently for each
condition alternative. A local condition cannot bind the enclosing rule. The
conditional head stores the outer prefix separately from local condition and
generated-head slots. It shares the local binding scheduler used by choices;
completed local heads join the same disjunction. Ordinary unconditional head
intervals retain their Cartesian family of whole rules. Pool occurrence
expansion is shared, while each consumer selects its required connective.

The Rust controls compare complete stable families with explicit substitutions,
test original and frozen truth over small complete interpretation pairs, and
exercise empty carriers, signed and Boolean heads, separate local slots, source
origins, and inclusive limits. The Lean laws do not prove parser scope analysis,
possible-support completeness, source-to-slot identity, the join enumerator,
machine arithmetic, allocation/work accounting, or failure atomicity. Those are
separate implementation obligations; a stopped prefix is never a completed
instance family.
