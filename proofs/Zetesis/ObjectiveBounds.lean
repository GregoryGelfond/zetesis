import Zetesis.Optimization

/-!
# Candidate-only incumbent bounds

A verified incumbent permits discarding only candidates with strictly greater
cost. Bounds remain non-strict to retain every optimum tie. The candidate filter
does not change original Ferraris stability or form a different program reduct.

The cost-filter laws use one unbounded integer cost. The replacement laws use
arbitrary predicates with an explicit strengthening premise. The relative laws
require that implication only under an unchanged permanent predicate, such as
satisfaction of the original theory. They also apply to multiple priorities once
their bound implication is established. Tuple
contributions, priority-vector ordering, the bound-formula compiler, retained
worker generations, restriction encoding, exact blocking and machine limits
remain separate obligations.
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

/-- A stronger candidate bound makes its predecessor redundant, while all
independent restrictions remain in force. The strengthening premise is logical
implication, not a relation between construction times or generation numbers.
Retain the independent restriction and the new bound in each direction; only
the reverse direction needs implication to recover the old bound. -/
theorem replacement_preserves_candidates (permanent earlier later : β → Prop)
    (stronger : ∀ value, later value → earlier value) (value : β) :
    (permanent value ∧ earlier value ∧ later value) ↔
      (permanent value ∧ later value) := by
  constructor
  · intro retained
    exact ⟨retained.1, retained.2.2⟩
  · intro retained
    exact ⟨retained.1, stronger value retained.2, retained.2⟩

/-- Every consequence of an earlier bound within a restricted region remains a
consequence after strengthening the bound. Thus dropping the old bound's stored
formula need not undo region decisions already justified by it. This statement
does not justify reusing mutable propagation knowledge from a different formula;
the implementation must establish its new knowledge separately. -/
theorem consequence_survives_tightening
    (region permanent earlier later consequence : β → Prop)
    (stronger : ∀ value, later value → earlier value)
    (known : ∀ value, region value → permanent value → earlier value → consequence value) :
    ∀ value, region value → permanent value → later value → consequence value := by
  intro value inside restricted bounded
  have earlierBound : earlier value := stronger value bounded
  exact known value inside restricted earlierBound

/-- An earlier bound is redundant when the new bound implies it under an
unchanged permanent predicate. That predicate can include satisfaction of the
original theory: interpretations outside it need not satisfy the implication.
Retain the permanent predicate and new bound in each direction, then recover the
earlier bound from those two premises in the reverse direction. -/
theorem replacement_preserves_candidates_relative (permanent earlier later : β → Prop)
    (stronger : ∀ value, permanent value → later value → earlier value) (value : β) :
    (permanent value ∧ earlier value ∧ later value) ↔
      (permanent value ∧ later value) := by
  constructor
  · intro retained
    exact ⟨retained.1, retained.2.2⟩
  · intro retained
    have earlierBound : earlier value := stronger value retained.1 retained.2
    exact ⟨retained.1, earlierBound, retained.2⟩

/-- A region consequence justified by an earlier bound remains valid when the
replacement implies it under the same permanent predicate. First recover the
earlier bound using that predicate, then apply the established consequence.
This preserves region decisions, not mutable knowledge about a different DAG;
the new bound still requires independently initialized propagation knowledge. -/
theorem consequence_survives_tightening_relative
    (region permanent earlier later consequence : β → Prop)
    (stronger : ∀ value, permanent value → later value → earlier value)
    (known : ∀ value, region value → permanent value → earlier value → consequence value) :
    ∀ value, region value → permanent value → later value → consequence value := by
  intro value inside restricted bounded
  have earlierBound : earlier value := stronger value restricted bounded
  exact known value inside restricted earlierBound

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
