import Zetesis.BatchAccounting

/-!
# Semantic outcomes and publication

A completed membership ledger establishes results in its covered candidate region.
An external consumer may publish only some verified values, or none at all. The
absence of published records is therefore distinct from semantic unsatisfiability.

Instantiate `required` with original-theory stable membership for ASP. The finite
carrier must cover the relevant problem before regional absence implies original
UNSAT. Producer coverage and sound classification remain explicit premises.
These laws do not verify Rust session transitions, byte accounting, sink writes,
output durability, optimization tie selection, or source/solver correspondence.
-/
namespace Zetesis.Outcomes
universe u
variable {α : Type u}

/-- No candidate in the stated region has the required semantic property. -/
def Unsatisfiable (carrier : List α) (required : α → Prop) : Prop :=
  ∀ a, a ∈ carrier → ¬ required a

/-- Every delivered value was previously accepted by complete membership checking.
    Order, multiplicity, and actual sink acknowledgement are separate obligations. -/
def DeliverySound (s : BatchAccounting.Ledger α) (delivered : List α) : Prop :=
  ∀ a, a ∈ delivered → a ∈ s.accepted

/-- Completed exact membership establishes regional UNSAT precisely when there
    are no accepted values. No publication premise is required or sufficient. -/
theorem completed_unsatisfiable_iff_empty (carrier : List α)
    (required : α → Prop) (s : BatchAccounting.Ledger α)
    (covered : BatchAccounting.Covers carrier s)
    (sound : BatchAccounting.Sound required s)
    (done : BatchAccounting.Complete s) :
    Unsatisfiable carrier required ↔ s.accepted = [] := by
  have membership_exact : ∀ a, a ∈ s.accepted ↔ a ∈ carrier ∧ required a := by
    intro a
    exact BatchAccounting.completed_results_exact carrier s required covered sound done a
  constructor
  · intro unsatisfiable
    have no_accepted_value : ∀ a, a ∉ s.accepted := by
      intro a accepted
      obtain ⟨inside, valid⟩ := (membership_exact a).mp accepted
      exact unsatisfiable a inside valid
    exact List.eq_nil_iff_forall_not_mem.mpr no_accepted_value
  · intro empty
    show ∀ a, a ∈ carrier → ¬ required a
    intro a inside valid
    have accepted : a ∈ s.accepted := (membership_exact a).mpr ⟨inside, valid⟩
    have absent : a ∉ s.accepted := by simp [empty]
    exact absent accepted

/-- Publication can select accepted values without weakening their membership
    validity. This law does not claim that every valid value was delivered. -/
theorem delivered_values_are_valid (required : α → Prop)
    (s : BatchAccounting.Ledger α) (delivered : List α)
    (sound : BatchAccounting.Sound required s)
    (delivery : DeliverySound s delivered) :
    ∀ a, a ∈ delivered → required a := by
  intro a published
  have accepted : a ∈ s.accepted := delivery a published
  exact sound.1 a accepted

/-- A complete sound ledger can contain a valid model even when nothing was
    published. Zero delivered records cannot supply the empty-accepted premise. -/
theorem empty_delivery_can_hide_a_valid_model :
    ∃ s : BatchAccounting.Ledger Unit,
      BatchAccounting.Covers [()] s ∧
      BatchAccounting.Sound (fun _ => True) s ∧
      BatchAccounting.Complete s ∧ DeliverySound s [] ∧
      ¬ Unsatisfiable [()] (fun _ => True) := by
  let s : BatchAccounting.Ledger Unit := ⟨[], [], [()], [], false⟩
  have covered : BatchAccounting.Covers [()] s := by
    intro a
    simp [BatchAccounting.Accounted, s]
  have sound : BatchAccounting.Sound (fun _ => True) s := by
    simp [BatchAccounting.Sound, s]
  have complete : BatchAccounting.Complete s := by
    simp [BatchAccounting.Complete, s]
  have empty_delivery : DeliverySound s [] := by
    simp [DeliverySound]
  have satisfiable : ¬ Unsatisfiable [()] (fun _ => True) := by
    intro absent
    exact absent () (by simp) True.intro
  exact ⟨s, covered, sound, complete, empty_delivery, satisfiable⟩

end Zetesis.Outcomes
