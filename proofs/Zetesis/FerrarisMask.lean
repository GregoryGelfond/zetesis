import Zetesis.Ferraris

/-!
# Frozen truth-mask evaluation

The mask supplies the outer candidate's classical truth at each formula node.
The tested interpretation supplies atom truth during the inner pass. Masking
every node after combining its already-masked children has exactly the truth
of the explicit Ferraris reduct, without constructing that reduct's syntax.

This is a denotational tree-transform theorem. It does not verify the Rust DAG's
array indices, its stored mask, packed membership, resource accounting, or
exhaustive subset counter. No aggregate source translation is assumed or proved.
-/

namespace Zetesis.Ferraris

universe u

variable {α : Type u}

/-- Combine the inner truth of the children, then conjoin the outer frozen mask
    for the current node. The mask is never recomputed in interpretation J. -/
def MaskedEval (mask : Formula α → Prop) (J : Atoms α) : Formula α → Prop
  | .atom a => mask (.atom a) ∧ J a
  | .bot => False
  | .conj F G => mask (.conj F G) ∧ (MaskedEval mask J F ∧ MaskedEval mask J G)
  | .disj F G => mask (.disj F G) ∧ (MaskedEval mask J F ∨ MaskedEval mask J G)
  | .imp F G => mask (.imp F G) ∧ (MaskedEval mask J F → MaskedEval mask J G)

/-- A correct frozen classical mask gives exactly the explicit reduct's truth
    in any tested interpretation, not only subsets of the outer candidate. -/
theorem masked_eval_iff_reduct (M J : Atoms α) (mask : Formula α → Prop)
    (correct : ∀ F, mask F ↔ Satisfies M F) (F : Formula α) :
    MaskedEval mask J F ↔ Satisfies J (Reduct M F) := by
  classical
  induction F with
  | atom a =>
    by_cases h : M a <;> simp [MaskedEval, Reduct, correct, Satisfies, h]
  | bot => rfl
  | conj F G ihF ihG =>
    by_cases hF : Satisfies M F <;> by_cases hG : Satisfies M G
      <;> simp_all [MaskedEval, Reduct, Satisfies]
  | disj F G ihF ihG =>
    by_cases hF : Satisfies M F <;> by_cases hG : Satisfies M G
      <;> simp_all [MaskedEval, Reduct, Satisfies]
  | imp F G ihF ihG =>
    by_cases hF : Satisfies M F <;> by_cases hG : Satisfies M G
      <;> simp_all [MaskedEval, Reduct, Satisfies]

/-- Assert each original root using the same frozen mask. -/
def MaskedModels (mask : Formula α → Prop) (J : Atoms α) (T : Theory α) : Prop :=
  ∀ F, F ∈ T → MaskedEval mask J F

theorem masked_models_iff_reduct (M J : Atoms α) (mask : Formula α → Prop)
    (correct : ∀ F, mask F ↔ Satisfies M F) (T : Theory α) :
    MaskedModels mask J T ↔ Models J (ReductTheory M T) := by
  constructor
  · intro h F hF
    obtain ⟨G, hG, rfl⟩ := List.mem_map.mp hF
    exact (masked_eval_iff_reduct M J mask correct G).mp (h G hG)
  · intro h F hF
    apply (masked_eval_iff_reduct M J mask correct F).mpr
    exact h (Reduct M F) (List.mem_map.mpr ⟨F, hF, rfl⟩)

/-- A correct frozen mask may replace explicit reduct materialization in the
    stability criterion. Minimality still requires all proper subsets. -/
theorem stable_iff_masked_minimality (M : Atoms α) (mask : Formula α → Prop)
    (correct : ∀ F, mask F ↔ Satisfies M F) (T : Theory α) :
    Stable M T ↔ Models M T ∧
      ¬ ∃ J, ProperSub J M ∧ MaskedModels mask J T := by
  simp only [Stable, masked_models_iff_reduct M _ mask correct T]

end Zetesis.Ferraris
