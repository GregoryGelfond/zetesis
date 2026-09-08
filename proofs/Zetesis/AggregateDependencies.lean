import Zetesis.AggregateAssignment
import Zetesis.Ferraris

/-!
# Aggregate candidate families indexed by completed predecessors

A dependent aggregate selects a finite candidate family for each completed
outer row. This is a dependent sum of carriers, not an independent Cartesian
product. Rows retain their predecessor association, including equal child
values obtained from different parents. Candidate coverage composes when the
actual outer row is covered and its own child family covers the actual result.

The sum specialization uses the existing complete-full-tuple coverage theorem.
An emitted clause retains both its predecessor equalities/activation and the
dependent aggregate equality. Original and arbitrary frozen M/J laws describe
that same clause family, without replacing a proposal by aggregate truth.

These finite denotational laws assume complete carriers and total value/tuple
functions. They do not prove Rust free-input extraction, source safety,
topological scheduling, local joins, candidate/caching/reset implementation,
machine arithmetic, possible-support completion or resource accounting. Cyclic
source bindings have no completeness premise supplied by this module. No Rust
or WGSL refinement is claimed.
-/

namespace Zetesis.AggregateDependencies
open Ferraris
universe u v w
variable {V : Type u} {W : Type v} {A : Type w}

def rows (carrier : List V) (family : V → List W) : List (V × W) :=
  carrier.flatMap (fun parent => (family parent).map (fun child => (parent, child)))

/-- A child belongs to its own predecessor's carrier, not a union of families. -/
theorem row_membership (carrier : List V) (family : V → List W)
    (parent : V) (child : W) :
    (parent, child) ∈ rows carrier family ↔ parent ∈ carrier ∧ child ∈ family parent := by
  simp only [rows, List.mem_flatMap, List.mem_map]
  constructor
  · rintro ⟨outer, member, value, inside, same⟩
    cases same
    exact ⟨member, inside⟩
  · rintro ⟨member, inside⟩
    exact ⟨parent, member, child, inside, rfl⟩

/-- Completed coverage composes for a dependent next value. -/
theorem covered_extension (carrier : List V) (family : V → List W)
    (parent : V) (actual : W) (covered : parent ∈ carrier)
    (child_covered : actual ∈ family parent) :
    (parent, actual) ∈ rows carrier family :=
  (row_membership carrier family parent actual).mpr ⟨covered, child_covered⟩

/-- Full-tuple coverage for the actual predecessor covers its signed sum.
    Tuple eligibility may be correlated; no independence premise is required. -/
theorem covered_sum (carrier : List V)
    (tuples : V → List AggregateAssignment.FullTuple)
    (elements : V → List AggregateAssignment.Element)
    (weight : AggregateAssignment.FullTuple → Int) (parent : V)
    (covered : parent ∈ carrier)
    (tuple_coverage : ∀ element, element ∈ elements parent → element.eligible = true →
      element.tuple ∈ tuples parent) :
    (parent, AggregateAssignment.actualSum weight (elements parent)) ∈ rows carrier
      (fun outer => AggregateAssignment.subsetSums
        ((AggregateAssignment.unique (tuples outer)).map weight)) := by
  have child_covered := AggregateAssignment.actual_tuple_sum_is_candidate
    weight (tuples parent) (elements parent) tuple_coverage
  exact covered_extension carrier _ parent _ covered child_covered

/-- A further dependent family preserves the complete two-value prefix. -/
theorem covered_continuation {X : Type w} (carrier : List V)
    (family : V → List W) (continuation : V × W → List X)
    (parent : V) (child : W) (actual : X)
    (covered : parent ∈ carrier) (child_covered : child ∈ family parent)
    (next_covered : actual ∈ continuation (parent, child)) :
    ((parent, child), actual) ∈ rows (rows carrier family) continuation := by
  have prefix_covered : (parent, child) ∈ rows carrier family :=
    covered_extension carrier family parent child covered child_covered
  exact covered_extension (rows carrier family) continuation _ actual prefix_covered next_covered

/-- Later predecessor rows contribute their own complete families even when
    earlier rows contain equal parent values or equal child values. -/
theorem appended_predecessors (before after : List V) (family : V → List W) :
    rows (before ++ after) family = rows before family ++ rows after family := by
  simp [rows, List.flatMap_append]

def clause (previous equality head : Formula A) : Formula A :=
  .imp (.conj previous equality) head

def clauses (carrier : List V) (family : V → List W)
    (previous : V → Formula A) (equality head : V → W → Formula A) : Theory A :=
  (rows carrier family).map
    (fun row => clause (previous row.1) (equality row.1 row.2) (head row.1 row.2))

/-- Each original row is conditional on both its prior activation/equalities
    and its own aggregate equality. Membership alone proves neither condition. -/
theorem original_rows (M : Atoms A) (carrier : List V) (family : V → List W)
    (previous : V → Formula A) (equality head : V → W → Formula A) :
    Models M (clauses carrier family previous equality head) ↔
      ∀ parent ∈ carrier, ∀ child ∈ family parent,
        Satisfies M (previous parent) ∧ Satisfies M (equality parent child) →
          Satisfies M (head parent child) := by
  constructor
  · intro model parent member child inside
    have row := covered_extension carrier family parent child member inside
    have satisfied := model _ (List.mem_map.mpr ⟨(parent, child), row, rfl⟩)
    exact satisfied
  · intro all formula member
    obtain ⟨⟨parent, child⟩, row, rfl⟩ := List.mem_map.mp member
    have association := (row_membership carrier family parent child).mp row
    exact all parent association.1 child association.2

/-- Reduct formation preserves the exact complete clause associated with each
    dependent row. The frozen interpretation is arbitrary, including J ⊄ M. -/
theorem frozen_rows (M J : Atoms A) (carrier : List V) (family : V → List W)
    (previous : V → Formula A) (equality head : V → W → Formula A) :
    Models J (ReductTheory M (clauses carrier family previous equality head)) ↔
      ∀ parent ∈ carrier, ∀ child ∈ family parent,
        Satisfies J (Reduct M
          (clause (previous parent) (equality parent child) (head parent child))) := by
  constructor
  · intro model parent member child inside
    have row := covered_extension carrier family parent child member inside
    have original : clause (previous parent) (equality parent child) (head parent child) ∈
        clauses carrier family previous equality head :=
      List.mem_map.mpr ⟨(parent, child), row, rfl⟩
    exact model _ (List.mem_map.mpr ⟨_, original, rfl⟩)
  · intro all formula member
    obtain ⟨original, original_member, rfl⟩ := List.mem_map.mp member
    obtain ⟨⟨parent, child⟩, row, rfl⟩ := List.mem_map.mp original_member
    have association := (row_membership carrier family parent child).mp row
    exact all parent association.1 child association.2

end Zetesis.AggregateDependencies
