import Zetesis.ObjectiveDirections
import Zetesis.ObjectiveBounds

/-!
# Nonnegative candidate bounds for signed objectives

After direction normalization and complete-key OR coalescing, each signed
contribution becomes a nonnegative weight on its condition or its Boolean
complement, together with a candidate-independent negative offset. The grouped
list is retained: this transformation never deduplicates transformed entries.

These are classical candidate-query identities. Bounds filter candidates while
the original Ferraris theory and its reduct remain unchanged. They do not
justify rewriting signed aggregates in that theory. Integers are unbounded;
machine representability, overflow/fallback, bound compilation, source grounding,
query restriction, resource completion and Rust refinement remain unproved.
-/

namespace Zetesis.SignedObjectiveBounds

open AggregateAssignment (weightedSum)
open ObjectiveDirections

universe u v w
variable {σ : Type u} {χ : Type v} {α : Type w}

/-- Candidate-independent constant contributed by a negative weight. -/
def negativeOffset (weight : Int) : Int :=
  if weight < 0 then weight else 0

/-- Exact integer magnitude; finite-width negation is a separate obligation. -/
def magnitude (weight : Int) : Int :=
  if weight < 0 then -weight else weight

def normalizedActive (weight : Int) (active : Bool) : Bool :=
  if weight < 0 then !active else active

def nonnegativeTerm (weight : Int) (active : Bool) : Int :=
  if normalizedActive weight active then magnitude weight else 0

theorem magnitude_nonnegative (weight : Int) : 0 ≤ magnitude weight := by
  unfold magnitude
  split <;> omega

/-- Complementing a negative term is exact only with its constant offset. -/
theorem contribution_exact (weight : Int) (active : Bool) :
    (if active then weight else 0) = negativeOffset weight + nonnegativeTerm weight active := by
  by_cases negative : weight < 0 <;> cases active <;>
    simp [negativeOffset, nonnegativeTerm, normalizedActive, magnitude, negative] <;> omega

def offset (priority : Int) (groups : List (Group σ χ)) : Int :=
  weightedSum (fun group => negativeOffset (keyAt priority group.key)) groups

def nonnegativeCost (priority : Int) (truth : χ → Bool) (groups : List (Group σ χ)) : Int :=
  weightedSum (fun group => nonnegativeTerm (keyAt priority group.key) (groupActive truth group)) groups

theorem nonnegative_cost_nonnegative (priority : Int) (truth : χ → Bool)
    (groups : List (Group σ χ)) : 0 ≤ nonnegativeCost priority truth groups := by
  induction groups with
  | nil => simp [nonnegativeCost, weightedSum]
  | cons group groups ih =>
    have weight := magnitude_nonnegative (keyAt priority group.key)
    simp only [nonnegativeCost, weightedSum, nonnegativeTerm] at *
    split <;> omega

/-- Every original group remains a separate term, even when transformed
    magnitudes and conditions coincide. No uniqueness premise is needed here. -/
theorem compiled_cost_exact (priority : Int) (truth : χ → Bool) (groups : List (Group σ χ)) :
    compiledCost priority truth groups = offset priority groups + nonnegativeCost priority truth groups := by
  induction groups with
  | nil => rfl
  | cons group groups ih =>
    have term := contribution_exact (keyAt priority group.key) (groupActive truth group)
    simp only [compiledCost, offset, nonnegativeCost, weightedSum] at *
    omega

/-- The actual mixed-objective reference is connected only after complete-key
    normalization and global OR coalescing by the existing compiler contract. -/
theorem mixed_cost_exact [DecidableEq σ] (priority : Int) (truth : χ → Bool)
    (entries : List (Entry σ χ)) :
    mixedCost priority truth entries = offset priority (normalize entries) +
      nonnegativeCost priority truth (normalize entries) := by
  rw [← normalization_preserves_cost]
  exact compiled_cost_exact priority truth (normalize entries)

theorem shifted_lt (priority ceiling : Int) (truth : χ → Bool) (groups : List (Group σ χ)) :
    compiledCost priority truth groups < ceiling ↔
      nonnegativeCost priority truth groups < ceiling - offset priority groups := by
  have identity := compiled_cost_exact priority truth groups
  omega

theorem shifted_eq (priority ceiling : Int) (truth : χ → Bool) (groups : List (Group σ χ)) :
    compiledCost priority truth groups = ceiling ↔
      nonnegativeCost priority truth groups = ceiling - offset priority groups := by
  have identity := compiled_cost_exact priority truth groups
  omega

theorem shifted_le (priority ceiling : Int) (truth : χ → Bool) (groups : List (Group σ χ)) :
    compiledCost priority truth groups ≤ ceiling ↔
      nonnegativeCost priority truth groups ≤ ceiling - offset priority groups := by
  have identity := compiled_cost_exact priority truth groups
  omega

def nonnegativeVector (priorities : List Int) (truth : χ → Bool) (groups : List (Group σ χ)) : List Int :=
  priorities.map (fun priority => nonnegativeCost priority truth groups)

/-- Each priority has its own offset. Shifting an arbitrary fixed-priority
    bound preserves the complete lexicographic comparison, including equality. -/
theorem shifted_vector_comparison (priorities : List Int) (truth : χ → Bool)
    (groups : List (Group σ χ)) (ceiling : Int → Int) :
    compareVectors (compiledVector priorities truth groups) (priorities.map ceiling) =
      compareVectors (nonnegativeVector priorities truth groups)
        (priorities.map (fun priority => ceiling priority - offset priority groups)) := by
  induction priorities with
  | nil => rfl
  | cons priority priorities ih =>
    have identity := compiled_cost_exact priority truth groups
    have lower : compiledCost priority truth groups < ceiling priority ↔
        nonnegativeCost priority truth groups < ceiling priority - offset priority groups := by omega
    have upper : ceiling priority < compiledCost priority truth groups ↔
        ceiling priority - offset priority groups < nonnegativeCost priority truth groups := by omega
    simp only [compiledVector, nonnegativeVector, List.map_cons, compareVectors] at *
    simp only [lower, upper, ih]

theorem normalization_preserves_comparison (priorities : List Int) (left right : χ → Bool)
    (groups : List (Group σ χ)) :
    compareVectors (nonnegativeVector priorities left groups) (nonnegativeVector priorities right groups) =
      compareVectors (compiledVector priorities left groups) (compiledVector priorities right groups) := by
  have shifted := shifted_vector_comparison priorities left groups
    (fun priority => compiledCost priority right groups)
  have values :
      priorities.map (fun priority => compiledCost priority right groups - offset priority groups) =
        nonnegativeVector priorities right groups := by
    apply List.map_congr_left
    intro priority _
    have identity := compiled_cost_exact priority right groups
    omega
  rw [values] at shifted
  exact shifted.symm

theorem normalization_preserves_vector_equality (priorities : List Int) (left right : χ → Bool)
    (groups : List (Group σ χ)) :
    nonnegativeVector priorities left groups = nonnegativeVector priorities right groups ↔
      compiledVector priorities left groups = compiledVector priorities right groups := by
  induction priorities with
  | nil => simp [nonnegativeVector, compiledVector]
  | cons priority priorities ih =>
    have first := compiled_cost_exact priority left groups
    have second := compiled_cost_exact priority right groups
    have same : nonnegativeCost priority left groups = nonnegativeCost priority right groups ↔
        compiledCost priority left groups = compiledCost priority right groups := by omega
    simp only [nonnegativeVector, compiledVector, List.map_cons, List.cons.injEq] at *
    rw [same, ih]

/-- A candidate bound remains a predicate alongside original stability. Its
    normalization preserves that region; it need not retain worse stable models. -/
theorem candidate_filter_exact (theory : Ferraris.Theory α) (priority ceiling : Int)
    (truth : Atoms α → χ → Bool) (groups : List (Group σ χ)) (candidate : Atoms α) :
    (Ferraris.Stable candidate theory ∧
      nonnegativeCost priority (truth candidate) groups ≤ ceiling - offset priority groups) ↔
    (Ferraris.Stable candidate theory ∧ compiledCost priority (truth candidate) groups ≤ ceiling) := by
  rw [← shifted_le]

/-- Ranking still quantifies only over the original theory's stable models.
    Removing fixed per-priority offsets preserves every original optimum. -/
theorem normalization_preserves_optimal_models (theory : Ferraris.Theory α)
    (priorities : List Int) (truth : Atoms α → χ → Bool) (groups : List (Group σ χ))
    (candidate : Atoms α) :
    ObjectiveDirections.Optimal theory (fun value => nonnegativeVector priorities (truth value) groups) candidate ↔
      ObjectiveDirections.Optimal theory (fun value => compiledVector priorities (truth value) groups) candidate := by
  simp only [ObjectiveDirections.Optimal, normalization_preserves_comparison]

/-- All candidates tied with an incumbent retain their order and multiplicity. -/
theorem normalization_preserves_tie_list (priorities : List Int)
    (truth : Atoms α → χ → Bool) (groups : List (Group σ χ))
    (incumbent : Atoms α) (candidates : List (Atoms α)) :
    ties (fun value => nonnegativeVector priorities (truth value) groups) incumbent candidates =
      ties (fun value => compiledVector priorities (truth value) groups) incumbent candidates := by
  unfold ties
  apply List.filter_congr
  intro candidate _
  apply Bool.eq_iff_iff.mpr
  simp only [beq_iff_eq]
  exact normalization_preserves_vector_equality priorities (truth candidate) (truth incumbent) groups

/-- Every optimum survives the normalized non-strict lexicographic bound of a
    verified incumbent. Equal-cost stable models cannot be discarded. -/
theorem optimal_survives_incumbent (theory : Ferraris.Theory α)
    (priorities : List Int) (truth : Atoms α → χ → Bool) (groups : List (Group σ χ))
    (incumbent candidate : Atoms α) (verified : Ferraris.Stable incumbent theory)
    (optimal : ObjectiveDirections.Optimal theory
      (fun value => compiledVector priorities (truth value) groups) candidate) :
    compareVectors (nonnegativeVector priorities (truth candidate) groups)
      (nonnegativeVector priorities (truth incumbent) groups) ≠ .gt := by
  rw [normalization_preserves_comparison]
  exact optimal.2 incumbent verified

/-- Reuse the existing completed-region theorem. Coverage is explicitly for
    the normalized scalar bound; no search termination or compiler is assumed. -/
theorem completed_scalar_ties_exact (theory : Ferraris.Theory α) (priority : Int)
    (truth : Atoms α → χ → Bool) (groups : List (Group σ χ))
    (incumbent : Atoms α) (verified : Ferraris.Stable incumbent theory)
    (values : List (Atoms α))
    (sound : ∀ value ∈ values, Ferraris.Stable value theory)
    (complete : ∀ value, Ferraris.Stable value theory →
      nonnegativeCost priority (truth value) groups ≤
        compiledCost priority (truth incumbent) groups - offset priority groups → value ∈ values)
    (candidate : Atoms α) :
    candidate ∈ Optimization.bestTies (fun value => compiledCost priority (truth value) groups) values ↔
      Optimization.Optimal theory (fun value => compiledCost priority (truth value) groups) candidate := by
  apply ObjectiveBounds.bounded_ties_exact theory _ incumbent verified values sound
  intro value stable within
  exact complete value stable ((shifted_le priority _ (truth value) groups).mp within)

/-- A duplicate source key is charged once before weight complementation;
    counting raw occurrences first would change the objective. -/
theorem coalescing_order_example :
    let entry : Entry Int Bool := ⟨.minimize, -2, 0, [7], true⟩
    mixedCost 0 id [entry, entry] = -2 ∧
      offset 0 (normalize [entry, entry]) + nonnegativeCost 0 id (normalize [entry, entry]) = -2 ∧
      offset 0 [⟨normalizedKey entry, [true]⟩, ⟨normalizedKey entry, [true]⟩] +
        nonnegativeCost 0 id [⟨normalizedKey entry, [true]⟩, ⟨normalizedKey entry, [true]⟩] = -4 := by
  decide

end Zetesis.SignedObjectiveBounds
