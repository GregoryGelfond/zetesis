import Zetesis.Ferraris

/-!
# Double-negated candidate guards

Double negation freezes the truth of any formula in the candidate. Therefore a
double-negated formula can filter classical candidates while contributing no
restriction to the reduct of a candidate that satisfies the guard. This module
proves the generic law for a single guard and a finite family of guards.

These are formula-level results. They do not prove that a particular graph
translation supplies necessary supportedness conditions, or that the Rust
translation or SAT implementation refines these definitions.
-/

namespace Zetesis.Ferraris

universe u

variable {α : Type u}

/-- Classical double negation has the truth value of its guarded formula. -/
theorem double_neg_satisfies (M : Atoms α) (F : Formula α) :
    Satisfies M (Neg (Neg F)) ↔ Satisfies M F := by
  classical
  simp [Neg, Satisfies]

/-- A candidate-true formula's double negation reduces literally to truth. -/
theorem double_neg_reduct_eq_truth (M : Atoms α) (F : Formula α)
    (hF : Satisfies M F) :
    Reduct M (Neg (Neg F)) = .imp .bot .bot := by
  classical
  simp [Neg, Reduct, Satisfies, hF]

/-- The reduct of a double-negated arbitrary formula depends only on its frozen
    candidate truth, and is independent of the tested interpretation. -/
theorem double_neg_formula_reduct (M J : Atoms α) (F : Formula α) :
    Satisfies J (Reduct M (Neg (Neg F))) ↔ Satisfies M F := by
  classical
  by_cases hF : Satisfies M F <;> simp [Neg, Reduct, Satisfies, hF]

/-- Conjoining one asserted root separates its satisfaction from the tail. -/
theorem models_cons (M : Atoms α) (F : Formula α) (T : Theory α) :
    Models M (F :: T) ↔ Satisfies M F ∧ Models M T := by
  simp [Models]

/-- A double-negated root imposes only candidate truth on a reduct model. -/
theorem models_reduct_cons_double_neg (M J : Atoms α) (F : Formula α)
    (T : Theory α) :
    Models J (ReductTheory M (Neg (Neg F) :: T)) ↔
      Satisfies M F ∧ Models J (ReductTheory M T) := by
  change Models J (Reduct M (Neg (Neg F)) :: ReductTheory M T) ↔ _
  rw [models_cons, double_neg_formula_reduct]

/-- A double-negated root filters stable models by candidate truth; it cannot
    create a stable model that was absent from the original theory. -/
theorem stable_cons_double_neg_iff (M : Atoms α) (F : Formula α)
    (T : Theory α) :
    Stable M (Neg (Neg F) :: T) ↔ Stable M T ∧ Satisfies M F := by
  classical
  by_cases hF : Satisfies M F
  · simp [Stable, models_cons, double_neg_satisfies,
      models_reduct_cons_double_neg, hF]
  · simp [Stable, models_cons, double_neg_satisfies, hF]

/-- For a candidate already satisfying the guard, stable membership is unchanged. -/
theorem stable_cons_double_neg_iff_of_satisfies (M : Atoms α) (F : Formula α)
    (T : Theory α) (hF : Satisfies M F) :
    Stable M (Neg (Neg F) :: T) ↔ Stable M T := by
  rw [stable_cons_double_neg_iff]
  simp [hF]

/-- Turn a finite family of candidate predicates into asserted frozen guards. -/
def GuardTheory (guards : Theory α) : Theory α :=
  guards.map (fun F => Neg (Neg F))

/-- A finite guard family retains exactly the original stable models satisfying
    every guarded formula, including empty and duplicate guard families. -/
theorem stable_guard_theory_iff (M : Atoms α) (T guards : Theory α) :
    Stable M (GuardTheory guards ++ T) ↔ Stable M T ∧ Models M guards := by
  induction guards with
  | nil => simp [GuardTheory, Models]
  | cons F guards ih =>
    change Stable M (Neg (Neg F) :: (GuardTheory guards ++ T)) ↔ _
    rw [stable_cons_double_neg_iff, ih, models_cons]
    constructor
    · rintro ⟨⟨hstable, hguards⟩, hF⟩
      exact ⟨hstable, hF, hguards⟩
    · rintro ⟨hstable, hF, hguards⟩
      exact ⟨⟨hstable, hguards⟩, hF⟩

/-- Guards entailed by every original stable model preserve the entire stable
    model set. Establishing that entailment for a concrete compiler is a separate
    obligation supplied explicitly as the hypothesis of this theorem. -/
theorem stable_guard_theory_iff_of_stable_entails (T guards : Theory α)
    (entails : ∀ M, Stable M T → Models M guards) (M : Atoms α) :
    Stable M (GuardTheory guards ++ T) ↔ Stable M T := by
  rw [stable_guard_theory_iff]
  constructor
  · exact And.left
  · intro hstable
    exact ⟨hstable, entails M hstable⟩

end Zetesis.Ferraris
