import Zetesis.PairwiseExclusions

/-!
# A factorized source exclusion as a grouped at-most-one constraint

A fiber's indices are distinct source binding identities, never formula IDs.
Its ordinary source edges cover exactly all distinct binding pairs. A shared
condition and the two lane conditions remain formulas; none is promoted from
possible support to a fact. Duplicating the shared guard inside each occurrence
lets the existing PairwiseExclusions algebra replace the complete pair family.
Original truth, every frozen M/J reading and answer sets in any unchanged context
are preserved. Multiple fibers stay separate.

These laws do not verify a Rust recognizer, a source join, provenance,
finite-width counters, resource admission or validation order. Coverage and
binding distinctness are premises. The edge list is specification data; the
laws do not require a runtime Cartesian allocation or traversal to certify it.
-/

namespace Zetesis.GroupedExclusionCoverage

open Ferraris PairwiseExclusions ThresholdConnectives

universe u v w
variable {α : Type u} {β : Type v} {κ : Type w}

/-- Exact membership of the directed distinct Cartesian square supplies sound
endpoints and complete pair coverage. Edge repetition is immaterial. The direct
Coverage premise used below also permits only one orientation per pair. -/
theorem coverage_from_exact_membership (indices : List β) (edges : List (β × β))
    (membership : ∀ left right, (left, right) ∈ edges ↔
      left ∈ indices ∧ right ∈ indices ∧ left ≠ right) :
    Coverage indices edges := by
  constructor
  · rintro ⟨left, right⟩ present
    exact (membership left right).mp present
  · intro left leftIn right rightIn unequal
    exact Or.inl ((membership left right).mpr ⟨leftIn, rightIn, unequal⟩)

/-- A shared condition may remain in both lanes. It is an arbitrary formula,
including a derived atom; no truth or support assumption about it is used. -/
theorem shared_guard_pair (M : Atoms α) (guard left right : Formula α) :
    Satisfies M (Ferraris.Neg (.conj guard (.conj left right))) ↔
      Satisfies M (Ferraris.Neg
        (.conj (.conj guard left) (.conj guard right))) := by
  change (Satisfies M guard ∧ (Satisfies M left ∧ Satisfies M right) → False) ↔
    ((Satisfies M guard ∧ Satisfies M left) ∧
      (Satisfies M guard ∧ Satisfies M right) → False)
  constructor
  · intro denied pair
    exact denied ⟨pair.1.1, pair.1.2, pair.2.2⟩
  · intro denied pair
    exact denied ⟨⟨pair.1, pair.2.1⟩, ⟨pair.1, pair.2.2⟩⟩

/-- The guard rearrangement also preserves every arbitrary frozen reading.
Both enclosing constraints freeze according to their original candidate truth. -/
theorem shared_guard_pair_frozen (M J : Atoms α)
    (guard left right : Formula α) :
    Satisfies J (Reduct M (Ferraris.Neg (.conj guard (.conj left right)))) ↔
      Satisfies J (Reduct M (Ferraris.Neg
        (.conj (.conj guard left) (.conj guard right)))) := by
  rw [constraint_frozen_iff, constraint_frozen_iff]
  exact shared_guard_pair M guard left right

/-- Each original pair body retains the shared guard and both lane conditions. -/
def guardedBodies (guard : Formula α) (lane : β → Formula α)
    (edges : List (β × β)) : List (Formula α) :=
  edges.map (fun edge => .conj guard (.conj (lane edge.1) (lane edge.2)))

/-- Guard distribution preserves the complete negative family occurrence by
occurrence, before invoking any coverage or distinctness property. -/
theorem guarded_family (M : Atoms α) (guard : Formula α)
    (lane : β → Formula α) (edges : List (β × β)) :
    Models M ((guardedBodies guard lane edges).map Ferraris.Neg) ↔
      Models M ((bodies (fun index => .conj guard (lane index)) edges).map Ferraris.Neg) := by
  simp only [Models, guardedBodies, bodies, List.forall_mem_map]
  constructor
  · intro source edge present
    exact (shared_guard_pair M guard (lane edge.1) (lane edge.2)).mp (source edge present)
  · intro grouped edge present
    exact (shared_guard_pair M guard (lane edge.1) (lane edge.2)).mpr (grouped edge present)

/-- A native threshold counts source binding occurrences, retaining two entries
when two distinct bindings happen to have the same condition formula. -/
def groupedBody (guard : Formula α) (lane : β → Formula α)
    (indices : List β) : Formula α :=
  ThresholdReduct.formula (fun index => .conj guard (lane index)) (unitRow indices) 2

/-- Complete pair exclusions become a grouped at-most-one constraint. No
injectivity of lane formulas, uniqueness of formula IDs or fact-status premise
is required. The only distinctness is that of source binding indices. -/
theorem grouped_original (M : Atoms α) (guard : Formula α)
    (lane : β → Formula α) (indices : List β) (edges : List (β × β))
    (distinct : indices.Nodup) (covered : Coverage indices edges) :
    Models M ((guardedBodies guard lane edges).map Ferraris.Neg) ↔
      Satisfies M (Ferraris.Neg (groupedBody guard lane indices)) := by
  exact (guarded_family M guard lane edges).trans
    (PairwiseExclusions.original M (fun index => .conj guard (lane index))
      indices edges distinct covered)

/-- The same replacement preserves every frozen M/J reading, even if J is not
a subset of M. This is a constraint rewrite, not a substitute membership test. -/
theorem grouped_frozen (M J : Atoms α) (guard : Formula α)
    (lane : β → Formula α) (indices : List β) (edges : List (β × β))
    (distinct : indices.Nodup) (covered : Coverage indices edges) :
    Models J (ReductTheory M ((guardedBodies guard lane edges).map Ferraris.Neg)) ↔
      Satisfies J (Reduct M (Ferraris.Neg (groupedBody guard lane indices))) := by
  rw [constraints_frozen_iff, constraint_frozen_iff]
  exact grouped_original M guard lane indices edges distinct covered

/-- The grouped replacement retains exactly the same answer sets in any
unchanged surrounding theory, including rules deriving the shared guard. -/
theorem grouped_stable_in_context (M : Atoms α) (context : Theory α)
    (guard : Formula α) (lane : β → Formula α)
    (indices : List β) (edges : List (β × β))
    (distinct : indices.Nodup) (covered : Coverage indices edges) :
    Stable M (context ++ (guardedBodies guard lane edges).map Ferraris.Neg) ↔
      Stable M (context ++ [Ferraris.Neg (groupedBody guard lane indices)]) := by
  have nativeLaw := ConstrainedPositive.stable_append_constraints context
    [groupedBody guard lane indices] M
  simp only [List.map_cons, List.map_nil] at nativeLaw
  rw [ConstrainedPositive.stable_append_constraints, nativeLaw]
  have originalLaw := grouped_original M guard lane indices edges distinct covered
  simp only [Models, List.mem_singleton, forall_eq] at originalLaw ⊢
  exact and_congr Iff.rfl originalLaw

/-- Negative families over different fiber keys impose separate requirements;
keys may repeat but their carrier occurrences are never pooled across fibers. -/
theorem constraints_by_fiber (M : Atoms α) (keys : List κ)
    (family : κ → List (Formula α)) :
    Models M ((keys.flatMap family).map Ferraris.Neg) ↔
      ∀ key ∈ keys, Models M ((family key).map Ferraris.Neg) := by
  constructor
  · intro combined key present constraint listed
    obtain ⟨body, member, rfl⟩ := List.mem_map.mp listed
    exact combined _ (List.mem_map.mpr ⟨body,
      List.mem_flatMap.mpr ⟨key, present, member⟩, rfl⟩)
  · intro separated constraint listed
    obtain ⟨body, member, rfl⟩ := List.mem_map.mp listed
    obtain ⟨key, present, bodyIn⟩ := List.mem_flatMap.mp member
    exact separated key present _ (List.mem_map.mpr ⟨body, bodyIn, rfl⟩)

/-- Every fiber can be replaced independently. Distinctness and full pair
coverage are required per fiber, not globally across unrelated bindings. -/
theorem fibers_original (M : Atoms α) (keys : List κ)
    (guard : κ → Formula α) (lane : κ → β → Formula α)
    (indices : κ → List β) (edges : κ → List (β × β))
    (distinct : ∀ key ∈ keys, (indices key).Nodup)
    (covered : ∀ key ∈ keys, Coverage (indices key) (edges key)) :
    Models M ((keys.flatMap (fun key => guardedBodies (guard key) (lane key) (edges key))).map
      Ferraris.Neg) ↔
    Models M ((keys.map (fun key => groupedBody (guard key) (lane key) (indices key))).map
      Ferraris.Neg) := by
  rw [constraints_by_fiber]
  simp only [Models, List.forall_mem_map]
  constructor
  · intro source key present
    have fiberLaw := grouped_original M (guard key) (lane key) (indices key) (edges key)
      (distinct key present) (covered key present)
    simp only [Models, List.forall_mem_map] at fiberLaw
    exact fiberLaw.mp (source key present)
  · intro grouped key present
    have fiberLaw := grouped_original M (guard key) (lane key) (indices key) (edges key)
      (distinct key present) (covered key present)
    simp only [Models, List.forall_mem_map] at fiberLaw
    exact fiberLaw.mpr (grouped key present)

/-- Replacing each fiber preserves the entire arbitrary frozen theory. -/
theorem fibers_frozen (M J : Atoms α) (keys : List κ)
    (guard : κ → Formula α) (lane : κ → β → Formula α)
    (indices : κ → List β) (edges : κ → List (β × β))
    (distinct : ∀ key ∈ keys, (indices key).Nodup)
    (covered : ∀ key ∈ keys, Coverage (indices key) (edges key)) :
    Models J (ReductTheory M
      ((keys.flatMap (fun key => guardedBodies (guard key) (lane key) (edges key))).map
        Ferraris.Neg)) ↔
    Models J (ReductTheory M
      ((keys.map (fun key => groupedBody (guard key) (lane key) (indices key))).map
        Ferraris.Neg)) := by
  rw [constraints_frozen_iff, constraints_frozen_iff]
  exact fibers_original M keys guard lane indices edges distinct covered

/-- Fiber replacement preserves complete answer-set membership in any context.
An enumerator still owes its separate coverage and no-duplication contracts. -/
theorem fibers_stable_in_context (M : Atoms α) (context : Theory α) (keys : List κ)
    (guard : κ → Formula α) (lane : κ → β → Formula α)
    (indices : κ → List β) (edges : κ → List (β × β))
    (distinct : ∀ key ∈ keys, (indices key).Nodup)
    (covered : ∀ key ∈ keys, Coverage (indices key) (edges key)) :
    Stable M (context ++
      (keys.flatMap (fun key => guardedBodies (guard key) (lane key) (edges key))).map
        Ferraris.Neg) ↔
    Stable M (context ++
      (keys.map (fun key => groupedBody (guard key) (lane key) (indices key))).map
        Ferraris.Neg) := by
  rw [ConstrainedPositive.stable_append_constraints,
    ConstrainedPositive.stable_append_constraints]
  exact and_congr Iff.rfl (fibers_original M keys guard lane indices edges distinct covered)

/-- Empty and singleton fibers cannot violate at-most-one, regardless of the
shared or lane formulas. A runtime may avoid emitting their tautological root,
but atom creation, validation and provenance still require separate treatment. -/
theorem short_fiber_tautology (M : Atoms α) (guard : Formula α)
    (lane : β → Formula α) (indices : List β) (short : indices.length < 2) :
    Satisfies M (Ferraris.Neg (groupedBody guard lane indices)) := by
  intro satisfied
  have reached := (ThresholdReduct.original M (fun index => .conj guard (lane index))
    (unitRow indices) 2).mp satisfied
  have bounded := masked_unit_sum_le_length
    (ThresholdReduct.active M (fun index => .conj guard (lane index))) indices
  omega

/-- Repeated source witnesses for the same ordered pair do not change its
constraint requirement. This permits quotienting pair occurrences only by exact
binding-pair identity, not quotienting distinct bindings by equal formulas. -/
theorem duplicate_edge_witnesses_irrelevant (M : Atoms α) (guard : Formula α)
    (lane : β → Formula α) (first second : List (β × β))
    (sameMembers : ∀ edge, edge ∈ first ↔ edge ∈ second) :
    Models M ((guardedBodies guard lane first).map Ferraris.Neg) ↔
      Models M ((guardedBodies guard lane second).map Ferraris.Neg) := by
  simp only [Models, guardedBodies, List.forall_mem_map]
  constructor
  · intro original edge member
    exact original edge ((sameMembers edge).mpr member)
  · intro compressed edge member
    exact compressed edge ((sameMembers edge).mp member)

/-! Concrete counterexamples pin the premises a future recognizer may not drop. -/

/-- Two distinct bindings mapping to the same true formula contribute twice.
Deduplicating formula IDs would incorrectly accept the one-entry threshold. -/
theorem formula_deduplication_is_unsound (M : Atoms α) (q : Formula α)
    (holds : Satisfies M q) :
    ¬ Satisfies M (Ferraris.Neg
      (ThresholdReduct.formula (fun _ : Bool => q) (unitRow [false, true]) 2)) ∧
    Satisfies M (Ferraris.Neg
      (ThresholdReduct.formula (fun _ : Bool => q) (unitRow [false]) 2)) := by
  simp [Ferraris.Neg, ThresholdReduct.formula, unitRow, Satisfies, holds]

/-- Repeated witnesses for one binding must not be counted as distinct bindings.
Duplicating a singleton carrier manufactures a nonexistent self-exclusion. -/
theorem duplicate_binding_is_unsound (M : Atoms α) (q : Formula α)
    (holds : Satisfies M q) :
    Satisfies M (Ferraris.Neg
      (ThresholdReduct.formula (fun _ : Bool => q) (unitRow [false]) 2)) ∧
    ¬ Satisfies M (Ferraris.Neg
      (ThresholdReduct.formula (fun _ : Bool => q) (unitRow [false, false]) 2)) := by
  simp [Ferraris.Neg, ThresholdReduct.formula, unitRow, Satisfies, holds]

/-- A guard absent in this interpretation cannot be discarded merely because
it has possible support. Doing so would incorrectly forbid two true lanes. -/
theorem dropping_shared_guard_is_unsound (M : Atoms α) (guard left right : Formula α)
    (absent : ¬ Satisfies M guard) (leftTrue : Satisfies M left)
    (rightTrue : Satisfies M right) :
    Satisfies M (Ferraris.Neg (.conj guard (.conj left right))) ∧
    ¬ Satisfies M (Ferraris.Neg (.conj left right)) := by
  exact ⟨fun held => absent held.1, fun denied => denied ⟨leftTrue, rightTrue⟩⟩

/-- Two singleton fibers admit both conditions. Pooling their distinct keys
creates an exclusion that the original per-fiber program never imposed. -/
theorem merging_fibers_is_unsound (M : Atoms α) (left right : Formula α)
    (leftTrue : Satisfies M left) (rightTrue : Satisfies M right) :
    Satisfies M (Ferraris.Neg (ThresholdReduct.formula
      (fun selected : Bool => if selected then right else left) (unitRow [false]) 2)) ∧
    Satisfies M (Ferraris.Neg (ThresholdReduct.formula
      (fun selected : Bool => if selected then right else left) (unitRow [true]) 2)) ∧
    ¬ Satisfies M (Ferraris.Neg (ThresholdReduct.formula
      (fun selected : Bool => if selected then right else left) (unitRow [false, true]) 2)) := by
  simp [Ferraris.Neg, ThresholdReduct.formula, unitRow, Satisfies, leftTrue, rightTrue]

/-- Connectivity is weaker than complete pair coverage: a three-vertex path
allows its two true endpoints when its middle condition is false. -/
theorem incomplete_pair_coverage_is_unsound (M : Atoms α)
    (left middle right : Formula α) (leftTrue : Satisfies M left)
    (middleFalse : ¬ Satisfies M middle) (rightTrue : Satisfies M right) :
    Models M ((bodies (fun index : Nat =>
      if index = 0 then left else if index = 1 then middle else right)
      [(0, 1), (1, 2)]).map Ferraris.Neg) ∧
    ¬ Satisfies M (Ferraris.Neg (ThresholdReduct.formula
      (fun index : Nat => if index = 0 then left else if index = 1 then middle else right)
      (unitRow [0, 1, 2]) 2)) := by
  simp [Models, bodies, Ferraris.Neg, ThresholdReduct.formula, unitRow,
    Satisfies, leftTrue, middleFalse, rightTrue]

end Zetesis.GroupedExclusionCoverage
