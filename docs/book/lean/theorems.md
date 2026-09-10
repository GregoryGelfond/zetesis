# A map of the central theorems

Follow the logical question to its definitions and hypotheses. The table is a
reading path, not a theorem-count claim or a substitute for the declarations.
The maintained full index is
[`proofs/theorems.json`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/theorems.json).

| Question | Module and central law | Required boundary |
| --- | --- | --- |
| What makes an interpretation an answer set? | `Ferraris.stable_iff_minimal_reduct` | Fixed original theory and candidate |
| When does least closure suffice? | `Semantics.stable_iff_gamma` | Normalized single-head rules and constraints |
| Why do normal and formula checking agree on this fragment? | `NormalFerraris.answer_set_iff`, `ferraris_answer_set_iff_closure` | The specified filter-aware translation and one shared atom universe |
| Why may only gate atoms be guessed? | `Semantics.accept_sound`, `stable_complete`, `stable_iff_exists_seed` | The supplied carrier covers every frozen gate |
| Can a stored mask evaluate a reduct? | `Ferraris.masked_eval_iff_reduct` in `FerrarisMask` | Mask agrees with original truth at every formula; arbitrary tested interpretation |
| Can joins be decomposed? | `Lifted.composition_exact` | Binding, filtering, gates and projection denote the supplied template |
| When may equality columns prefilter full matches? | `ColumnRelations.full_matches_preserved` | Exact dictionary/columns, supplied rows and a total matcher entailing the equalities |
| When may candidate domains be narrowed? | `DomainContraction.compatible_contraction` | Unchanged theory, fixed assignment interpretation and activation, recognized constraints and a sound filter |
| When is a lazy closure complete? | `LiftedBridge.completed_lazy_stage_exact` | Sound stages, conceptual grounding, final enabled-instance coverage and closure |
| Can a union scan serve independent worlds? | `LazyRounds.world_consequences_exact`, `world_constraints_exact` | Individual-world gates and positive truth are still checked |
| Can empty world masks prune joins? | `WorldMasks.masked_scan_covers_world` | Membership belongs to the current immutable snapshots; no future-carrier conclusion |
| Is reused join storage semantically harmless? | `JoinFrames.retained_frames_irrelevant` | Root reset and child overwrites follow the defined schedule |
| Can a class certificate replace a subset query? | `TightPlans.ranked_support_stable` | Complete original producers, rank, original-model truth and support |
| Can certificates and residual checks compose? | `CertifiedExecution.completed_membership_exact` | Sound verdict, exact residual answer and completed result |
| Does batched enumeration cover the requested family? | `BatchAccounting.completed_results_exact` | Candidate coverage, sound committed classification and exhaustion |
| Does no output mean inconsistency? | `Outcomes.empty_delivery_can_hide_a_valid_model` | Counterexample: absence of delivery is insufficient |
| When does a collection represent a world view? | `WorldViews.completed_capture_represents_world_view` | Original answer coverage, exact completed classification and complete capture |
| Does a complete optimal family suffice? | `WorldViews.optimal_family_omits_worse_answer` | A strictly worse original answer witnesses the difference |

Each module is available under
[`proofs/Zetesis`](https://github.com/GregoryGelfond/zetesis/tree/main/proofs/Zetesis).
Read `Core` before `Transformers`; read `Ferraris` before `FerrarisMask` and
`TightPlans`; read `Lifted` before `LiftedBridge`, `LazyRounds` and `WorldMasks`.
The [worked proof](reading.md) follows the certificate/completion path.
The [normal-rule bridge](normal-rules.md) follows model preservation into
minimality and least closure.

## Source and aggregate laws

The source library is organized around preservation obligations rather than
surface syntax alone. `StructuralBindings`, `FiniteValues`, `ConstructorPatterns`
and `EvaluatedWitnesses` address finite witnesses and checked construction.
`UniversalConditionals`, `ConsequentAlternatives` and `BooleanHeads` preserve
conditional and signed-head meaning. `CountHeadActivity` distinguishes
head permission coalesced by atom from aggregate activity coalesced by complete
tuple. Those equivalence relations cannot be interchanged.

`WeightedHeadActivity.formula_original` extends that independent tuple activity
to signed sums over a complete, duplicate-free key carrier.
`WeightedHeadActivity.bound_frozen` shows why the activated bound constrains
the candidate without supplying reduct support. The canonical finite mask
formula is the mathematical reference; the optimized Rust lowering remains a
separate correspondence.

`ExtremumHeadActivity.formula_original` and `formula_frozen` connect the same
complete-key activities to numeric minimum and maximum. Empty selections have
explicit infinite extrema; `finite_measure_has_witness` requires a selected,
eligible tuple for a finite result. `bound_frozen` retains candidate-only bound
checking, and `stable_in_context` preserves the complete head group when rows
are duplicated or reordered without changing their keys. These laws assume
finite complete tables and mathematical integers; they do not verify source
enumeration, optimized comparison construction or machine endpoints.

`BooleanHeadElements` represents an atom or a truth constant explicitly.
`activity_original` and `activity_frozen` retain the operand and eligibility
semantics; `remove_boolean_permissions` removes constants from atom permissions.
`boolean_group_in_context` proves that a Boolean-only head group can filter
the context's answer sets by its bound without creating atom support. Ordinary
choice keys distinguish Boolean element occurrences from atom identities;
explicit aggregate keys remain complete tuples. These are distinct source
contracts, not requirements to reproduce another solver's internal lowering.
Assigning those keys from the concrete source remains a refinement obligation.
Boolean choice elements follow an explicitly adopted compatibility extension;
the [ASP-Core-2 choice grammar (§4)](https://arxiv.org/pdf/1911.04326) covers
classical atom heads. Observable clingo comparisons corroborate the extension
contract; they do not make its internal data structures authoritative.

[`SignedHeadElements`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/SignedHeadElements.lean)
extends this distinction to `not` and `not not`. `operand_frozen` fixes either
default-negated operand's truth in the candidate; `activity_frozen` retains the
eligibility formula's own reduct. `signed_permission_frozen` explains why these
operands supply no positive atom permission. Ordinary atomic keys include their
sign, while Boolean keys retain their source occurrence. `coalesced_group_in_context`
preserves complete answer sets under its finite family and coalescing hypotheses;
`nonproducing_group_in_context` shows that a group without positive atomic operands
can filter the context's answers without creating new support. The embedding into
the earlier head-element laws is mathematical and does not prescribe a compiler
rewrite or verify its source catalog.

`AggregateReduct.direct_reduct` relates the failing-subset formula to original
and frozen eligibility under complete masks. `AggregateRanges` establishes bounds
on mathematical signed partial sums. `AggregateDependencies` composes complete
predecessor-indexed value families. `ObjectiveTransport` and `ExtremumPresence`
retain the distinction between possible carriers, realized values and objective
presence.

[`OrderedBounds`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/OrderedBounds.lean)
connects numeric measures to logical bounds. `separated_comparison` requires the
bound to have the same ordering against every integer. `constant_aggregate`
then proves original and frozen equivalence for the complete canonical aggregate;
`replacement_in_context` permits that replacement under arbitrary connectives.
`measured_head_in_context` retains the head's separate positive permissions.
These are all-mask laws, not simplification by a single candidate's truth.

[`ObjectivePriorities`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/ObjectivePriorities.lean)
retains the correlation between fields of an eligible objective row.
`completed_presence` requires numeric weight and priority at the same completed
binding; `independent_fields_invent_priority` gives a counterexample to separate
field projection. `partition_vector` and `partition_optima` preserve costs and
optimal ties when complete rows are partitioned by their fixed priority. The
priority layout remains shared across candidates, including zero-valued slots.

[`AggregateInvariants`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/AggregateInvariants.lean)
provides `sum_invariant` for optional zero contributions and `extremum_invariant`
for optional values absorbed by the required extremum. Required keys must survive
and actual keys must lie within the possible carrier; the extremum law states
its order and empty-endpoint assumptions explicitly. Optional keys may be
correlated. `fixed_priority_presence` connects a fixed resolved value with
complete eligible rows and a numeric weight from the same binding.

[`SourceMeasures`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/SourceMeasures.lean)
defines the finite measures of key selections between required and possible
keys. `carrier_membership` characterizes that enumeration;
`actual_value_covered` includes every actual active selection.
`invariant_carrier` gives the singleton case. `source_priority_presence`
retains complete binding and numeric-field requirements.

[`IntegerEnvelopes`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/IntegerEnvelopes.lean)
proves bounds along directed comparisons and through affine expressions.
Endpoint rounding is mathematical integer division. `filtered_bindings_exact`
states that complete envelope enumeration followed by the original guard gives
exact bindings. Source recognition and machine arithmetic remain separate.

These are useful mathematical components. Their coverage or arithmetic premises
do not prove that a particular Rust source cursor enumerates the required
bindings, that whole tuples are coalesced correctly, or that the machine obeys
the stated integer bounds. See [implementation correspondences](correspondence.md)
before interpreting a semantic law as an executable guarantee.
