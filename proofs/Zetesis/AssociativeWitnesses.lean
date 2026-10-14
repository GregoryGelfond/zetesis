import Zetesis.FormulaChains
import Zetesis.ThresholdConnectives
import Zetesis.ThresholdPrefixes

/-!
# Witnesses across shared associative formulas

A tree of one connective has the truth of its leaves, even when equal leaves
occur more than once. A cut may stop at a compound condition: another cut of the
same formula may unfold it further. Coverage and containment below therefore
refer to meanings, not to one privileged flattened representation.

These original-satisfaction laws justify witness and unit consequences. They do
not verify a Rust DAG traversal, identity comparison, worklist or frozen-reduct
propagation. The implementation must establish the stated chain and membership
premises before transferring a consequence.
-/

namespace Zetesis.AssociativeWitnesses

open Ferraris FormulaBounds

universe u
variable {α : Type u}

/-- A disjunction chain holds exactly when one of its leaves holds. At a node,
    the witness lies in either child's leaf list. Repeated leaves cause no
    change to this existential statement. -/
theorem disjunction_truth (M : Atoms α) {F : Formula α} {leaves : List (Formula α)}
    (chain : DisjChain F leaves) :
    Satisfies M F ↔ ∃ leaf ∈ leaves, Satisfies M leaf := by
  induction chain with
  | leaf F => simp
  | node left right ihLeft ihRight =>
    change (_ ∨ _) ↔ _
    rw [ihLeft, ihRight]
    simp only [List.mem_append, or_and_right, exists_or]

/-- A conjunction chain holds exactly when every leaf holds. Concatenation
    preserves the requirements from both children, including repeated leaves. -/
theorem conjunction_truth (M : Atoms α) {F : Formula α} {leaves : List (Formula α)}
    (chain : ConjChain F leaves) :
    Satisfies M F ↔ ∀ leaf ∈ leaves, Satisfies M leaf := by
  induction chain with
  | leaf F => simp
  | node left right ihLeft ihRight =>
    change (_ ∧ _) ↔ _
    rw [ihLeft, ihRight]
    constructor
    · intro holds leaf member
      rcases List.mem_append.mp member with inLeft | inRight
      · exact holds.1 leaf inLeft
      · exact holds.2 leaf inRight
    · intro holds
      exact ⟨fun leaf member => holds leaf (List.mem_append_left _ member),
        fun leaf member => holds leaf (List.mem_append_right _ member)⟩

/-- If a disjunction holds and every distinct leaf except U fails, U holds.
    Its occurrence count is irrelevant: any satisfying witness must equal U. -/
theorem disjunction_unit (M : Atoms α) {F U : Formula α}
    {leaves : List (Formula α)} (chain : DisjChain F leaves)
    (holds : Satisfies M F)
    (othersFail : ∀ leaf ∈ leaves, leaf ≠ U → ¬ Satisfies M leaf) :
    Satisfies M U := by
  classical
  obtain ⟨leaf, member, witness⟩ := (disjunction_truth M chain).mp holds
  by_cases same : leaf = U
  · exact same ▸ witness
  · exact False.elim (othersFail leaf member same witness)

/-- If a conjunction fails and every distinct leaf except U holds, U fails.
    Otherwise every leaf would hold, contradicting the failed conjunction. -/
theorem conjunction_unit (M : Atoms α) {F U : Formula α}
    {leaves : List (Formula α)} (chain : ConjChain F leaves)
    (fails : ¬ Satisfies M F)
    (othersHold : ∀ leaf ∈ leaves, leaf ≠ U → Satisfies M leaf) :
    ¬ Satisfies M U := by
  intro unitHolds
  have allHold : ∀ leaf ∈ leaves, Satisfies M leaf := by
    intro leaf member
    by_cases same : leaf = U
    · exact same.symm ▸ unitHolds
    · exact othersHold leaf member same
  exact fails ((conjunction_truth M chain).mpr allHold)

/-- A row covering one disjunction cut contains a true condition whenever the
    formula holds. The cut may stop at compound conditions; it need not unfold
    to atoms or match any other cut used for the same formula. -/
theorem covered_disjunction_has_witness (M : Atoms α) {F : Formula α}
    (row leaves : List (Formula α)) (chain : DisjChain F leaves)
    (covers : ∀ leaf ∈ leaves, leaf ∈ row) (holds : Satisfies M F) :
    ∃ condition ∈ row, Satisfies M condition := by
  obtain ⟨leaf, member, witness⟩ := (disjunction_truth M chain).mp holds
  exact ⟨leaf, covers leaf member, witness⟩

/-- If every row condition occurs in some disjunction cut of F, any true row
    condition makes F true. Different row conditions may use different cuts,
    including cuts that stop at different depths of the formula. -/
theorem contained_disjunction_receives_witness (M : Atoms α) {F : Formula α}
    (row : List (Formula α))
    (contained : ∀ condition ∈ row,
      ∃ leaves, DisjChain F leaves ∧ condition ∈ leaves)
    (witness : ∃ condition ∈ row, Satisfies M condition) : Satisfies M F := by
  obtain ⟨condition, member, holds⟩ := witness
  obtain ⟨leaves, chain, occurs⟩ := contained condition member
  exact (disjunction_truth M chain).mpr ⟨condition, occurs, holds⟩

/-- A row covering one conjunction cut cannot have all its conditions true
    when the formula fails. Otherwise the covering row makes every cut leaf
    true and hence makes the conjunction true. -/
theorem covered_conjunction_lacks_full_row (M : Atoms α) {F : Formula α}
    (row leaves : List (Formula α)) (chain : ConjChain F leaves)
    (covers : ∀ leaf ∈ leaves, leaf ∈ row) (fails : ¬ Satisfies M F) :
    ¬ (∀ condition ∈ row, Satisfies M condition) := by
  intro rowHolds
  have leavesHold : ∀ leaf ∈ leaves, Satisfies M leaf :=
    fun leaf member => rowHolds leaf (covers leaf member)
  exact fails ((conjunction_truth M chain).mpr leavesHold)

/-- If every row condition occurs in some conjunction cut of F, failure of
    the full row implies failure of F. A true F would satisfy each such cut
    and therefore every row condition. -/
theorem contained_conjunction_receives_failure (M : Atoms α) {F : Formula α}
    (row : List (Formula α))
    (contained : ∀ condition ∈ row,
      ∃ leaves, ConjChain F leaves ∧ condition ∈ leaves)
    (fails : ¬ (∀ condition ∈ row, Satisfies M condition)) : ¬ Satisfies M F := by
  intro formulaHolds
  have rowHolds : ∀ condition ∈ row, Satisfies M condition := by
    intro condition member
    obtain ⟨leaves, chain, occurs⟩ := contained condition member
    exact (conjunction_truth M chain).mp formulaHolds condition occurs
  exact fails rowHolds

/-- A unit row contributes zero when all its listed conditions are false.
    Each head contributes zero, so induction reduces the sum to its empty tail. -/
theorem masked_unit_sum_zero (mask : Formula α → Bool) (row : List (Formula α))
    (absent : ∀ condition ∈ row, mask condition = false) :
    ThresholdReduct.maskedSum mask (ThresholdConnectives.unitRow row) = 0 := by
  induction row with
  | nil => simp [ThresholdConnectives.unitRow, ThresholdReduct.maskedSum,
      ThresholdReduct.queries, Thresholds.enabledSum]
  | cons condition rest ih =>
    have headAbsent : mask condition = false := absent condition (by simp)
    have tailAbsent : ∀ leaf ∈ rest, mask leaf = false :=
      fun leaf member => absent leaf (List.mem_cons_of_mem condition member)
    change (if mask condition then 1 else 0) +
      ThresholdReduct.maskedSum mask (ThresholdConnectives.unitRow rest) = 0
    rw [headAbsent, ih tailAbsent]
    rfl

/-- Suppose the inside part of a unit row consists of conditions that each
    imply a disjunction. If the row's selected sum exceeds the number of outside
    positions, the disjunction holds: otherwise the inside contributes zero,
    leaving too few outside positions to meet the lower bound. -/
theorem row_lower_forces_disjunction (M : Atoms α) {F : Formula α}
    (inside outside : List (Formula α))
    (contained : ∀ condition ∈ inside,
      ∃ leaves, DisjChain F leaves ∧ condition ∈ leaves)
    (lower : outside.length < ThresholdReduct.maskedSum
      (ThresholdReduct.active M id) (ThresholdConnectives.unitRow (inside ++ outside))) :
    Satisfies M F := by
  classical
  apply Classical.byContradiction
  intro fails
  have insideAbsent : ∀ condition ∈ inside,
      ThresholdReduct.active M id condition = false := by
    intro condition member
    have absent : ¬ Satisfies M condition := by
      intro holds
      exact fails (contained_disjunction_receives_witness M inside contained
        ⟨condition, member, holds⟩)
    simp [ThresholdReduct.active, absent]
  have split := ThresholdPrefixes.masked_sum_append (ThresholdReduct.active M id)
    (ThresholdConnectives.unitRow inside) (ThresholdConnectives.unitRow outside)
  have insideZero := masked_unit_sum_zero (ThresholdReduct.active M id) inside insideAbsent
  have outsideBound := ThresholdConnectives.masked_unit_sum_le_length
    (ThresholdReduct.active M id) outside
  simp only [ThresholdConnectives.unitRow, List.map_append] at split lower insideZero outsideBound
  omega

/-- Suppose every inside position belongs to some conjunction cut. A true
    conjunction would make all those positions contribute. A selected sum below
    their count therefore proves the conjunction false, even when the row has
    additional outside conditions. -/
theorem row_upper_forces_conjunction_failure (M : Atoms α) {F : Formula α}
    (inside outside : List (Formula α))
    (contained : ∀ condition ∈ inside,
      ∃ leaves, ConjChain F leaves ∧ condition ∈ leaves)
    (upper : ThresholdReduct.maskedSum (ThresholdReduct.active M id)
      (ThresholdConnectives.unitRow (inside ++ outside)) < inside.length) :
    ¬ Satisfies M F := by
  intro holds
  have insideHolds : ∀ condition ∈ inside,
      ThresholdReduct.active M id condition = true := by
    intro condition member
    obtain ⟨leaves, chain, occurs⟩ := contained condition member
    have trueCondition := (conjunction_truth M chain).mp holds condition occurs
    simpa only [ThresholdReduct.active, id_eq, decide_eq_true_eq] using trueCondition
  have insideBound := (ThresholdConnectives.masked_unit_sum_full
    (ThresholdReduct.active M id) inside).mpr insideHolds
  have split := ThresholdPrefixes.masked_sum_append (ThresholdReduct.active M id)
    (ThresholdConnectives.unitRow inside) (ThresholdConnectives.unitRow outside)
  simp only [ThresholdConnectives.unitRow, List.map_append] at split upper insideBound
  omega

end Zetesis.AssociativeWitnesses
