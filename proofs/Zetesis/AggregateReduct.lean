import Zetesis.ChoiceIntervals

/-!
# Native evaluation of finite aggregate reducts

Ferraris, *Answer Sets for Propositional Theories*, Proposition 7, characterizes
aggregate truth directly, without expanding the canonical subset implications.
This module formalizes that semantic boundary for an arbitrary Boolean guard on
a complete finite tuple mask. Element eligibility remains an arbitrary formula.
Original and frozen evaluation are separate; neither assumes J is contained in M.

The supplied mask list must cover every subset of the finite element positions.
These positions represent already distinct whole tuples, not distinct weights.
Source tuple coalescing, arithmetic and term ordering,
optimized compiler correspondence, resource accounting and Rust/WGSL refinement
remain separate obligations. The definition is a reference semantics, not a
recommendation to enumerate subsets at runtime.

Source: https://www.cs.utexas.edu/~ai-lab/pubs/proptheories.pdf, Proposition 7.
-/

namespace Zetesis.AggregateReduct

open Ferraris ChoiceIntervals
open RuleFactorization (any)

universe u
variable {α : Type u} {n : Nat}

/-- One Boolean per distinct finite tuple position. -/
abbrev Mask (n : Nat) := Fin n → Bool

/-- The mask enumeration contains every tuple subset. Duplicates do not affect
    the conjunction's meaning, though an executable builder must charge them. -/
def Complete (masks : List (Mask n)) : Prop := ∀ mask, mask ∈ masks

/-- Enumerate all masks by extending each shorter mask with both head bits.
    This is an executable finite reference, with exponentially many masks. -/
def masks : (size : Nat) → List (Mask size)
  | 0 => [fun index => Fin.elim0 index]
  | size + 1 => (masks size).flatMap
      (fun tail => [Fin.cases false tail, Fin.cases true tail])

/-- Head extension covers every Boolean mask, including the empty carrier. -/
theorem masks_complete (size : Nat) : Complete (masks size) := by
  induction size with
  | zero =>
    intro mask
    have empty : mask = (fun index => Fin.elim0 index) := by
      funext index
      exact Fin.elim0 index
    simp [masks, empty]
  | succ size ih =>
    intro mask
    let tail : Mask size := fun index => mask index.succ
    have reconstruct : Fin.cases (mask 0) tail = mask := by
      funext index
      exact Fin.cases rfl (fun _ => rfl) index
    apply List.mem_flatMap.mpr
    refine ⟨tail, ih tail, ?_⟩
    rw [← reconstruct]
    cases head : mask 0 <;> simp

/-- Retain selected eligibility formulas without changing their meaning. -/
def selected (conditions : Fin n → Formula α) (mask : Mask n) : List (Formula α) :=
  ((List.finRange n).filter mask).map conditions

/-- Universal selected-formula truth is indexed by the same complete tuple mask. -/
theorem selected_forall (predicate : Formula α → Prop)
    (conditions : Fin n → Formula α) (mask : Mask n) :
    (∀ F ∈ selected conditions mask, predicate F) ↔
      ∀ index, mask index = true → predicate (conditions index) := by
  simp only [selected, List.forall_mem_map, List.forall_mem_filter,
    List.mem_finRange, true_implies]

/-- An existential selected-formula witness retains its tuple position. -/
theorem selected_exists (predicate : Formula α → Prop)
    (conditions : Fin n → Formula α) (mask : Mask n) :
    (∃ F ∈ selected conditions mask, predicate F) ↔
      ∃ index, mask index = true ∧ predicate (conditions index) := by
  constructor
  · rintro ⟨F, member, holds⟩
    obtain ⟨index, inside, rfl⟩ := List.mem_map.mp member
    exact ⟨index, (List.mem_filter.mp inside).2, holds⟩
  · rintro ⟨index, inside, holds⟩
    exact ⟨_, List.mem_map.mpr
      ⟨index, List.mem_filter.mpr ⟨List.mem_finRange index, inside⟩, rfl⟩, holds⟩

/-- The canonical implication excludes exactly one failing tuple subset. -/
def clause (conditions : Fin n → Formula α) (mask : Mask n) : Formula α :=
  .imp (all (selected conditions mask))
    (any (selected conditions (fun index => !(mask index))))

/-- Canonical finite aggregate formula: exclude every subset rejected by the
    fixed guard. Source values and the guard are fixed within this instance. -/
def aggregate (conditions : Fin n → Formula α) (guard : Mask n → Bool)
    (masks : List (Mask n)) : Formula α :=
  all ((masks.filter (fun mask => !(guard mask))).map (clause conditions))

open Classical in
/-- Original eligibility truth, retaining one bit per complete tuple. -/
noncomputable def active (M : Atoms α) (conditions : Fin n → Formula α) : Mask n :=
  fun index => decide (Satisfies M (conditions index))

/-- The finite conjunction commutes with satisfaction of the frozen reduct. -/
theorem all_frozen (M J : Atoms α) (formulas : List (Formula α)) :
    Satisfies J (Reduct M (all formulas)) ↔
      ∀ F ∈ formulas, Satisfies J (Reduct M F) := by
  classical
  induction formulas with
  | nil => simp [all, top, Reduct, Satisfies]
  | cons first rest ih => simp [all, RuleFactorization.reduct_conj, ih]

/-- An implication from selected truths to an unselected truth excludes exactly
    the mask equal to the complete truth vector, including an empty vector. -/
theorem clause_mask (mask truth : Mask n) :
    ((∀ index, mask index = true → truth index = true) →
      ∃ index, mask index = false ∧ truth index = true) ↔ mask ≠ truth := by
  classical
  constructor
  · intro holds same
    subst truth
    obtain ⟨index, outside, inside⟩ := holds (fun _ selected => selected)
    simp [outside] at inside
  · intro different antecedent
    apply Classical.byContradiction
    intro noWitness
    apply different
    funext index
    cases selected : mask index with
    | false =>
      cases value : truth index with
      | false => rfl
      | true => exact False.elim (noWitness ⟨index, selected, value⟩)
    | true => exact (antecedent index selected).symm

/-- Original clause truth depends only on its finite eligibility mask. -/
theorem clause_original (M : Atoms α) (conditions : Fin n → Formula α)
    (mask : Mask n) :
    Satisfies M (clause conditions mask) ↔ mask ≠ active M conditions := by
  classical
  simp only [clause, Satisfies, all_classical, RuleFactorization.satisfies_any,
    selected_forall, selected_exists, Bool.not_eq_true']
  simpa only [active, decide_eq_true_eq] using clause_mask mask (active M conditions)

/-- A frozen clause retains both its original gate and the recursively frozen
    eligibility mask. Original conditions are not re-evaluated directly in J. -/
theorem clause_frozen (M J : Atoms α) (conditions : Fin n → Formula α)
    (mask : Mask n) :
    Satisfies J (Reduct M (clause conditions mask)) ↔
      mask ≠ active M conditions ∧
        mask ≠ active J (fun index => Reduct M (conditions index)) := by
  classical
  rw [clause, RuleFactorization.reduct_imp]
  have original := clause_original M conditions mask
  have frozen := clause_mask mask (active J (fun index => Reduct M (conditions index)))
  simpa only [clause, all_frozen, RuleFactorization.reduct_any, selected_forall,
    selected_exists, Bool.not_eq_true', active, decide_eq_true_eq] using
    and_congr original frozen

/-- Excluding every failing mask is equivalent to accepting the actual mask.
    Completeness of the supplied enumeration is essential in this direction. -/
theorem excluded_masks (guard : Mask n → Bool) (masks : List (Mask n))
    (complete : Complete masks) (truth : Mask n) :
    (∀ mask ∈ masks, guard mask = false → mask ≠ truth) ↔ guard truth = true := by
  constructor
  · intro excludes
    cases value : guard truth with
    | false => exact False.elim (excludes truth (complete truth) value rfl)
    | true => rfl
  · intro accepted mask _ rejected same
    subst mask
    simp [rejected] at accepted

/-- Proposition 7(a): canonical aggregate satisfaction is direct evaluation of
    the guard on original tuple eligibility. No numeric monotonicity is assumed. -/
theorem original (M : Atoms α) (conditions : Fin n → Formula α)
    (guard : Mask n → Bool) (masks : List (Mask n)) (complete : Complete masks) :
    Satisfies M (aggregate conditions guard masks) ↔ guard (active M conditions) = true := by
  have clauses : Satisfies M (aggregate conditions guard masks) ↔
      ∀ mask ∈ masks, guard mask = false → mask ≠ active M conditions := by
    simp only [aggregate, all_classical, List.forall_mem_map,
      List.forall_mem_filter, Bool.not_eq_true', clause_original]
  exact clauses.trans (excluded_masks guard masks complete (active M conditions))

/-- Proposition 7(b): a native aggregate reduct evaluates the original gate and
    the same guard over recursively frozen eligibility. This holds for arbitrary
    M/J and arbitrary fixed guards, including signed/nonmonotone comparisons. -/
theorem frozen (M J : Atoms α) (conditions : Fin n → Formula α)
    (guard : Mask n → Bool) (masks : List (Mask n)) (complete : Complete masks) :
    Satisfies J (Reduct M (aggregate conditions guard masks)) ↔
      guard (active M conditions) = true ∧
        guard (active J (fun index => Reduct M (conditions index))) = true := by
  have clauses : Satisfies J (Reduct M (aggregate conditions guard masks)) ↔
      ∀ mask ∈ masks, guard mask = false →
        mask ≠ active M conditions ∧
          mask ≠ active J (fun index => Reduct M (conditions index)) := by
    simp only [aggregate, all_frozen, List.forall_mem_map,
      List.forall_mem_filter, Bool.not_eq_true', clause_frozen]
  constructor
  · intro satisfies
    have each := clauses.mp satisfies
    exact ⟨(excluded_masks guard masks complete _).mp
      (fun mask inside rejected => (each mask inside rejected).1),
      (excluded_masks guard masks complete _).mp
      (fun mask inside rejected => (each mask inside rejected).2)⟩
  · intro ⟨originalTrue, frozenTrue⟩
    apply clauses.mpr
    intro mask inside rejected
    exact ⟨(excluded_masks guard masks complete _).mpr originalTrue mask inside rejected,
      (excluded_masks guard masks complete _).mpr frozenTrue mask inside rejected⟩

/-- Direct reduct evaluation on the reference mask enumeration needs no
    unproved coverage premise. Its cost is a mathematical, not runtime, claim. -/
theorem direct_reduct (M J : Atoms α) (conditions : Fin n → Formula α)
    (guard : Mask n → Bool) :
    Satisfies J (Reduct M (aggregate conditions guard (masks n))) ↔
      guard (active M conditions) = true ∧
        guard (active J (fun index => Reduct M (conditions index))) = true :=
  frozen M J conditions guard (masks n) (masks_complete n)

/-- Any executable guard agreeing on every finite mask may replace the guard in
    direct reduct evaluation. Concrete arithmetic agreement remains a premise. -/
theorem evaluator_refinement (M J : Atoms α) (conditions : Fin n → Formula α)
    (guard evaluate : Mask n → Bool) (agreement : ∀ mask, evaluate mask = guard mask) :
    Satisfies J (Reduct M (aggregate conditions guard (masks n))) ↔
      evaluate (active M conditions) = true ∧
        evaluate (active J (fun index => Reduct M (conditions index))) = true := by
  rw [direct_reduct, agreement, agreement]

end Zetesis.AggregateReduct
