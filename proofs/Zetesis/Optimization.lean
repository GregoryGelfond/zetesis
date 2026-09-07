import Zetesis.Ferraris

/-!
# Ranking verified stable models

A separate, numeric objective ranks interpretations after Ferraris stability has
been established. A finite minimum reducer and tie filter are connected to the
global optimum only when their input list covers exactly the stable models.
Incomplete enumeration does not supply that hypothesis.

This source-free layer uses one unbounded integer cost. It does not formalize
lifted tuple joins, tuple coalescing, source objective presence, priority vectors,
finite machine integers, the Rust evaluator, or its streaming incumbent manager.
-/

namespace Zetesis.Optimization

universe u v

variable {α : Type u} {β : Type v}

/-- Choose a minimum-cost member of a finite list. Empty input has no incumbent.
    Ties select the earlier member; a separate filter retains every tied member. -/
def best (cost : β → Int) : List β → Option β
  | [] => none
  | x :: xs => match best cost xs with
    | none => some x
    | some y => if cost x ≤ cost y then some x else some y

theorem best_none_iff (cost : β → Int) (xs : List β) :
    best cost xs = none ↔ xs = [] := by
  cases xs with
  | nil => simp [best]
  | cons x xs =>
    cases h : best cost xs <;> simp [best, h]
    split <;> simp

/-- The reducer returns an actual input member whose cost is no greater than
    any input member's cost. This alone says nothing about unseen candidates. -/
theorem best_spec (cost : β → Int) (xs : List β) (x : β)
    (found : best cost xs = some x) :
    x ∈ xs ∧ ∀ y, y ∈ xs → cost x ≤ cost y := by
  induction xs generalizing x with
  | nil => simp [best] at found
  | cons a xs ih =>
    cases tail : best cost xs with
    | none =>
      have empty : xs = [] := (best_none_iff cost xs).mp tail
      subst xs
      simp [best] at found
      subst x
      simp
    | some b =>
      have hb := ih b tail
      by_cases order : cost a ≤ cost b
      · have same : a = x := by simpa [best, tail, order] using found
        subst x
        refine ⟨by simp, ?_⟩
        intro y hy
        rcases List.mem_cons.mp hy with same | inside
        · subst y
          exact Int.le_refl _
        · exact Int.le_trans order (hb.2 y inside)
      · have same : b = x := by simpa [best, tail, order] using found
        subst x
        refine ⟨by simp [hb.1], ?_⟩
        intro y hy
        rcases List.mem_cons.mp hy with same | inside
        · subst y
          exact Int.le_of_lt (Int.lt_of_not_ge order)
        · exact hb.2 y inside

/-- Optimality adds a ranking requirement to unchanged Ferraris stability. -/
def Optimal (T : Ferraris.Theory α) (cost : Atoms α → Int) (M : Atoms α) : Prop :=
  Ferraris.Stable M T ∧
    ∀ N, Ferraris.Stable N T → cost M ≤ cost N

/-- A low objective value can never make an unstable interpretation optimal. -/
theorem optimal_is_stable (T : Ferraris.Theory α) (cost : Atoms α → Int)
    (M : Atoms α) (optimal : Optimal T cost M) : Ferraris.Stable M T :=
  optimal.1

/-- Every ranked optimum retains the original frozen-reduct minimality test. -/
theorem optimal_is_minimal_reduct (T : Ferraris.Theory α)
    (cost : Atoms α → Int) (M : Atoms α) (optimal : Optimal T cost M) :
    Ferraris.MinimalModel M (Ferraris.ReductTheory M T) :=
  (Ferraris.stable_iff_minimal_reduct M T).mp optimal.1

/-- Soundness plus complete stable-model coverage lifts a finite incumbent to
    a global optimum. A prefix or stopped search does not establish coverage. -/
theorem completed_best_is_optimal (T : Ferraris.Theory α)
    (cost : Atoms α → Int) (xs : List (Atoms α)) (M : Atoms α)
    (sound : ∀ N, N ∈ xs → Ferraris.Stable N T)
    (complete : ∀ N, Ferraris.Stable N T → N ∈ xs)
    (found : best cost xs = some M) : Optimal T cost M := by
  have minimum := best_spec cost xs M found
  exact ⟨sound M minimum.1, fun N hN => minimum.2 N (complete N hN)⟩

/-- Retain every input interpretation tied with the finite minimum. -/
def bestTies (cost : β → Int) (xs : List β) : List β :=
  match best cost xs with
  | none => []
  | some x => xs.filter (fun y => cost y == cost x)

/-- With exact stable-model coverage, the tie filter contains exactly all
    global optima. Duplicate input entries remain duplicates in the output. -/
theorem completed_ties_exact (T : Ferraris.Theory α)
    (cost : Atoms α → Int) (xs : List (Atoms α))
    (sound : ∀ N, N ∈ xs → Ferraris.Stable N T)
    (complete : ∀ N, Ferraris.Stable N T → N ∈ xs) (M : Atoms α) :
    M ∈ bestTies cost xs ↔ Optimal T cost M := by
  cases found : best cost xs with
  | none =>
    have empty := (best_none_iff cost xs).mp found
    subst xs
    constructor
    · simp [bestTies, best]
    · intro optimal
      have impossible := complete M optimal.1
      simp at impossible
  | some N =>
    have optimalN := completed_best_is_optimal T cost xs N sound complete found
    simp only [bestTies, found, List.mem_filter, beq_iff_eq]
    constructor
    · rintro ⟨member, same⟩
      refine ⟨sound M member, ?_⟩
      intro J stableJ
      rw [same]
      exact optimalN.2 J stableJ
    · intro optimalM
      exact ⟨complete M optimalM.1,
        Int.le_antisymm (optimalM.2 N optimalN.1) (optimalN.2 M optimalM.1)⟩

/-- An objective constant across stable models preserves every stable model as
    an optimum, including a present objective whose total always equals zero. -/
theorem constant_on_stable_iff (T : Ferraris.Theory α)
    (cost : Atoms α → Int) (value : Int)
    (constant : ∀ N, Ferraris.Stable N T → cost N = value) (M : Atoms α) :
    Optimal T cost M ↔ Ferraris.Stable M T := by
  constructor
  · exact And.left
  · intro stableM
    refine ⟨stableM, ?_⟩
    intro N stableN
    rw [constant M stableM, constant N stableN]
    exact Int.le_refl _

end Zetesis.Optimization
