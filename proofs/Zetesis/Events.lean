import Zetesis.Transformers

/-!
Abstract event schedules for one frozen reduct operator.

An event adds one atom justified by the current state. Event lists may contain
duplicates, and independent derivations may occur in any legal order. The
model verifies semantic schedules, not hardware counters, packet delivery,
epoch isolation, or a particular quiescence detector. Completion below means
the final state is semantically closed; it is not inferred from silence.
-/

namespace Zetesis

universe u

def InsertAtom {α : Type u} (X : Atoms α) (a : α) : Atoms α :=
  fun b => X b ∨ b = a

def ApplyEvents {α : Type u} (X : Atoms α) : List α → Atoms α
  | [] => X
  | a :: rest => ApplyEvents (InsertAtom X a) rest

/-- A duplicate is allowed: legality requires justification, not novelty. -/
def LegalEvents {α : Type u} (T : Transformer α) (X : Atoms α) : List α → Prop
  | [] => True
  | a :: rest => T X a ∧ LegalEvents T (InsertAtom X a) rest

theorem insertAtom_grows {α : Type u} (X : Atoms α) (a : α) :
    Sub X (InsertAtom X a) := by
  intro b hb
  exact Or.inl hb

theorem insertAtom_contains {α : Type u} (X : Atoms α) (a : α) :
    InsertAtom X a a := Or.inr rfl

theorem insertAtom_duplicate {α : Type u} {X : Atoms α} {a : α}
    (ha : X a) : InsertAtom X a = X := by
  apply atoms_ext
  intro b
  constructor
  · intro hb
    cases hb with
    | inl hx => exact hx
    | inr heq => exact heq ▸ ha
  · exact fun hb => Or.inl hb

theorem insertAtom_idempotent {α : Type u} (X : Atoms α) (a : α) :
    InsertAtom (InsertAtom X a) a = InsertAtom X a :=
  insertAtom_duplicate (insertAtom_contains X a)

theorem applyEvents_grows {α : Type u} (X : Atoms α) (events : List α) :
    Sub X (ApplyEvents X events) := by
  induction events generalizing X with
  | nil => exact sub_refl X
  | cons a rest ih =>
      exact sub_trans (insertAtom_grows X a) (ih (InsertAtom X a))

/-- Splitting an event list into snapshots preserves the execution state. -/
theorem applyEvents_append {α : Type u} (X : Atoms α) (first rest : List α) :
    ApplyEvents X (first ++ rest) = ApplyEvents (ApplyEvents X first) rest := by
  induction first generalizing X with
  | nil => rfl
  | cons a first ih => exact ih (InsertAtom X a)

theorem legalEvents_append {α : Type u} (T : Transformer α)
    (X : Atoms α) (first rest : List α) :
    LegalEvents T X (first ++ rest) ↔
      LegalEvents T X first ∧ LegalEvents T (ApplyEvents X first) rest := by
  induction first generalizing X with
  | nil => exact ⟨fun h => ⟨True.intro, h⟩, fun h => h.2⟩
  | cons a first ih =>
      change (T X a ∧ LegalEvents T (InsertAtom X a) (first ++ rest)) ↔
        (T X a ∧ LegalEvents T (InsertAtom X a) first) ∧
          LegalEvents T (ApplyEvents (InsertAtom X a) first) rest
      rw [ih]
      exact and_assoc.symm

theorem applyEvents_prefix_grows {α : Type u} (X : Atoms α)
    (first rest : List α) :
    Sub (ApplyEvents X first) (ApplyEvents X (first ++ rest)) := by
  rw [applyEvents_append]
  exact applyEvents_grows (ApplyEvents X first) rest

theorem insertAtom_sound {α : Type u} {T : Transformer α}
    (hT : MonotoneT T) {X : Atoms α} {a : α}
    (hX : Sub X (Least T)) (ha : T X a) :
    Sub (InsertAtom X a) (Least T) := by
  intro b hb
  cases hb with
  | inl hx => exact hX b hx
  | inr heq =>
      subst b
      exact least_closed hT a (hT hX a ha)

/-- Each insertion has a finite history of justification. A cyclic supported
set cannot enter through the sound-initial-state hypothesis. -/
theorem legalEvents_sound {α : Type u} {T : Transformer α}
    (hT : MonotoneT T) {X : Atoms α} (events : List α)
    (hX : Sub X (Least T)) (hlegal : LegalEvents T X events) :
    Sub (ApplyEvents X events) (Least T) := by
  induction events generalizing X with
  | nil => exact hX
  | cons a rest ih =>
      exact ih (insertAtom_sound hT hX hlegal.1) hlegal.2

theorem events_from_bottom_sound {α : Type u} {T : Transformer α}
    (hT : MonotoneT T) (events : List α)
    (hlegal : LegalEvents T Empty events) :
    Sub (ApplyEvents Empty events) (Least T) :=
  legalEvents_sound hT events (fun _ h => False.elim h) hlegal

/-- Closedness discharges completeness, independently of event order or
duplicate atom derivations. It must be justified by the execution backend. -/
theorem completed_events_exact {α : Type u} {T : Transformer α}
    (hT : MonotoneT T) (events : List α)
    (hlegal : LegalEvents T Empty events)
    (hclosed : Closed T (ApplyEvents Empty events)) :
    ApplyEvents Empty events = Least T :=
  exact_of_sound_and_closed (events_from_bottom_sound hT events hlegal) hclosed

theorem completed_events_order_independent {α : Type u} {T : Transformer α}
    (hT : MonotoneT T) (first second : List α)
    (hfirst : LegalEvents T Empty first)
    (hsecond : LegalEvents T Empty second)
    (hcfirst : Closed T (ApplyEvents Empty first))
    (hcsecond : Closed T (ApplyEvents Empty second)) :
    ApplyEvents Empty first = ApplyEvents Empty second := by
  rw [completed_events_exact hT first hfirst hcfirst,
      completed_events_exact hT second hsecond hcsecond]

theorem duplicate_events_legal {α : Type u} {T : Transformer α}
    (hT : MonotoneT T) {X : Atoms α} {a : α} (ha : T X a) :
    LegalEvents T X [a, a] :=
  ⟨ha, hT (insertAtom_grows X a) a ha, True.intro⟩

theorem duplicate_events_same_state {α : Type u} (X : Atoms α) (a : α) :
    ApplyEvents X [a, a] = ApplyEvents X [a] :=
  insertAtom_idempotent X a

namespace EventCounterexamples

/-- One unconditional fact. Processing no events is a legal incomplete trace. -/
def FactOperator : Transformer Unit := fun _ => Full

theorem factOperator_monotone : MonotoneT FactOperator := by
  intro X Y h a ha
  exact ha

theorem empty_trace_legal : LegalEvents FactOperator Empty [] := True.intro

theorem empty_trace_not_closed :
    ¬ Closed FactOperator (ApplyEvents Empty []) := by
  intro hc
  exact hc () True.intro

/-- Every interpretation of `a ← a` is closed, including an unjustified atom. -/
theorem self_support_closed : Closed (@IdentityT Unit) Full := by
  intro a ha
  exact ha

theorem self_support_least_empty : Least (@IdentityT Unit) = Empty := by
  apply sub_antisymm
  · exact least_le (fun _ h => h)
  · exact fun _ h => False.elim h

theorem self_support_not_sound : ¬ Sub (@Full Unit) (Least IdentityT) := by
  intro h
  have hf := h () True.intro
  rw [self_support_least_empty] at hf
  exact hf

theorem self_support_has_no_complete_full_trace (events : List Unit)
    (hlegal : LegalEvents IdentityT Empty events) :
    ApplyEvents Empty events ≠ Full := by
  intro heq
  have hsound := events_from_bottom_sound identity_monotone events hlegal
  rw [heq] at hsound
  exact self_support_not_sound hsound

end EventCounterexamples

end Zetesis
