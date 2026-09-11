# zetesis proof library

This Lean library formalizes answer-set semantics and the mathematical contracts
of zetesis's grounding, reduct checking and complete search. It is independently
usable without the Rust solver. The package uses **Lean 4.33.1** and its standard
library, with no external package dependencies.

Start with [Part III of the manual](../docs/book/lean/foundations.md) for the
semantic foundations and their relationship to the solver. The [reading guide](guide/README.md)
organizes results by logic-programming questions. [STYLE.md](STYLE.md) describes
how declarations, named claims and nested justifications make the proofs readable.
The checked Lean source remains the mathematical deliverable.

## Find a definition or theorem

[theorems.json](theorems.json) is the authoritative inventory of qualified theorem
names and source locations. [Zetesis.lean](Zetesis.lean) imports the complete
library; consumers can instead import individual modules and their dependency
closures. The manual's [theorem map](../docs/book/lean/theorems.md) supplies a
conceptual overview.

| Question | Starting point |
|---|---|
| What is an answer set under the formula reduct? | [Ferraris](Zetesis/Ferraris.lean) defines truth, the frozen reduct and subset minimality. |
| Why does normalized least closure suffice? | [NormalFerraris](Zetesis/NormalFerraris.lean) proves equivalence with [Semantics](Zetesis/Semantics.lean); the [bridge guide](guide/normal-ferraris.md) explains the argument. |
| What does a completed membership result establish? | [CertifiedExecution](Zetesis/CertifiedExecution.lean) and its [worked proof](guide/certified-membership.md) separate a verified result from unfinished work. |
| When does enumeration establish the original world view? | [WorldViews](Zetesis/WorldViews.lean) composes exact membership, original candidate coverage and complete capture; optimal selection remains distinct. |
| How do shown terms preserve full answer identity? | [Observations](Zetesis/Observations.lean) separates complete enabled-value coverage from decorating original models. Its [reading guide](guide/observations.md) states the binding, scope and implementation premises. |
| How do weighted heads handle shared atoms and tuples? | [WeightedHeadActivity](Zetesis/WeightedHeadActivity.lean) connects distinct tuple sums to canonical guards and candidate-only bounds. |
| How do extrema heads handle those aliases? | [ExtremumHeadActivity](Zetesis/ExtremumHeadActivity.lean) connects selected complete tuples to numeric extrema, explicit empty results and independent atom permissions. |
| How do logical values extend extrema heads? | [OrderedHeadActivity](Zetesis/OrderedHeadActivity.lean) connects signed tuple activity to ordered selection over an arbitrary value carrier; complete-key coverage, comparison laws and logical empty values are explicit premises. |
| When is a logical bound constant for a numeric aggregate? | [OrderedBounds](Zetesis/OrderedBounds.lean) preserves original and frozen aggregate truth when the comparison agrees across every finite tuple mask; concrete term ordering and checked evaluation remain premises. |
| How do variable objective priorities preserve correlated values? | [ObjectivePriorities](Zetesis/ObjectivePriorities.lean) selects numeric weight/priority pairs from the same completed row and preserves costs and optimum ties under per-priority evaluation. Exact source eligibility is a separate obligation. |
| How do shared objective query nodes retain original truth? | [ObjectiveConditionTable](Zetesis/ObjectiveConditionTable.lean) proves backward-indexed table correspondence with unfolded conditions, including the empty-table result. The [reading guide](guide/objective-condition-table.md) explains the invariant and remaining implementation obligations. |
| When can an aggregate have one fixed value despite optional tuples? | [AggregateInvariants](Zetesis/AggregateInvariants.lean) proves that zero optional contributions preserve sums and dominated optional values preserve extrema, then composes a fixed priority with complete eligible objective rows. Source classification and checked arithmetic remain separate obligations. |
| How do changing aggregates supply source priorities? | [SourceMeasures](Zetesis/SourceMeasures.lean) defines the finite carrier between required and possible complete keys. The carrier may contain unrealized values; original conditions determine costs. Complete whole rows still determine numeric priority presence. |
| When does a completed producer carrier cover answer sets? | [SourceSupport](Zetesis/SourceSupport.lean) connects closure and reduct minimality under an explicit projection premise; its [reading guide](guide/source-support.md) distinguishes possible support from truth in an answer set. |
| How do comparison generators retain all valid bindings? | [IntegerEnvelopes](Zetesis/IntegerEnvelopes.lean) proves directed path bounds, affine endpoint rounding and finite envelope enumeration. The retained whole guard selects exact bindings; source safety and checked runtime inference remain separate. |
| Can a truth constant supply atom support? | [BooleanHeadElements](Zetesis/BooleanHeadElements.lean) separates Boolean activity from atom permission and preserves bounded head groups in context. |
| What changes when a choice operand has default negation? | [SignedHeadElements](Zetesis/SignedHeadElements.lean) keeps signed contribution identity, candidate-frozen operand truth and positive-only permissions separate; eligibility still has its own reduct. |
| Where does negation apply to an anonymous consequent? | [ProjectedConditionals](Zetesis/ProjectedConditionals.lean) separates complete anonymous witness projections, signed source alternatives and universal condition rows. |
| How does a consumer apply the normal/Ferraris bridge? | [The checked choices example](Zetesis/Examples/Choices.lean) proves that `{a}` is an answer set of the guided tour's two-rule program through its least reduct closure. |
| Which implementation correspondences remain open? | The manual's [proof boundary](../docs/book/lean/correspondence.md) separates mathematical laws from executable refinement. |

For example, `NormalFerraris.answer_set_iff` relates the two independently defined
predicates:

```lean
Semantics.Stable P M ↔ Ferraris.Stable M (NormalFerraris.translate P)
```

Its translation selects applicable ground filters. The direct ground-rule-map
corollary makes prior filter discharge explicit; frozen-subset correspondence
states `J ⊆ M`. No assumption that every atom occurs in a rule is needed because
both sides use the same atom universe.

## Build and audit

Install the toolchain pinned by [lean-toolchain](lean-toolchain), then run from
this directory:

```sh
lake build
lake env lean -DautoImplicit=false -DwarningAsError=true Audit.lean
```

`lake build` compiles the umbrella and all semantic modules.
[Audit.lean](Audit.lean) requests the transitive axiom dependencies of every
registered theorem. The admitted standard logical axioms are `propext`,
`Quot.sound` and `Classical.choice`; project axioms, proof holes and native
proof-evaluation shortcuts are not admitted.

From the repository root, check the retained record with:

```sh
cargo run --locked -p zetesis-maintenance -- proof-record --proofs-dir proofs
```

[verification.json](verification.json) identifies the recorded source hashes,
commands and logs. [axiom-audit.txt](axiom-audit.txt) contains the corresponding
complete audit output. The record checker validates inventories, source
locations, counts, hashes and allowed axioms. It does not rerun Lean or establish
that a command was executed; kernel checking and record consistency are separate
requirements. `scripts/check.sh proofs` runs both.

## Verification boundary

The laws prove their stated mathematical conclusions under explicit premises,
such as finite carrier coverage, complete proposals, preserved atom identity or
agreement of an abstract evaluator. A checked preservation law does not establish
that Rust source recognition or a GPU kernel satisfies those premises.

The library does **not** yet formally verify the complete solver. Remaining
obligations include source-to-ground correspondence, concrete atom and formula
representations, Rust and WGSL execution, and resource, cancellation and output
behavior. The normalized/Ferraris bridge proves semantic translation and
minimality equivalence; it does not certify DAG allocation or physical hardware.
Tests, device qualification and performance measurements provide distinct
implementation evidence.
