import Zetesis.GroupedExclusionCoverage
import Zetesis.RuleFactorization

/-!
# Finite witnesses, distinct binding lanes, and guarded at-most-one

Each fiber retains a finite distinct list of binding identities. Each binding
owns a finite list of condition witnesses; existential disjunction is taken
within that list, never across distinct identities or shared fiber keys.
The source family contains the full witness product for every covered binding
pair. Establishing that product from a source grammar is a separate obligation.
The native target has one unit-weight occurrence per binding even when two
conditions are the same formula. Its common guard remains outside the count.
-/

namespace Zetesis.GroupedWitnessExclusion

open Ferraris PairwiseExclusions GroupedExclusionCoverage ThresholdConnectives

universe u v w
variable {α : Type u} {β : Type v} {κ : Type w}

/-- The complete source witness product for one distinct binding pair. -/
def witnessBodies (guard : Formula α) (left right : List (Formula α)) :
    List (Formula α) :=
  left.flatMap (fun a => right.map (fun b => .conj guard (.conj a b)))

/-- A lane's Any reduction retains all its complete witness alternatives. -/
def lane (witnesses : β → List (Formula α)) (index : β) : Formula α :=
  RuleFactorization.any (witnesses index)

/-- This is also an exact frozen positive-condition reading, not only original
truth. The existing law makes no subset premise on the tested interpretation. -/
theorem lane_frozen (M J : Atoms α) (witnesses : β → List (Formula α)) (index : β) :
    Satisfies J (Reduct M (lane witnesses index)) ↔
      ∃ condition ∈ witnesses index, Satisfies J (Reduct M condition) := by
  exact RuleFactorization.reduct_any M J (witnesses index)

theorem witness_pair_requirement (M : Atoms α) (guard : Formula α)
    (left right : List (Formula α)) :
    Models M ((witnessBodies guard left right).map Ferraris.Neg) ↔
      ∀ a ∈ left, ∀ b ∈ right,
        ¬ (Satisfies M guard ∧ Satisfies M a ∧ Satisfies M b) := by
  constructor
  · intro source a ha b hb held
    exact source _ (List.mem_map.mpr ⟨_,
      List.mem_flatMap.mpr ⟨a, ha, List.mem_map.mpr ⟨b, hb, rfl⟩⟩, rfl⟩) held
  · intro source constraint listed
    obtain ⟨body, bodyListed, rfl⟩ := List.mem_map.mp listed
    obtain ⟨a, ha, pairListed⟩ := List.mem_flatMap.mp bodyListed
    obtain ⟨b, hb, rfl⟩ := List.mem_map.mp pairListed
    exact source a ha b hb

/-- Any coalesces only witnesses belonging to the same binding identity. -/
theorem witness_pair_any (M : Atoms α) (guard : Formula α)
    (left right : List (Formula α)) :
    Models M ((witnessBodies guard left right).map Ferraris.Neg) ↔
      Satisfies M (Ferraris.Neg (.conj guard
        (.conj (RuleFactorization.any left) (RuleFactorization.any right)))) := by
  rw [witness_pair_requirement]
  constructor
  · intro source held
    obtain ⟨a, ha, truthA⟩ := (RuleFactorization.satisfies_any M left).mp held.2.1
    obtain ⟨b, hb, truthB⟩ := (RuleFactorization.satisfies_any M right).mp held.2.2
    exact source a ha b hb ⟨held.1, truthA, truthB⟩
  · intro grouped a ha b hb held
    exact grouped ⟨held.1,
      (RuleFactorization.satisfies_any M left).mpr ⟨a, ha, held.2.1⟩,
      (RuleFactorization.satisfies_any M right).mpr ⟨b, hb, held.2.2⟩⟩

/-- Full witness products, grouped by covered source binding pairs. -/
def witnessedBodies (guard : Formula α) (witnesses : β → List (Formula α))
    (edges : List (β × β)) : List (Formula α) :=
  edges.flatMap (fun edge => witnessBodies guard (witnesses edge.1) (witnesses edge.2))

theorem witnessed_family_any (M : Atoms α) (guard : Formula α)
    (witnesses : β → List (Formula α)) (edges : List (β × β)) :
    Models M ((witnessedBodies guard witnesses edges).map Ferraris.Neg) ↔
      Models M ((guardedBodies guard (lane witnesses) edges).map Ferraris.Neg) := by
  unfold witnessedBodies
  rw [constraints_by_fiber]
  simp only [Models, guardedBodies, List.forall_mem_map]
  constructor
  · intro source edge present
    exact (witness_pair_any M guard (witnesses edge.1) (witnesses edge.2)).mp
      (by simpa only [Models, List.forall_mem_map] using source edge present)
  · intro grouped edge present
    simpa only [Models, List.forall_mem_map] using
      (witness_pair_any M guard (witnesses edge.1) (witnesses edge.2)).mpr
        (grouped edge present)

/-- A shared guard may be retained once outside the native threshold. -/
def guardedCount (guard : Formula α) (condition : β → Formula α)
    (indices : List β) : Formula α :=
  .conj guard (ThresholdReduct.formula condition (unitRow indices) 2)

theorem guarded_pair_requirement (M : Atoms α) (guard : Formula α)
    (condition : β → Formula α) (edges : List (β × β)) :
    Models M ((guardedBodies guard condition edges).map Ferraris.Neg) ↔
      (Satisfies M guard → Models M ((bodies condition edges).map Ferraris.Neg)) := by
  simp only [Models, guardedBodies, bodies, List.forall_mem_map]
  constructor
  · intro source guardTrue edge present pairTrue
    exact source edge present ⟨guardTrue, pairTrue⟩
  · intro source edge present held
    exact source held.1 edge present held.2

theorem guarded_native_original (M : Atoms α) (guard : Formula α)
    (condition : β → Formula α) (indices : List β) (edges : List (β × β))
    (distinct : indices.Nodup) (covered : Coverage indices edges) :
    Models M ((guardedBodies guard condition edges).map Ferraris.Neg) ↔
      Satisfies M (Ferraris.Neg (guardedCount guard condition indices)) := by
  rw [guarded_pair_requirement]
  constructor
  · intro source held
    exact (PairwiseExclusions.original M condition indices edges distinct covered).mp
      (source held.1) held.2
  · intro grouped guardTrue
    apply (PairwiseExclusions.original M condition indices edges distinct covered).mpr
    intro countTrue
    exact grouped ⟨guardTrue, countTrue⟩

/-- Map exact bindings, reduce witnesses with Any, and count binding occurrences.
No injectivity of formulas is assumed. -/
theorem witnessed_group_original (M : Atoms α) (guard : Formula α)
    (witnesses : β → List (Formula α)) (indices : List β) (edges : List (β × β))
    (distinct : indices.Nodup) (covered : Coverage indices edges) :
    Models M ((witnessedBodies guard witnesses edges).map Ferraris.Neg) ↔
      Satisfies M (Ferraris.Neg (guardedCount guard (lane witnesses) indices)) := by
  exact (witnessed_family_any M guard witnesses edges).trans
    (guarded_native_original M guard (lane witnesses) indices edges distinct covered)

/-- The complete negative-family replacement preserves every frozen M/J reading. -/
theorem witnessed_group_frozen (M J : Atoms α) (guard : Formula α)
    (witnesses : β → List (Formula α)) (indices : List β) (edges : List (β × β))
    (distinct : indices.Nodup) (covered : Coverage indices edges) :
    Models J (ReductTheory M ((witnessedBodies guard witnesses edges).map Ferraris.Neg)) ↔
      Satisfies J (Reduct M (Ferraris.Neg
        (guardedCount guard (lane witnesses) indices))) := by
  rw [constraints_frozen_iff, constraint_frozen_iff]
  exact witnessed_group_original M guard witnesses indices edges distinct covered

/-- The unchanged context may itself derive common guards and lane conditions. -/
theorem witnessed_group_stable (M : Atoms α) (context : Theory α)
    (guard : Formula α) (witnesses : β → List (Formula α))
    (indices : List β) (edges : List (β × β))
    (distinct : indices.Nodup) (covered : Coverage indices edges) :
    Stable M (context ++ (witnessedBodies guard witnesses edges).map Ferraris.Neg) ↔
      Stable M (context ++ [Ferraris.Neg (guardedCount guard (lane witnesses) indices)]) := by
  have nativeLaw := ConstrainedPositive.stable_append_constraints context
    [guardedCount guard (lane witnesses) indices] M
  simp only [List.map_cons, List.map_nil] at nativeLaw
  rw [ConstrainedPositive.stable_append_constraints, nativeLaw]
  have originalLaw := witnessed_group_original M guard witnesses indices edges distinct covered
  simp only [Models, List.mem_singleton, forall_eq] at originalLaw ⊢
  exact and_congr Iff.rfl originalLaw

/-- Shared key fibers remain separate through map, witness reduction and count. -/
theorem witnessed_fibers_original (M : Atoms α) (keys : List κ)
    (guard : κ → Formula α) (witnesses : κ → β → List (Formula α))
    (indices : κ → List β) (edges : κ → List (β × β))
    (distinct : ∀ key ∈ keys, (indices key).Nodup)
    (covered : ∀ key ∈ keys, Coverage (indices key) (edges key)) :
    Models M ((keys.flatMap (fun key =>
      witnessedBodies (guard key) (witnesses key) (edges key))).map Ferraris.Neg) ↔
    Models M ((keys.map (fun key =>
      guardedCount (guard key) (lane (witnesses key)) (indices key))).map Ferraris.Neg) := by
  rw [constraints_by_fiber]
  simp only [Models, List.forall_mem_map]
  constructor
  · intro source key present
    exact (witnessed_group_original M (guard key) (witnesses key) (indices key) (edges key)
      (distinct key present) (covered key present)).mp
      (by simpa only [Models, List.forall_mem_map] using source key present)
  · intro grouped key present
    simpa only [Models, List.forall_mem_map] using
      (witnessed_group_original M (guard key) (witnesses key) (indices key) (edges key)
        (distinct key present) (covered key present)).mpr (grouped key present)

/-- Arbitrary frozen interpretations preserve the complete collection of fibers. -/
theorem witnessed_fibers_frozen (M J : Atoms α) (keys : List κ)
    (guard : κ → Formula α) (witnesses : κ → β → List (Formula α))
    (indices : κ → List β) (edges : κ → List (β × β))
    (distinct : ∀ key ∈ keys, (indices key).Nodup)
    (covered : ∀ key ∈ keys, Coverage (indices key) (edges key)) :
    Models J (ReductTheory M ((keys.flatMap (fun key =>
      witnessedBodies (guard key) (witnesses key) (edges key))).map Ferraris.Neg)) ↔
    Models J (ReductTheory M ((keys.map (fun key =>
      guardedCount (guard key) (lane (witnesses key)) (indices key))).map Ferraris.Neg)) := by
  rw [constraints_frozen_iff, constraints_frozen_iff]
  exact witnessed_fibers_original M keys guard witnesses indices edges distinct covered

/-- Separate per-key constraints preserve answer-set membership in any context. -/
theorem witnessed_fibers_stable (M : Atoms α) (context : Theory α) (keys : List κ)
    (guard : κ → Formula α) (witnesses : κ → β → List (Formula α))
    (indices : κ → List β) (edges : κ → List (β × β))
    (distinct : ∀ key ∈ keys, (indices key).Nodup)
    (covered : ∀ key ∈ keys, Coverage (indices key) (edges key)) :
    Stable M (context ++ (keys.flatMap (fun key =>
      witnessedBodies (guard key) (witnesses key) (edges key))).map Ferraris.Neg) ↔
    Stable M (context ++ (keys.map (fun key =>
      guardedCount (guard key) (lane (witnesses key)) (indices key))).map Ferraris.Neg) := by
  rw [ConstrainedPositive.stable_append_constraints,
    ConstrainedPositive.stable_append_constraints]
  exact and_congr Iff.rfl
    (witnessed_fibers_original M keys guard witnesses indices edges distinct covered)

end Zetesis.GroupedWitnessExclusion
