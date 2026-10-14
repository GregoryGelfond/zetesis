# Vocabulary

The first column gives the logical concept; the others locate its current
representations. A representation name does not confer a semantic property.

| Concept | Current Rust representation | Lean vocabulary |
| --- | --- | --- |
| Canonical logical program | `themelios_program::Program`, re-exported as `zetesis_themelios::logical::Program`; constructed directly or raised from source | Source-template and translation laws use explicit abstract premises |
| Original statement evidence | `zetesis_themelios::ProgramSite` carries an owner-local `StatementId` when available and an optional real `Location` | Identity and provenance do not by themselves establish a translation law |
| Admitted relational program | `zetesis_core::Program` | `Semantics.Program` |
| Finite propositional theory | `zetesis_ferraris::Theory` | `Ferraris.Theory` |
| Interpretation | `zetesis_core::Interpretation` / `Model`; theory-bound `zetesis_ferraris::Interpretation` | `Atoms α` |
| Candidate gate assignment | `zetesis_core::Seed` | Seed `z` and `Semantics.GateCarrier` |
| Checked answer set | Privately constructed `zetesis_solve::AnswerSet` (`SessionModel` compatibility alias), or a native `StableInterpretation` | `Semantics.Stable` / `Ferraris.Stable` |
| Ground-symbol exchange set | `zetesis_themelios::logical::AnswerSet`, a plain `BTreeSet<Symbol>` used by explicit symbol export | A set representation with no checked membership claim |
| Complete world view | `WorldView` retains the original subject and all full answers after unrestricted exhaustion | Membership, original-family coverage and complete-capture laws |
| Positive consequence closure | CPU normal oracle result; static or lazy representation | `Semantics.Gamma`, `Zetesis.Least` |
| Frozen reduct | `zetesis_ferraris::FrozenReduct` or candidate-gated normal consequences | `Ferraris.ReductTheory` / `Semantics.ReductModel` |
| Display | Source observations selecting shown atoms and terms | Distinct from full interpretation |
| Proved optimum | `SemanticOutcome::optimum_proved()` and retained objective evidence | Objective ordering and completed-search laws |
| The sides of a region: held, cut, open | `Region::is_held`, `is_cut`, `is_open`; the carrier's `held_gate_atoms` and `cut_gate_atoms`; the cube's `must` is the held set and the complement of `may` the cut set | `Cube`, `Bounds.undecided` for the open cube |
| The two readings of a formula under a region: sure, never | `Knowledge`, known to hold or known never | `FormulaBounds.Sure`, `FormulaBounds.Never` |
| A region visited as a leaf | `RegionStatistics::leaves`, the carrier's `regions_leaves`, the proposers' `leaves` | `decided_leaf_models` |
| The carrier's narrowing by its two closures | `NarrowingState`, `narrowing_passes`, `narrowing_stop`, `root_refuted`; the record key `carrier_narrowing` | `Bounds.lean` |
| An argument's bound | `argument_bounds::infer`, `Bound` | None; the inference serves preparation |
| An integer comparison form: a comparison literal's operands compiled once, when the compiler constructs the literal, for deciding it with machine integers | `formula_ir::Comparison`, `formula_ir::integer` (`IntegerComparison`) and `formula_support::integer_comparisons`; the grounding counters `integer_comparisons` and `integer_comparison_declines` (rows decided and declined); `ComparisonForms` (the prepared program's literals with a form, `with_form`, and without one by reason) | None; the form's verdict is the ASP order restricted to numbers |
| A checked numeric index: one immutable relation snapshot indexed by a column-mapped expression's integer value; equality and range probes retain source row order | `formula_support::queries::expression_keys`; `numeric_index_builds` and `numeric_index_declines` count completed certificates and data declines; `expression_key_probes`/`range_probes` count queries and `expression_key_rows`/`range_probe_rows` count visited rows | The residual comparison and first-new partition remain required; index totality is checked at build time |
| A tuple, a block of tuples sharing a bound prefix, the pending marks of a layout | the dense relations' `Tuple`, `Block` and `block_steps`, `PendingMarks` | None |
| A fact block: one predicate's fact rows at one position of the prepared program, in a product or run layout | `formula_facts::FactBlock`, traversed through `Item::Facts`; its [layout rule](architecture/grounding.md#facts-as-fact-blocks) preserves the fact rules' order | None; each row is a body-free ground rule |
| A positive body atom named from a held matched row, with an unchanged-prefix node reused when current | `formula_support::witnesses::WitnessRow`, `formula_ground::matched_nodes::MatchedNodes`; `row_atoms` counts successful ordinary-emission/capture names and `row_atom_reuses` its reused subset, excluding the producer and element joins | `AtomCatalogs.identity_determines_position`, `DagSharing.intern_stable_under_extension`; matching, lifetime and cache currency remain runtime obligations |
| A complete answer count: the size of the selected full answer-set family, or its optimal ties, established only after exhaustion | `SemanticOutcome::complete_answer_count`, `SessionBuilder::counting`, `PublicationMode::Count` and `CountRenderer`; `--count` prints the count and ignores display/projection identity | Original answer-set membership and complete selected-family coverage are both required |
| A statistics record; an admission's usage; a receipt | `RegionCounts` and its kin; `ExpansionUsage`; a `receipt` in `zetesis-core` is a value proving an operation completed | None |
| This solver's run and the reference solver | `native` and `reference` in the series view; clingo is the tool filling the reference's role | None |
| A requested solver configuration | a validation `profile`, a request for a `SolveConfig` | None |
| An ongoing solve | `zetesis_solve::Session` owns search and membership execution | Membership and coverage laws describe its semantic obligations |
| Cooperative cancellation and a deadline | `zetesis_cpu::Cancellation` carries shared cancellation and an optional absolute deadline | Stopped coverage remains distinct from exhaustion |

A fact block stores source facts. The dense relation's block above groups
execution tuples sharing a bound prefix; it is a separate representation.

An ordinary Rust `Model` or `logical::AnswerSet` is a collection, not by itself
a sealed answer-set certificate. Likewise, a `Check` value describing a decision should
not be attached to another subject; the checked-owner APIs preserve that
association explicitly.

**Grounding** supplies finite instances or a stream with the required coverage.
**Candidate generation** supplies the interpretations or gate assignments to be
checked. **Membership** decides one candidate against the original program's
reduct. **Enumeration** combines membership with complete candidate coverage.
**Publication** delivers a selected view to a consumer. These operations compose,
but none can substitute for another's evidence.
