# Observe a completed model without replacing its identity

[Observations](../Zetesis/Observations.lean) starts after a caller supplies an
interpretation and completes a finite query. It does not decide whether that
interpretation is an answer set. The central distinctions are binding coverage,
model-relative truth, complete value identity, and the original model retained
beside its display.

## Read the enabled-value law

`Emits rows enabled values value` says that one supplied row is enabled and
produces the complete logical value. `completed_terms_exact` requires two separate
correspondences: `rows` contains exactly the intended bindings, and each valid
row's value list contains exactly its intended values. Soundness follows the
same row from the supplied lists to its intended meaning. Completeness follows
an intended enabled value back to that row and list. A value from one alternative
cannot be paired with the truth of another alternative.

The theorem describes the distinct term channel by membership. Repeated
derivations do not add new members. `same_rows_preserve_terms` permits reordering
or repeating complete rows; it does not permit replacing a tuple key with its
first component. `equal_measures_do_not_identify_tuple_keys` gives two complete
keys with the same measure and shows why deduplicating measures changes the key
set. Numeric reduction, logical extrema ordering and checked arithmetic remain
separate implementation correspondences.

## Preserve the quantifier and scope boundaries

An ordinary positive atom supplies structural captures from an actual atom of
the given model. A structural pool offers alternative patterns. Only names
captured in every branch can be exported to another body element without an
independent binder. Arithmetic consumes established values; matching does not
invert an arithmetic expression. Directed scalar and aggregate equalities can
supply later values, while aggregate element inputs must already be established.

A conditional quantifies universally over its completed local condition rows.
Within one row, source alternatives are existential; anonymous matches form
another existential projection inside each signed alternative. Default negation
applies after that projection. The reusable laws are
[ProjectedConditionals.original_semantics](../Zetesis/ProjectedConditionals.lean)
and its complete-projection and negative-projection laws. Empty condition rows,
empty source alternatives and an empty anonymous witness projection have distinct
meanings. An unfinished scan supplies none of the required completeness premises.

Local names do not become outer bindings. Fresh data-slot extension is described
by [FiniteValues.extension_preserves](../Zetesis/FiniteValues.lean); the concrete
scope planner must establish which slots are fresh and which outer inputs are
actually read. A successful expression supplies a value, while
`FiniteValues.failed_step` propagates failure without a shortened successful row.
The observation laws do not reinterpret a resource or arithmetic error as an
empty successful display.

## Keep full answers beside their displays

`decorate` attaches a completed display to each original model in a list.
`original_family_projection` recovers exactly the original list by removing only
the added display. This preserves order and multiplicity, not merely membership.
`different_models_remain_different_records` requires no injective display policy:
distinct original models remain distinct even if their displays agree.

`decorated_world_view` composes this projection with an existing
`WorldViews.Represents` premise. That premise supplies answer-set membership and
complete original-family coverage. Output supplies neither. The concrete
`display_deduplication_can_shrink_a_family` example demonstrates why deduplicating
after erasing hidden identity is a different operation. An atom selected for
ordinary output and an equal separately shown term also occupy distinct output
channels; this law does not justify merging them.

## Locate the executable correspondence

The Rust [observation compiler](../../crates/zetesis-themelios/src/observation/compile.rs)
retains source locations and builds queries separately from formula grounding.
Its pattern modules select finite structural alternatives before compiling their
captures. The [evaluator](../../crates/zetesis-themelios/src/observation/evaluate.rs)
reads only the supplied full model. Positive cardinality keys retain the complete
matched atom; negative keys are constructed from established source values and
tested against that same model. Complete tuple keys are deduplicated only after
all local conditions pass.

Borrowed model values have no local owned-payload charge. Generated alternatives,
owned binding values and retained aggregate keys share `Limits::max_local_bytes`.
The metric is 16 bytes per semantic node plus UTF-8 text; it excludes container
capacity and allocator overhead and is not RSS. A local scope releases its own
bindings and alternatives on exhaustion, early success or failure; enclosing
aggregate keys remain live until their own scope ends. The independently bounded
output set retains its own payload. These are Rust contracts supported by
[focused scope tests](../../crates/zetesis-themelios/tests/observation_scopes.rs),
not a theorem about allocation or cancellation.

The [complete-family matrix](../../crates/zetesis-themelios/tests/observation_families.rs)
checks original theory identity, display multiplicities and exact work ceilings
across hidden model families. The
[expression and pattern tests](../../crates/zetesis-themelios/tests/observation_expressions.rs)
include external comparisons for structural pools and explicit valid-source
refusals. Those refusals keep generative equality chains, constructor equality inversion,
inverse arithmetic binding, and anonymous negative cardinality keys visibly open. Native anonymous projection treats both strong signs uniformly; the
[separate diagnostic fixtures](../../crates/zetesis-themelios/tests/fixtures/observation-strong-anonymous.jsonl)
record that clingo 5.8.2 refuses anonymous strongly signed negative projections.
That extension is not a parity result or a diagnosis of clingo's implementation.
Passing examples and these abstract laws do not establish unrestricted language parity or complete
Rust refinement.
