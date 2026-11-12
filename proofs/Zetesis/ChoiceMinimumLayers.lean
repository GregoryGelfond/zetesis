import Zetesis.RequiredChoiceCosts

/-!
A candidate-only lower bound represented by nonnegative weight increments.
This proves the value bound. Source ownership, the Boolean prefix compiler,
finite-width arithmetic and Rust execution remain separate obligations.
-/
namespace Zetesis.ChoiceMinimumLayers

open AggregateAssignment (weightedSum)
open RequiredChoiceCosts (contribution)

universe u
variable {κ : Type u}

def OrderedFrom (base : Int) : List Int → Prop
  | [] => True
  | level :: rest => base ≤ level ∧ OrderedFrom level rest

def increments (excluded : Int → Bool) (base : Int) : List Int → Int
  | [] => 0
  | level :: rest =>
      (if excluded level then level - base else 0) + increments excluded level rest

def lower (active : Bool) (excluded : Int → Bool) (base : Int) (levels : List Int) : Int :=
  if active then base + increments excluded base levels else 0

/-- A selected eligible key prevents charging an increment beyond its weight.
    Nonconsecutive charged increments can only make the value smaller. -/
theorem increments_le (excluded : Int → Bool) (levels : List Int)
    (base witness : Int) (ordered : OrderedFrom base levels)
    (sound : ∀ level ∈ levels, excluded level = true → level ≤ witness) :
    increments excluded base levels ≤ if base ≤ witness then witness - base else 0 := by
  induction levels generalizing base with
  | nil =>
    simp only [increments]
    split <;> omega
  | cons level rest ih =>
    obtain ⟨step, tail⟩ := ordered
    have remaining := ih level tail (fun other member => sound other (by simp [member]))
    by_cases within : level ≤ witness
    · have initial : base ≤ witness := by omega
      simp only [if_pos within] at remaining
      simp only [increments, if_pos initial]
      cases selected : excluded level <;> simp only [Bool.false_eq_true, ↓reduceIte] <;> omega
    · have absent : excluded level = false := by
        cases selected : excluded level with
        | false => rfl
        | true => exact False.elim (within (sound level (by simp) selected))
      simp only [if_neg within] at remaining
      simp only [increments, absent, Bool.false_eq_true, ↓reduceIte]
      split <;> omega

/-- An active mandatory group has an eligible witness. Its layer value cannot
    exceed that witness's contribution or the whole nonnegative group cost. -/
theorem group_lower_le (keys : List κ) (weight : κ → Int) (eligible : κ → Bool)
    (active : Bool) (excluded : Int → Bool) (base : Int) (levels : List Int)
    (ordered : OrderedFrom base levels)
    (nonnegative : ∀ key ∈ keys, 0 ≤ weight key)
    (initial : ∀ key ∈ keys, base ≤ weight key)
    (required : active = true → ∃ key ∈ keys, eligible key = true)
    (sound : ∀ level ∈ levels, excluded level = true →
      ∀ key ∈ keys, eligible key = true → level ≤ weight key) :
    lower active excluded base levels ≤ weightedSum (contribution weight eligible) keys := by
  have terms : ∀ key ∈ keys, 0 ≤ contribution weight eligible key := by
    intro key member
    have bounded := nonnegative key member
    unfold contribution
    split <;> omega
  cases activation : active with
  | false =>
    simpa [lower, activation] using RequiredChoiceCosts.sum_nonnegative _ keys terms
  | true =>
    obtain ⟨key, member, enabled⟩ := required activation
    have baseBound := initial key member
    have layers := increments_le excluded levels base (weight key) ordered
      (fun level inside selected => sound level inside selected key member enabled)
    have total := RequiredChoiceCosts.member_le_sum _ keys key member terms
    simp only [if_pos baseBound] at layers
    simp only [contribution, enabled, ↓reduceIte] at total
    simp only [lower, ↓reduceIte]
    omega

/-- Two independent necessary restrictions are conjoined, never added as costs. -/
theorem conjoin_necessary (accepted first second : Prop)
    (firstNecessary : accepted → first) (secondNecessary : accepted → second) :
    (accepted ∧ first ∧ second) ↔ accepted := by
  constructor
  · exact fun proof => proof.1
  · exact fun proof => ⟨proof, firstNecessary proof, secondNecessary proof⟩

end Zetesis.ChoiceMinimumLayers
