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
An equality edge inside a positive comparison chain can establish an unbound
variable once its other operand is available. The planner moves that operand
into the binding instruction and retains every original comparison, replacing
the moved operand with its new value slot. Strict comparisons and default-negated
chains remain tests. A middle interval or pool has one chosen value shared by
both adjacent comparisons. `shared_equality_choice_exact` proves the finite
two-edge law with an arbitrary surrounding guard; it does not prove the concrete
planner, arithmetic evaluation or resource behavior.

Generation precedes the complete query's condition checks. A reached arithmetic
failure or exceeded limit therefore fails the observation even if a later guard
would reject that row. Each assigned value remains owned by its binding depth
and charged until that depth is left; moving a source operand does not add a
logical atom or a grounding-domain value.

A positive structural equality can capture constructor or tuple components from
an already finite evaluated operand. The complete chosen value occupies one
fresh slot. Input captures borrow their original terms; generated captures retain
canonical IDs and their own logical local charge, without copying term payload.
Keeping that complete value avoids reconstructing an expression or making a new
pool choice when another chain edge reads it. Matching checks the constructor's
sign, name and ordered children, as specified abstractly by
`FiniteValues.constructor_identity` and `FiniteValues.tuple_identity`. Repeated
variables constrain equal components. Arithmetic positions consume established
or same-pattern structural captures; they do not invert arithmetic. A mismatch
releases partial captures before trying the next value. Error and visitor-stop
paths release the same ownership.

An equality guard on `#min` or `#max` can use that same structural capture.
The aggregate first computes its one actual measure from the supplied model;
each original guard then reads the retained measure slot. The planner leaves
the equality in place for structural matching and retains every other guard,
including guards that depend on the new captures. Aggregate element inputs
must be available independently of the result. Empty extrema still return
`#sup` and `#inf`, which do not match a constructor or tuple. Numeric aggregate
guards keep their widened comparison contract; this structural generation path
does not convert a `#count` or `#sum` measure into an ordinary scalar value.

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

Borrowed model values have no local payload charge. Generated alternatives,
generated binding values and retained aggregate keys share `Limits::max_local_bytes`.
The metric is 16 bytes per semantic node plus UTF-8 text; it excludes container
capacity and allocator overhead and is not RSS. Repeated references can carry
separate logical charges while sharing canonical payload. A local scope releases
its own charges on exhaustion, early success or failure; enclosing aggregate
keys keep their charges until their scope ends. The derived arena retains its
registered roots until evaluation ends, under the separate
`Limits::max_term_storage_bytes` allowance. It borrows model and compiled-metadata
prefixes rather than copying their payload. The output boundary constructs the
independently bounded public terms. These are Rust contracts supported by
[focused scope tests](../../crates/zetesis-themelios/tests/integration/observation_scopes.rs),
not a theorem about allocation or cancellation.

The [complete-family matrix](../../crates/zetesis-themelios/tests/integration/observation_families.rs)
checks original theory identity, display multiplicities and exact work ceilings
across hidden model families. The
[expression and pattern tests](../../crates/zetesis-themelios/tests/integration/observation_expressions.rs)
include external comparisons for structural pools and explicit valid-source
refusals. The [binding contracts](../../crates/zetesis-themelios/tests/integration/observation_bindings.rs)
cover finite equality chains, shared middle alternatives, original family identity,
resource boundaries and complete external displays. Structural pool alternatives
on a capturing equality operand, inverse arithmetic binding, and anonymous negative
cardinality keys remain visibly open. Repeated unary negation around an unbound
constructor pattern is not yet normalized for capture. Constructor patterns equated to numeric
aggregate measures are also still refused when the pattern has no other binder:
these valid comparisons have no matching rows, but their recognition is not part
of the constructor-valued extremum generation path. The binding contracts retain
exact sources and complete external references for those remaining refusals.
Native anonymous projection treats both strong signs uniformly; the
[separate diagnostic fixtures](../../crates/zetesis-themelios/tests/fixtures/observation-strong-anonymous.jsonl)
record that clingo 5.8.2 refuses anonymous strongly signed negative projections.
That extension is not a parity result or a diagnosis of clingo's implementation.
Passing examples and these abstract laws do not establish unrestricted language parity or complete
Rust refinement.
