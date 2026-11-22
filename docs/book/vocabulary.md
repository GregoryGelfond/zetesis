# Vocabulary

The first column gives the logical concept; the others locate its current
representations. A representation name does not confer a semantic property.

| Concept | Current Rust representation | Lean vocabulary |
| --- | --- | --- |
| Canonical logical program | `themelios_program::Program`, re-exported as `zetesis_themelios::logical::Program`; constructed directly or raised from source | Source-template and translation laws use explicit abstract premises |
| Original semantic input | `zetesis_solve::Original`, also re-exported by `zetesis_cli`; checked results retain it through `original` fields or accessors | An input-identity owner, not a new Lean primitive or a membership proof |
| Original admission evidence | `zetesis_themelios::ProgramOriginal`: whole-program, part or statement identity in typed admission failures | Source ownership and provenance remain concrete obligations |
| Original statement evidence | `zetesis_themelios::ProgramSite` carries an owner-local `StatementId` when available and an optional real `Location` | Identity and provenance do not by themselves establish a translation law |
| Finite propositional theory | `zetesis_ferraris::Theory` | `Ferraris.Theory` |
| Interpretation | `zetesis_core::Interpretation` / `Model`; theory-bound `zetesis_ferraris::Interpretation` | `Atoms α` |
| Checked answer set | Privately constructed `zetesis_solve::AnswerSet` (`SessionModel` compatibility alias), or a native `StableInterpretation` | `Semantics.Stable` / `Ferraris.Stable` |
| Ground-symbol exchange set | `zetesis_themelios::logical::AnswerSet`, a plain `BTreeSet<Symbol>` used by explicit symbol export | A set representation with no checked membership claim |
| Complete world view | `WorldView` retains the original input and all full answers after unrestricted exhaustion | Membership, original-family coverage and complete-capture laws |
| Value combination over accepted members | Sealed `zetesis_algebra::reduction::Reduction`; pure values are separate from witnesses, fallible effects and completion evidence | `Zetesis.AxisFolds.Reduction`, a commutative monoid under each instance's stated premises |
| Boolean values and products | `zetesis_algebra::reduction::{All, Any}` combine by conjunction/disjunction; `(L, R)` combines both coordinates and absorbs only when both absorb | `Zetesis.AxisFolds.all`, `any` and `pair`; value laws do not commute effects |
| Bounded nonnegative addition | `zetesis_algebra::reduction::CheckedSum`: `Some(u64)` is exact, `None` propagates overflow, and no value signals successful early absorption | `Zetesis.AxisFolds.BoundedSum.reduction` at `u64::MAX`; an exact answer count still requires complete distinct-member coverage |
| Minimum value and incumbent selection | `zetesis_algebra::reduction::Minimum<V>` uses `None` as identity; `step` retains the incumbent on equality and returns its incoming comparison | `Zetesis.AxisFolds.Minimum.reduction` and `step_value_map`, under a pure lawful order consistent with denoted value equality; selection alone proves neither optimality nor complete enumeration of optimal ties |
| Set values represented by finite occurrence streams | `zetesis_algebra::reduction::sets::{intersection, union, ordered_union}` select or merge occurrences; `Model` retains canonical storage and fallible materialization | `Zetesis.AxisFolds.SetStreams.selected_inter`, `append_union`, `ordered_union_value`; faithful finite reads and, for the ordered cursor, strictly ordered unique inputs remain concrete premises |
| A reduction's result when enumeration may stop | `zetesis_algebra::reduction::Verdict`: `Absorbed`, `Complete`, `Empty` or `Undecided`, with caller-owned value, witness and typed stop payloads | `Algebra.FoldVerdict`; membership, accepted absorption and completed coverage are separate premises |
| A world-view question folded over answer sets: decided at an absorbing value, complete after exhaustion, or inconsistent | `WorldViewDecision` and its `WorldViewVerdict`, from `SessionBuilder::{known, possible, cautious, brave, answer}` | `AxisFolds.Reduction`, `world_view_fold_decided`, `world_view_fold_complete` |
| Positive consequence closure | Checked positive-formula specialization | `Semantics.Gamma`, `Zetesis.Least` |
| Frozen reduct | `zetesis_ferraris::FrozenReduct` | `Ferraris.ReductTheory` / `Semantics.ReductModel` |
| Display | Source observations selecting shown atoms and terms | Distinct from full interpretation |
| Proved optimum | `SemanticOutcome::optimum_proved()` and retained objective evidence | Objective ordering and completed-search laws |
| The sides of a region: held, cut, open | `Region::is_held`, `is_cut`, `is_open`; the carrier's `held_gate_atoms` and `cut_gate_atoms`; the region's `must` is the held set and the complement of `may` the cut set | `Region`, `Bounds.undecided` for the open region |
| The two readings of a formula under a region: sure, never | `Knowledge`, known to hold or known never | `FormulaBounds.Sure`, `FormulaBounds.Never` |
| A region visited as a leaf | `RegionStatistics::leaves`, the carrier's `regions_leaves`, the proposers' `leaves` | `decided_leaf_models` |
| The carrier's narrowing by its two closures | `NarrowingState`, `narrowing_passes`, `narrowing_stop`, `root_refuted`; the record key `carrier_narrowing` | `Bounds.lean` |
| An argument's bound | `argument_bounds::infer`, `Bound` | None; the inference serves preparation |
| An integer comparison form: a comparison literal's operands compiled once, when the compiler constructs the literal, for deciding it with machine integers | `formula_ir::Comparison`, `formula_ir::integer` (`IntegerComparison`) and `formula_support::integer_comparisons`; the grounding counters `integer_comparisons` and `integer_comparison_declines` (rows decided and declined); `ComparisonForms` (the prepared program's literals with a form, `with_form`, and without one by reason) | None; the form's verdict is the ASP order restricted to numbers |
| A checked numeric index: one immutable relation snapshot indexed by a column-mapped expression's integer value; equality and range probes retain source row order | `formula_support::queries::expression_keys`; `numeric_index_builds` and `numeric_index_declines` count completed certificates and data declines, `numeric_index_extensions` counts certificates support completion kept and extended with a later round's rows; `expression_key_probes`/`range_probes` count queries and `expression_key_rows`/`range_probe_rows` count visited rows | The residual comparison and first-new partition remain required; index totality is checked at build time |
| A tuple, a block of tuples sharing a bound prefix, the pending marks of a layout | the dense relations' `Tuple`, `Block` and `block_steps`, `PendingMarks` | None |
| A fact block: one predicate's fact rows at one position of the prepared program, in a product or run layout | `formula_facts::FactBlock`, traversed through `Item::Facts`; its [layout rule](architecture/grounding.md#facts-as-fact-blocks) preserves the fact rules' order | None; each row is a body-free ground rule |
| A positive body atom named from a held matched row, with an unchanged-prefix node reused when current | `formula_support::witnesses::WitnessRow`, `formula_ground::matched_nodes::MatchedNodes`; `row_atoms` counts successful ordinary-emission/capture names and `row_atom_reuses` its reused subset, excluding the producer and element joins | `AtomCatalogs.identity_determines_position`, `DagSharing.intern_stable_under_extension`; matching, lifetime and cache currency remain runtime obligations |
| A complete answer count: the size of the selected full answer-set family, or its optimal ties, established only after exhaustion | `SemanticOutcome::complete_answer_count`, `SessionBuilder::counting`, `PublicationMode::Count` and `CountRenderer`; `--count` prints the count and ignores display/projection identity | Original answer-set membership and complete selected-family coverage are both required |
| A statistics record; an admission's usage; a receipt | `RegionCounts` and its kin; `ExpansionUsage`; a `receipt` in `zetesis-core` is a value proving an operation completed | None |
| This solver's run and the reference solver | `native` and `reference` in the series view; clingo is the tool filling the reference's role | None |
| A requested solver configuration | a validation `profile`, a request for a `SolveConfig` | None |
| An ongoing solve | `zetesis_solve::Session` owns search and membership execution | Membership and coverage laws describe its semantic obligations |
| Cooperative cancellation and a deadline | `zetesis_algebra::Cancellation` carries shared cancellation and an optional absolute deadline. The inlined `CancellationPoll::poll` reads flags only; the clock is read, and expiry latched, at coarse boundaries: owned `Cancellation` polls (account refills, the region walk), a local `Meter`'s `POLL_INTERVAL` boundaries and ceiling, settlement, and an explicit `CancellationPoll::poll_deadline` | Stopped coverage remains distinct from exhaustion |
| A workgroup size: the invocations cooperating on one device candidate, also every candidate-local loop's stride; and the sizing that selects one size per retained graph from its level widths, each level's node count | `zetesis_wgpu::GpuWorkgroupSize` and `GpuWorkgroupSizing` (`Fitted` or `Fixed`) in `GpuKernelOptions::workgroup_sizing`; the batch statistics' `workgroup_size`; the solver's `gpu_workgroup_size` and its JSON key | None; a physical schedule that preserves original truth |
| Device discovery: listing a requested API's adapters with their reported identities, creating no device; and a device source, where a GPU primitive obtains its device — its own adapter selection, the same selection over an adapter enumeration begun earlier, or an existing context. Adapter enumeration is distinct from answer-set enumeration | `zetesis_wgpu::discover_adapters`; `GpuDiscovery` (an adapter enumeration begun on a background thread); `GpuDeviceSource` (`Selected`, `Discovered`, `Context`) and the solve-path primitives' `from_source`; `ExecutionResources::discovering` | None; adapter selection is outside the semantic laws |
| A candidate's inner preparation: the units its query root cuts, and the parity rows and count bounds of its countermodel class, probed from the closed root before its walk | `zetesis_ferraris::FrozenPreparation` and `FrozenPreparationStatistics`; `ReductStatistics::inner_preparation` (`InnerPreparationStatistics`), the receipts' `preparation` object under `reduct_query_regions` | `Algebra.countermodelSubjects`, its rows entering as `RowOf (Countermodels T M)` proofs |
| A certain predicate: one every rule deriving which is definite over certain predicates, so that its supported atoms hold in every answer set | `formula_support::certain::Certainty`, installed in the support catalog; `Fixing::Certain` where a body reads one as true; the [rule](architecture/grounding.md#certain-predicates) | `CertainAtoms.Derived` and `Forces`; `derived_stable` |
| A decided negation: a default negation at the top level of a rule's body, over a flat atom of a certain predicate whose relation is complete whenever the rule is read, so that it is false for a supported atom and true otherwise | `Certainty::decided`; `formula_support::decided_negations_hold`, asked by support completion and source activity; `formula_ground::decided_negation` at emission; a producer the component schedule holds back for the atoms it negates; the [rule](architecture/grounding.md#certain-predicates) | `DecidedNegation.absent_false`, `absent_stable`; `CertainAtoms.fixed_stable` |
| An exact assignment: an aggregate assignment every tuple of which is required, so that it has one value | A plain assignment (tuple keys; conditions of positive atoms and decided literals) whose elements `Certainty` lists; emitted as that value | Aggregates are outside the formula language |
| A chain and its members: a conjunction or disjunction read as one node over its operands; in a region, its open operands and its own truth while unknown | The narrowing index's `Chain`; `neutral_operands` counts the operands settled without settling it | `FormulaChains` |
| A shortened chain: one that lost an operand to the region's decisions and keeps two or more members | `Knowledge::shortened`, the mark a closure sets; `Narrower::shortened_weight`, an atom's [score](architecture/execution.md#shortened-chains) | A split preference; `Search.CoverageTree.split` admits any open atom |
| Choice depth: the longest path of dependency components holding a generating head below a head | `GeneratorOrder`'s depth per atom | A split preference, as above |
| A saturation unit: a guard whose holding reaches the candidate, so cut in every countermodel and at the query root | `FrozenPreparation::units`; `saturation_units` in the receipts | `Algebra.saturation_unit`; `LearnedRegions.row_under_unit` for the rows found under it |
| A probe: the narrowing of a copy of the closed query root with a guard and a pattern decided, by the frozen subject in the first probing round and under the rows of the round before in a later one; refuted, the pattern is forbidden, as is a pattern a clause of its group, or of a group whose support it contains, forbids, read off before any probe runs | the preparation's private `Probe`; `probes` and `refuted_patterns` in the receipts | `Region.decideAll`, `Algebra.entailed_by_refutations`; `Algebra.joint_sound` for a later round's subject |

A fact block stores source facts. The dense relation's block above groups
execution tuples sharing a bound prefix; it is a separate representation.

An ordinary Rust `Model` or `logical::AnswerSet` is a collection, not by itself
a sealed answer-set certificate. Likewise, a `Check` value describing a decision should
not be attached to another original input; the checked-owner APIs preserve that
association explicitly.

**Grounding** supplies finite instances or a stream with the required coverage.
**Candidate generation** supplies the interpretations or gate assignments to be
checked. **Membership** decides one candidate against the original program's
reduct. **Enumeration** combines membership with complete candidate coverage.
**Publication** delivers a selected view to a consumer. These operations compose,
but none can substitute for another's evidence.

## Grounding: semantic names and concrete realizations

The following names are relations and laws, not claims of a mechanically proved
Rust refinement. `Lifted` uses abstract bindings; it does not require a
materialized substitution table. The Rust execution algebra is a smaller shared
layer. Grounding does not have a second implementation of a Rust `Bind` API:
there is no such API. See [the composition](architecture/alignment.md#positive-inference-is-a-composition-with-a-fixed-point)
and [correspondence obligations](lean/correspondence.md).

All Rust paths below are relative to `crates/`; Lean paths are relative to
`proofs/Zetesis/`. One Rust cursor can realize several operations in a fused
pass. A representation's name alone establishes no coverage or membership law.

| Lean primitive or law | Rust realization | Relationship and boundary |
| --- | --- | --- |
| `Lifted.Bind` | `zetesis-themelios/src/formula_support.rs` (`Join`); probes in `zetesis-themelios/src/formula_support/queries.rs` and `zetesis-themelios/src/formula_support/order.rs` | Positive witnesses bound by one cursor, fused with scalar checks; no shared Rust relational-combinator API. |
| `Lifted.Filter` (`Template.filter`) | `zetesis-core/src/template.rs` (`Filter`, the template's substituted equality/inequality checks); `zetesis-themelios/src/formula_support/filters.rs`, `zetesis-themelios/src/formula_support/scalar_selection/`; comparisons lowered in `zetesis-themelios/src/formula_ir.rs` | Scalar comparisons evaluated on bindings within that cursor; not a region-refuting subject or an output selection. |
| `Lifted.Gate` / `Enabled` | `zetesis-ferraris/src/reduct.rs`; `zetesis-themelios/src/formula_ground.rs`, `zetesis-themelios/src/formula_support/certain.rs` | Ordinary grounding retains candidate-dependent gates in formulas; a frozen reduct evaluates them later. Decided negations over completed certain predicates can be checked during grounding. |
| `Lifted.Project` / `RuleTransform` | `zetesis-themelios/src/formula_ground.rs` (head emission) | Consumers form heads from completed bindings; distinct bindings may produce the same atom. Factored rules emit heads the same way, but the factoring itself is the `RuleFactorization` rewrite. Not the source `#project` directive. |
| `Union` / `Inter` (`Core.lean`) | `zetesis-themelios/src/formula_support/relations/append.rs`, `zetesis-themelios/src/formula_support/relations/publication.rs`; `zetesis-core/src/catalog/`, `zetesis-core/src/relation/` | Atom identity, pending-round ordering and publication implement set-valued support; occurrence/provenance order is a separate obligation. |
| `Least`, `ComponentClosure` | `zetesis-themelios/src/formula_support.rs` (`complete_with_executor`, `advance_round`), `zetesis-themelios/src/formula_support/producers/` | Component-scheduled possible-support fixed point; not region narrowing or answer-set membership. |
| `DeltaJoins`, `DeltaRounds` | `zetesis-themelios/src/formula_support/delta.rs`, `zetesis-themelios/src/formula_support/producers/` | First-new-occurrence partitions over immutable round snapshots; rich producers retain their full-scan obligations. |
| `JoinFrames` | `zetesis-themelios/src/formula_support.rs` (`Join` prefix and undo state), `zetesis-themelios/src/formula_support/component_plan.rs`, `zetesis-themelios/src/formula_support/row_blocks/component_rows.rs` | Reuses storage but replaces bindings/truth for each scan; mathematical frame laws do not prove machine bounds or scheduling. |
| `ParallelGrounding` | `zetesis-themelios/src/formula_support/row_blocks/coordinator.rs`, `zetesis-themelios/src/formula_support/row_blocks/component_rows.rs`, `zetesis-themelios/src/formula_factor/parallel.rs` | Stages independent head-row blocks and replays in source order; ordered publication and accounting remain concrete obligations. |
| `ColumnRelations`, `CompactColumns`, `TableBindings` | `zetesis-core/src/relation/`, `zetesis-core/src/table/`; `zetesis-themelios/src/formula_support/relations.rs`, `zetesis-themelios/src/formula_support/queries.rs` | Shared typed columns and exact finite-table selection; themelios adds demand-driven postings and completion evidence. |
| `ContentOrderInterning.IdOrderEqCompare` | `zetesis-core/src/intern.rs`, `intern/sort.rs` | Generic generation-freezing interner; this is not yet the live grounding owner. The latter is `catalog::registry::AtomRegistry`, backed by the catalog. Catalog discovery IDs do not acquire ASP order by being integers. |
| None (source lowering) | `zetesis-themelios/src/formula_anonymous_ir.rs` (`AnonymousWitnesses`), `zetesis-themelios/src/formula_conditional_unpool.rs` | Gringo-style anonymous-variable lowering, expanded to a negated witness disjunction `not (w1 ∨ … ∨ wn)` with no auxiliary rules, and conditional-literal pool unpooling. Neither is `Lifted.Project`, and the witness disjunction goes beyond `Template.gateFalse`, which takes atom lists. |
| `RuleFactorization` | `zetesis-themelios/src/formula_factor/plan.rs`, `zetesis-themelios/src/formula_factor.rs` | Existential body factorization; source independence and complete support are separate compiler obligations. |
| `StreamedRegions`, `StreamedConstraints` | `zetesis-themelios/src/formula_hybrid/`, `zetesis-themelios/src/formula_fixed_constraints.rs`; `zetesis-stable/src/constraint_refutation.rs` (`ConstraintRefuter`, a refuting subject: `Algebra.refuting`) | Streamed source checks refute regions or check completed answers; they do not replace reduct membership. |
| `Algebra.joint`, `Inside`, `Subject` | `zetesis-ferraris/src/regions.rs` (`Narrower`); `zetesis-algebra/src/region.rs` | Joint region narrowing is different from the growing support set's `Least`; neither is a relational join. |

### Shared Rust execution primitives used by grounding

| Shared primitive (`zetesis_algebra`) | Grounding consumer / use |
| --- | --- |
| `Cancellation`, `Stop`, `poll_due`, `POLL_INTERVAL` | themelios `expansion.rs`, `formula_support/storage.rs`, and core catalog operations: cooperative stop propagation, never false or empty-result evidence. |
| `Meter`, `MeterBytes`, `MeterWorkError` | themelios `formula_support.rs` and `formula_support/storage.rs`; core `relation.rs`, `table/accounting.rs`, `catalog/storage/budget.rs`, and `atom_lookup/selected.rs`: bounded work and retained storage. |
| `words::Words` | themelios `formula_support/relations/append.rs` supported-atom marks; core `table` and `relation`: packed row selection. |
| `words::union_shifted` and related packed operations | themelios `formula_assignment/sums/packed.rs`: possible assignment sums, not an answer-count fold. |
| `threads::Pool` | themelios `formula_support/parallel_executor.rs`: counted grounding workers, wrapped by the executor. |
| `region::Region` | themelios streamed constraint preparation and source checks: held/cut/open atoms. |
| `reduction::{Any, Reduction}` | themelios `formula_support/row_blocks/objective_rows.rs`: Boolean combination of numeric eligibility. |
| `reduction::sets::{intersection, union, ordered_union}` | No grounding caller. These pure occurrence cursors do not provide the grounder's fallible, charged ordering/publication protocol. |

### Public names outside the algebra

`zetesis_core::{Filter, FilterRef, Filters}` are the equality/inequality part of
`Template.filter` (`Lifted.lean`); the grounder's scalar comparisons realize the
rest of `Lifted.Filter`. `zetesis_core::table::DomainSupport` is finite-table
domain support (`FiniteTables.lean`), not `Lifted.Project`.
`zetesis_themelios::{ProjectDirectives, ProjectiveDomain}` and
`zetesis_solve::Projective*` describe source `#project` answer projection,
clingo's projective enumeration (`ProjectedAnswers.lean`), not head projection.
`zetesis_wgpu::BinaryNodeKernel` selects how a GPU formula node computes its
exact supports (`GateProjection.lean`). `zetesis_core::intern::ContentInterner`
and `zetesis_core::atom_registry::AtomRegistry` are distinct public APIs.
Grounding uses the former for term order: each registry commit and each
vocabulary freeze interns the commit's new terms as one generation and freezes
it, so terms of one generation compare by their content-order identities;
a term of more than 64 expanded nodes gets no identity and compares by content.
Parallel support grounding also uses one per publication: workers intern each
staged row's head key (its projected columns' equality identifiers) through
`&self`, and the coordinator freezes that generation on the grounding pool and
publishes only the first row of each key (`HeadKeyPublication.lean`).
Atom identities are still the registry's discovery positions and term
deduplication is still the catalog's hash index; the rest of the
single-interner migration has not happened. Reserved algebra names (`Bind`, `Filter`, `Gate`, `Project`,
`Region`, `Narrower`, `Closed`, `Walk`, `Reduction`, `Meter`, `Pool`, …) name
only their realizations above.
