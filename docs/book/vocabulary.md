# Vocabulary

The first column gives the logical concept; the others locate its current
representations. A representation name does not confer a semantic property.

| Concept | Current Rust representation | Lean vocabulary |
| --- | --- | --- |
| Source program | `themelios_program::Program`, re-exported as `zetesis_themelios::logical::Program` | Source-template and translation laws use explicit abstract premises |
| Admitted relational program | `zetesis_core::Program` | `Semantics.Program` |
| Finite propositional theory | `zetesis_ferraris::Theory` | `Ferraris.Theory` |
| Interpretation | `zetesis_core::Interpretation` / `Model`; theory-bound `zetesis_ferraris::Interpretation` | `Atoms α` |
| Candidate gate assignment | `zetesis_core::Seed` | Seed `z` and `Semantics.GateCarrier` |
| Answer set | Privately constructed `AnswerSet` (`SessionModel` compatibility alias), or a native `StableInterpretation` | `Semantics.Stable` / `Ferraris.Stable` |
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
| A tuple, a block of tuples sharing a bound prefix, the pending marks of a layout | the dense relations' `Tuple`, `Block` and `block_steps`, `PendingMarks` | None |
| A statistics record; an admission's usage; a receipt | `RegionCounts` and its kin; `ExpansionUsage`; a `receipt` in `zetesis-core` is a value proving an operation completed | None |
| This solver's run and the reference solver | `native` and `reference` in the series view; clingo is the tool filling the reference's role | None |
| A requested solver configuration | a validation `profile`, a request for a `SolveConfig` | None |

An ordinary Rust `Model` is an atom collection, not by itself a sealed
answer-set certificate. Likewise, a `Check` value describing a decision should
not be attached to another subject; the checked-owner APIs preserve that
association explicitly.

**Grounding** supplies finite instances or a stream with the required coverage.
**Candidate generation** supplies the interpretations or gate assignments to be
checked. **Membership** decides one candidate against the original program's
reduct. **Enumeration** combines membership with complete candidate coverage.
**Publication** delivers a selected view to a consumer. These operations compose,
but none can substitute for another's evidence.
