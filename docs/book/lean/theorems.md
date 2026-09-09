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
| When is a lazy closure complete? | `LiftedBridge.completed_lazy_stage_exact` | Sound stages, conceptual grounding, final enabled-instance coverage and closure |
| Can a union scan serve independent worlds? | `LazyRounds.world_consequences_exact`, `world_constraints_exact` | Individual-world gates and positive truth are still checked |
| Can empty world masks prune joins? | `WorldMasks.masked_scan_covers_world` | Membership belongs to the current immutable snapshots; no future-carrier conclusion |
| Is reused join storage semantically harmless? | `JoinFrames.retained_frames_irrelevant` | Root reset and child overwrites follow the defined schedule |
| Can a class certificate replace a subset query? | `TightPlans.ranked_support_stable` | Complete original producers, rank, original-model truth and support |
| Can certificates and residual checks compose? | `CertifiedExecution.completed_membership_exact` | Sound verdict, exact residual answer and completed result |
| Does batched enumeration cover the requested family? | `BatchAccounting.completed_results_exact` | Candidate coverage, sound committed classification and exhaustion |
| Does no output mean inconsistency? | `Outcomes.empty_delivery_can_hide_a_valid_model` | Counterexample: absence of delivery is insufficient |

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

`AggregateReduct.direct_reduct` relates the failing-subset formula to original
and frozen eligibility under complete masks. `AggregateRanges` establishes bounds
on mathematical signed partial sums. `AggregateDependencies` composes complete
predecessor-indexed value families. `ObjectiveTransport` and `ExtremumPresence`
retain the distinction between possible carriers, realized values and objective
presence.

These are useful mathematical components. Their coverage or arithmetic premises
do not prove that a particular Rust source cursor enumerates the required
bindings, that whole tuples are coalesced correctly, or that the machine obeys
the stated integer bounds. See [implementation correspondences](correspondence.md)
before interpreting a semantic law as an executable guarantee.
