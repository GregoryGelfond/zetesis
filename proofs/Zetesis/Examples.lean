import Zetesis.Semantics

/-! Small semantic counterexamples are proved propositions, not sampled tests. -/
namespace Zetesis.Examples

open Semantics

/-- Normalized singleton choice `{a}.`: the head is a gate, not an antecedent. -/
def ChoiceRule : Rule Unit :=
  ⟨some (), [], [()], [], True⟩

def ChoiceProgram : Program Unit := [ChoiceRule]

theorem choice_consequence (z X : Atoms Unit) (a : Unit) :
    Consequence ChoiceProgram z X a ↔ z a := by
  cases a
  simp [Consequence, ChoiceProgram, ChoiceRule, Gate, Body]
  constructor
  · intro h
    exact h ()
  · intro h a
    cases a
    exact h

theorem choice_gamma (z : Atoms Unit) : Gamma ChoiceProgram z = z := by
  apply sub_antisymm
  · apply gamma_le
    intro a ha
    exact (choice_consequence z z a).mp ha
  · intro a ha
    exact gamma_closed ChoiceProgram z a
      ((choice_consequence z (Gamma ChoiceProgram z) a).mpr ha)

theorem choice_empty_stable : Stable ChoiceProgram Empty := by
  apply (stable_iff_gamma _ _).mpr
  refine ⟨choice_gamma _, ?_⟩
  simp [ConstraintsOK, ChoiceProgram, ChoiceRule]

theorem choice_full_stable : Stable ChoiceProgram Full := by
  apply (stable_iff_gamma _ _).mpr
  refine ⟨choice_gamma _, ?_⟩
  simp [ConstraintsOK, ChoiceProgram, ChoiceRule]

/-- Choice/double-negation prevents assuming antimonotonicity of Gamma. -/
theorem choice_gamma_not_antimonotone :
    ¬ (∀ z w : Atoms Unit, Sub z w →
      Sub (Gamma ChoiceProgram w) (Gamma ChoiceProgram z)) := by
  intro h
  have bad := h Empty Full (fun _ he => False.elim he)
  rw [choice_gamma, choice_gamma] at bad
  exact bad () True.intro

def FactRule : Rule Unit := ⟨some (), [], [], [], True⟩
def NeedsFactConstraint : Rule Unit := ⟨none, [], [], [()], True⟩
def FactConstraintProgram : Program Unit := [FactRule, NeedsFactConstraint]

theorem fact_constraint_consequence (z X : Atoms Unit) (a : Unit) :
    Consequence FactConstraintProgram z X a := by
  cases a
  exact ⟨FactRule, by simp [FactConstraintProgram], rfl, True.intro,
    ⟨by simp [FactRule], by simp [FactRule]⟩, by simp [Body, FactRule]⟩

theorem fact_constraint_gamma (z : Atoms Unit) :
    Gamma FactConstraintProgram z = Full := by
  apply sub_antisymm
  · intro _ _
    trivial
  · intro a _
    exact gamma_closed FactConstraintProgram z a (fact_constraint_consequence _ _ _)

theorem fact_constraint_stable : Stable FactConstraintProgram Full := by
  apply (stable_iff_gamma _ _).mpr
  refine ⟨fact_constraint_gamma _, ?_⟩
  simp [ConstraintsOK, FactConstraintProgram, FactRule, NeedsFactConstraint, Gate, Full]

/-- Omitting a constraint's gate from S can destroy seed completeness. -/
theorem missing_constraint_gate_loses_model :
    Stable FactConstraintProgram Full ∧
      ¬ Accept FactConstraintProgram Empty (Inter Full Empty) := by
  refine ⟨fact_constraint_stable, ?_⟩
  intro h
  have hc := h.2 NeedsFactConstraint (by simp [FactConstraintProgram]) rfl True.intro
  apply hc
  · simp [Gate, NeedsFactConstraint, Inter, Empty]
  · simp [Body, NeedsFactConstraint]

theorem missing_constraint_gate_not_carrier :
    ¬ GateCarrier FactConstraintProgram Empty := by
  intro h
  exact (h NeedsFactConstraint (by simp [FactConstraintProgram])).2 ()
    (by simp [NeedsFactConstraint])

end Zetesis.Examples
