import Std

/-! Finite numeric min/max candidate coverage. The empty sentinels are genuine
constructors, distinct from every finite integer. This proves the value carrier
obligation for a completed covering tuple set, not Rust/source/cache refinement. -/
namespace Zetesis.ExtremumCandidates

inductive Value where
  | infimum
  | number (value : Int)
  | supremum
  deriving DecidableEq

def choose (minimum : Bool) (left right : Int) : Int :=
  if minimum then (if left ≤ right then left else right)
  else (if left ≤ right then right else left)

def extreme (minimum : Bool) : List Int → Option Int
  | [] => none
  | head :: tail => match extreme minimum tail with
    | none => some head
    | some best => some (choose minimum head best)

def encode (minimum : Bool) : Option Int → Value
  | none => if minimum then .supremum else .infimum
  | some value => .number value

def candidates (minimum : Bool) (possible : List Int) : List Value :=
  encode minimum none :: possible.map Value.number

theorem choose_is_input (minimum : Bool) (left right : Int) :
    choose minimum left right = left ∨ choose minimum left right = right := by
  cases minimum <;> simp only [choose, Bool.false_eq_true, ↓reduceIte]
  all_goals split <;> simp

theorem extreme_none_iff (minimum : Bool) (values : List Int) :
    extreme minimum values = none ↔ values = [] := by
  cases values with
  | nil => simp [extreme]
  | cons head tail =>
    simp only [extreme, List.cons_ne_nil, iff_false]
    cases extreme minimum tail <;> simp

theorem extreme_some_mem (minimum : Bool) (values : List Int) (value : Int)
    (result : extreme minimum values = some value) : value ∈ values := by
  induction values generalizing value with
  | nil => simp [extreme] at result
  | cons head tail ih =>
    cases ht : extreme minimum tail with
    | none => exact List.mem_cons.mpr (Or.inl (by simpa [extreme, ht] using result.symm))
    | some best =>
      have same : choose minimum head best = value := by simpa [extreme, ht] using result
      rcases choose_is_input minimum head best with left | right
      · exact List.mem_cons.mpr (Or.inl (same.symm.trans left))
      · exact List.mem_cons.mpr (Or.inr (by
          have member : best ∈ tail := ih best ht
          simpa [same.symm.trans right] using member))

theorem complete_candidate_coverage (minimum : Bool) (actual possible : List Int)
    (coverage : ∀ value ∈ actual, value ∈ possible) :
    encode minimum (extreme minimum actual) ∈ candidates minimum possible := by
  cases result : extreme minimum actual with
  | none => simp [candidates]
  | some value =>
    apply List.mem_cons.mpr
    apply Or.inr
    apply List.mem_map.mpr
    exact ⟨value, coverage value (extreme_some_mem minimum actual value result), rfl⟩

theorem empty_maximum_is_infimum : encode false (extreme false []) = .infimum := rfl
theorem empty_minimum_is_supremum : encode true (extreme true []) = .supremum := rfl
theorem finite_ne_infimum (value : Int) : Value.number value ≠ .infimum := by intro impossible; cases impossible
theorem finite_ne_supremum (value : Int) : Value.number value ≠ .supremum := by intro impossible; cases impossible

theorem candidate_length (minimum : Bool) (possible : List Int) :
    (candidates minimum possible).length = possible.length + 1 := by simp [candidates]

end Zetesis.ExtremumCandidates
