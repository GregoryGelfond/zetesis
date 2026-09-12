import Zetesis.Semantics

/-!
# Positive gate restrictions from actual facts

A ground constraint with no negative gates forbids the conjunction of its gate
premises when every remaining positive antecedent is an unconditional fact.
Such a witness excludes every seed containing those premises. In a binary seed
counter it also excludes each region that changes only bits below the witness's
least selected index.

These laws preserve the normalized reduct's answer-set projections. Source
binding coverage, Rust join/copy operations, counter arithmetic and machine
resource accounting require separate refinements; an incomplete source scan
cannot claim that all usable restrictions were found.
-/
namespace Zetesis.GateRestrictions

universe u
variable {α : Type u}
open Semantics

/-- An actual unconditional fact, not merely an atom in possible support. -/
def Fact (P : Program α) (a : α) : Prop :=
  ∃ r ∈ P, r.head = some a ∧ r.positive = [] ∧
    r.gateTrue = [] ∧ r.gateFalse = [] ∧ r.filter

/-- Every model of a frozen reduct contains each actual unconditional fact. -/
theorem fact_in_model (P : Program α) (z M : Atoms α) (a : α)
    (fact : Fact P a) (model : ReductModel P z M) : M a := by
  obtain ⟨r, asserted, head, positive, trueGates, falseGates, filter⟩ := fact
  apply model.1
  refine ⟨r, asserted, head, filter, ?_, ?_⟩
  · constructor
    · intro b member
      simp [trueGates] at member
    · intro b member
      simp [falseGates] at member
  · intro b member
    simp [positive] at member

/-- A ground source constraint certifies an upward-closed forbidden conjunction.
Every positive antecedent is an actual fact or a premise, and every true gate is
a premise. This does not require all program constraints to have this form. -/
def Witness (P : Program α) (premise : Atoms α) : Prop :=
  ∃ r ∈ P, r.head = none ∧ r.filter ∧ r.gateFalse = [] ∧
    (∀ a, a ∈ r.positive → Fact P a ∨ premise a) ∧
    (∀ a, a ∈ r.gateTrue → premise a)

/-- No answer-set projection contains an entire certified conjunction.
Actual facts belong to the answer set by closure. Contained gate premises then
make the witnessed original constraint true, contradicting its satisfaction. -/
theorem answer_set_avoids (P : Program α) (S premise M : Atoms α)
    (witness : Witness P premise) (answer : Stable P M) :
    ¬ Sub premise (Inter M S) := by
  intro contained
  obtain ⟨r, asserted, head, filter, falseGates, positives, trueGates⟩ := witness
  have enabled : Gate r M := by
    constructor
    · intro a member
      exact (contained a (trueGates a member)).1
    · intro a member
      simp [falseGates] at member
  have body : Body r M := by
    intro a member
    rcases positives a member with fact | gate
    · exact fact_in_model P M M a fact answer.1
    · exact (contained a gate).1
  exact answer.1.2 r asserted head filter enabled body

/-- Supersets of a forbidden conjunction remain forbidden. -/
theorem upward_rejection (P : Program α) (S premise smaller larger : Atoms α)
    (witness : Witness P premise) (contained : Sub premise smaller)
    (extension : Sub smaller larger) :
    ¬ ∃ M, Stable P M ∧ Inter M S = larger := by
  rintro ⟨M, answer, projection⟩
  apply answer_set_avoids P S premise M witness answer
  rw [projection]
  exact sub_trans contained extension

/-- All interpretations agreeing above a cut retain a conjunction whose least
index is at least that cut. This is the region excluded by the binary jump;
low-index choices cannot repair a conjunction of unchanged true premises. -/
theorem suffix_region_rejected (P : Program Nat) (S premise current next : Atoms Nat)
    (witness : Witness P premise) (cut : Nat)
    (above : ∀ index, premise index → cut ≤ index)
    (selected : Sub premise current)
    (unchanged : ∀ index, cut ≤ index → (current index ↔ next index)) :
    ¬ ∃ M, Stable P M ∧ Inter M S = next := by
  apply upward_rejection P S premise premise next witness (sub_refl premise)
  intro index member
  exact (unchanged index (above index member)).mp (selected index member)

end Zetesis.GateRestrictions
