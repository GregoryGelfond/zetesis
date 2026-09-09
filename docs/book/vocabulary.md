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
