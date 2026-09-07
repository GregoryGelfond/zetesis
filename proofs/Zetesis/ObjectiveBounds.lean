import Zetesis.Optimization

/-!
# Candidate-only incumbent bounds

A verified incumbent permits discarding only candidates with strictly greater
cost. Bounds remain non-strict to retain every optimum tie. The candidate filter
does not change original Ferraris stability or form a different program reduct.

This source-free layer uses one unbounded integer cost. Tuple contributions,
priority-vector ordering, the bound-formula compiler, SAT restriction encoding,
restarts, exact blocking and machine limits remain separate obligations.
-/

namespace Zetesis.ObjectiveBounds

universe u v
variable {α : Type u} {β : Type v}
open Ferraris Optimization

/-- Executable finite candidate filtering; equal-cost candidates survive. -/
def prune (cost : β → Int) (ceiling : Int) (values : List β) : List β :=
  values.filter (fun value => decide (cost value ≤ ceiling))

theorem mem_prune (cost : β → Int) (ceiling : Int) (values : List β) (value : β) :
    value ∈ prune cost ceiling values ↔ value ∈ values ∧ cost value ≤ ceiling := by
  simp [prune]

theorem incumbent_survives (cost : β → Int) (values : List β) (incumbent : β)
    (present : incumbent ∈ values) :
    incumbent ∈ prune cost (cost incumbent) values := by
  exact (mem_prune cost (cost incumbent) values incumbent).mpr ⟨present, Int.le_refl _⟩

/-- Tightening a bound cannot restore an earlier excluded candidate. -/
theorem tighten (cost : β → Int) (earlier later : Int) (values : List β)
    (lower : later ≤ earlier) :
    prune cost later (prune cost earlier values) = prune cost later values := by
  simp only [prune, List.filter_filter]
  apply List.filter_congr
  intro value _
  by_cases within : cost value ≤ later
  · have old := Int.le_trans within lower
    simp [within, old]
  · simp [within]

/-- An incumbent is a stable model of the original theory, not merely a cheap
    classical assignment. Therefore every global optimum survives its bound. -/
theorem optimum_survives (theory : Theory α) (cost : Atoms α → Int)
    (incumbent candidate : Atoms α) (verified : Stable incumbent theory)
    (optimal : Optimal theory cost candidate) : cost candidate ≤ cost incumbent :=
  optimal.2 incumbent verified

theorem excluded_is_worse (cost : β → Int) (incumbent candidate : β)
    (excluded : ¬ cost candidate ≤ cost incumbent) : cost incumbent < cost candidate := by
  omega

/-- A minimum over the completed bounded region is a global minimum because
    every omitted stable candidate is worse than the verified incumbent. -/
theorem bounded_best_is_global (theory : Theory α) (cost : Atoms α → Int)
    (incumbent : Atoms α) (verified : Stable incumbent theory)
    (values : List (Atoms α)) (candidate : Atoms α)
    (sound : ∀ value ∈ values, Stable value theory)
    (complete : ∀ value, Stable value theory → cost value ≤ cost incumbent → value ∈ values)
    (found : best cost values = some candidate) : Optimal theory cost candidate := by
  have minimum := best_spec cost values candidate found
  have incumbent_present := complete incumbent verified (Int.le_refl _)
  have no_worse := minimum.2 incumbent incumbent_present
  refine ⟨sound candidate minimum.1, ?_⟩
  intro value stable
  by_cases inside : cost value ≤ cost incumbent
  · exact minimum.2 value (complete value stable inside)
  · omega

/-- Complete bounded coverage plus stored verified ties is sufficient; it is
    unnecessary to enumerate every worse stable model. Partial coverage is not. -/
theorem bounded_ties_exact (theory : Theory α) (cost : Atoms α → Int)
    (incumbent : Atoms α) (verified : Stable incumbent theory)
    (values : List (Atoms α))
    (sound : ∀ value ∈ values, Stable value theory)
    (complete : ∀ value, Stable value theory → cost value ≤ cost incumbent → value ∈ values)
    (candidate : Atoms α) :
    candidate ∈ bestTies cost values ↔ Optimal theory cost candidate := by
  cases found : best cost values with
  | none =>
    have empty := (best_none_iff cost values).mp found
    have present := complete incumbent verified (Int.le_refl _)
    rw [empty] at present
    exact False.elim (List.not_mem_nil present)
  | some winner =>
    have optimal_winner := bounded_best_is_global theory cost incumbent verified
      values winner sound complete found
    simp only [bestTies, found, List.mem_filter, beq_iff_eq]
    constructor
    · intro ⟨present, tie⟩
      exact ⟨sound candidate present, fun value stable => tie ▸ optimal_winner.2 value stable⟩
    · intro optimal
      exact ⟨complete candidate optimal.1 (optimum_survives theory cost incumbent candidate verified optimal),
        Int.le_antisymm (optimal.2 winner optimal_winner.1)
          (optimal_winner.2 candidate optimal.1)⟩

/-- All bounded optima still satisfy original frozen-reduct minimality. -/
theorem bounded_optimum_keeps_original_reduct (theory : Theory α)
    (cost : Atoms α → Int) (candidate : Atoms α)
    (optimal : Optimal theory cost candidate) :
    MinimalModel candidate (ReductTheory candidate theory) :=
  optimal_is_minimal_reduct theory cost candidate optimal

end Zetesis.ObjectiveBounds
