import Zetesis.CountEligibility

/-!
# Frozen negative eligibility

CountEligibility already factors tuple activity into its representative head and
an arbitrary retained eligibility formula. These corollaries describe that
activity when eligibility is default-negated or double-default-negated. The
head must remain true in both interpretations; the gate reads only the frozen
candidate. Classical equivalence of gates is insufficient for replacement.

These are formula laws. Complete possible rows, tuple/atom correspondence,
coalesced witnesses and preservation of source eligibility remain premises at
the count-head boundary. In particular, no Rust scope checker, anonymous-argument
projection, join completeness, arithmetic or resource accounting is verified.
-/

namespace Zetesis.NegativeEligibility
open Ferraris
universe u
variable {A : Type u}

/-- Negative eligibility can select a head only when the candidate falsifies
    the original gate, regardless of the gate's truth in the reduct world. -/
theorem negative_activity_frozen (M J : Atoms A) (head : A) (gate : Formula A) :
    Satisfies J (Reduct M (.conj (.atom head) (Neg gate))) ↔
      M head ∧ J head ∧ ¬ Satisfies M gate := by
  have head_frozen : Satisfies J (Reduct M (.atom head)) ↔ M head ∧ J head := by
    classical
    by_cases present : M head <;> simp [Reduct, Satisfies, present]
  have gate_frozen : Satisfies J (Reduct M (Neg gate)) ↔ ¬ Satisfies M gate :=
    ChoiceIntervals.negation_frozen M J gate
  rw [RuleFactorization.reduct_conj, head_frozen, gate_frozen]
  exact and_assoc

/-- Double-negative eligibility selects according to candidate truth; it does
    not demand that the gate be supported again in the reduct interpretation. -/
theorem double_negative_activity_frozen (M J : Atoms A) (head : A)
    (gate : Formula A) :
    Satisfies J (Reduct M (.conj (.atom head) (Neg (Neg gate)))) ↔
      M head ∧ J head ∧ Satisfies M gate := by
  have head_frozen : Satisfies J (Reduct M (.atom head)) ↔ M head ∧ J head := by
    classical
    by_cases present : M head <;> simp [Reduct, Satisfies, present]
  have gate_frozen : Satisfies J (Reduct M (Neg (Neg gate))) ↔ Satisfies M gate :=
    double_neg_formula_reduct M J gate
  rw [RuleFactorization.reduct_conj, head_frozen, gate_frozen]
  exact and_assoc

/-- Replacing a double-negative gate by its classically equal positive formula
    changes frozen truth even for a proper-subset interpretation. -/
theorem double_negative_gate_is_not_positive :
    ¬ ChoiceIntervals.Equivalent (Neg (Neg (.atom ()))) (.atom ()) := by
  intro same
  have gate_true : Satisfies (fun _ : Unit => False)
      (Reduct (fun _ => True) (Neg (Neg (.atom ())))) :=
    (double_neg_formula_reduct (fun _ => True) (fun _ => False) (.atom ())).mpr trivial
  have head_false : ¬ Satisfies (fun _ : Unit => False)
      (Reduct (fun _ => True) (.atom ())) := by
    simp only [Reduct, Satisfies, ↓reduceIte, not_false_eq_true]
  exact head_false ((same.2 (fun _ => True) (fun _ => False)).mp gate_true)

end Zetesis.NegativeEligibility
