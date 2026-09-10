import Zetesis.AggregateReduct
import Zetesis.GroundGuards
import Zetesis.HeadMeasures

/-!
# Logical bounds on finite numeric aggregates

A count or sum has a numeric result for every finite tuple mask. A logical
bound lying strictly below or above every integer therefore gives a constant
comparison. The comparison law is an explicit premise: this module does not
prove the Rust term comparator or classify concrete source values.

The canonical aggregate agrees with that constant in both the original
interpretation and every frozen M/J pair. Uniform comparison across all masks,
not just classical truth in one candidate, justifies replacement in a body.
The same equivalence composes with the separate permission/bound structure of
an aggregate head. It supplies neither producer support nor a count certificate.

Complete tuple masks and total mathematical integer measures are assumed.
Source eligibility, checked arithmetic, reached evaluation failures, budgets,
CountPlan admission and Rust/WGSL refinement remain separate obligations.
-/

namespace Zetesis.OrderedBounds

open Ferraris ChoiceIntervals AggregateReduct

universe u v w
variable {A : Type u} {V : Type v} {K : Type w} {n : Nat}

/-- A relation consumes the ordering of the numeric result against its bound. -/
inductive Relation where
  | less
  | lessEqual
  | equal
  | notEqual
  | greaterEqual
  | greater

def accepts : Relation → Ordering → Bool
  | .less, order => order == .lt
  | .lessEqual, order => order != .gt
  | .equal, order => order == .eq
  | .notEqual, order => order != .eq
  | .greaterEqual, order => order != .lt
  | .greater, order => order == .gt

/-- The actual measure is unrestricted: signed sums and counts share this law. -/
def guard (relation : Relation) (compare : Int → V → Ordering)
    (measure : Mask n → Int) (bound : V) : Mask n → Bool :=
  fun selected => accepts relation (compare (measure selected) bound)

/-- A bound with the same ordering against every integer yields the same guard
    at every tuple mask. No candidate or eligibility approximation is involved. -/
theorem separated_comparison (relation : Relation) (compare : Int → V → Ordering)
    (measure : Mask n → Int) (bound : V) (order : Ordering)
    (separated : ∀ value, compare value bound = order) (selected : Mask n) :
    guard relation compare measure bound selected = accepts relation order := by
  exact congrArg (accepts relation) (separated (measure selected))

/-- Canonical aggregate constant folding preserves every frozen query. Its
    completeness premise includes masks not realized by any original model. -/
theorem constant_aggregate (conditions : Fin n → Formula A)
    (test : Mask n → Bool) (carrier : List (Mask n)) (complete : Complete carrier)
    (value : Bool) (constant : ∀ selected, test selected = value) :
    Equivalent (aggregate conditions test carrier) (GroundGuards.constant value) := by
  constructor
  · intro M
    rw [AggregateReduct.original M conditions test carrier complete,
      GroundGuards.constant_original, constant]
  · intro M J
    rw [AggregateReduct.frozen M J conditions test carrier complete,
      GroundGuards.constant_frozen, constant, constant]
    exact and_self_iff

/-- Ordered separation specializes the generic canonical aggregate law; it
    does not require monotone eligibility or a subset relation between M and J. -/
theorem separated_aggregate (conditions : Fin n → Formula A)
    (relation : Relation) (compare : Int → V → Ordering) (measure : Mask n → Int)
    (bound : V) (order : Ordering) (separated : ∀ value, compare value bound = order)
    (carrier : List (Mask n)) (complete : Complete carrier) :
    Equivalent (aggregate conditions (guard relation compare measure bound) carrier)
      (GroundGuards.constant (accepts relation order)) := by
  exact constant_aggregate conditions _ carrier complete _
    (separated_comparison relation compare measure bound order separated)

/-- Replacement may occur under any formula connective, including default
    negation. Other leaves retain their full original and frozen meanings. -/
theorem replacement_in_context (context : Formula (Option K))
    (logical : K → Formula A) (conditions : Fin n → Formula A)
    (test : Mask n → Bool) (carrier : List (Mask n)) (complete : Complete carrier)
    (value : Bool) (constant : ∀ selected, test selected = value)
    (M : Atoms A) (theory : Theory A) :
    Stable M (substitute context
      (fun key => key.elim (aggregate conditions test carrier) logical) :: theory) ↔
      Stable M (substitute context
        (fun key => key.elim (GroundGuards.constant value) logical) :: theory) := by
  have leaf : ∀ key : Option K,
      Equivalent (key.elim (aggregate conditions test carrier) logical)
        (key.elim (GroundGuards.constant value) logical) := by
    intro key
    cases key with
    | none => exact constant_aggregate conditions test carrier complete value constant
    | some key => exact equivalent_refl (logical key)
  have same := substitute_equivalent context _ _ leaf
  simp only [Stable, models_cons, ReductTheory, List.map_cons, same.1, same.2]

/-- A folded numeric bound leaves every positive permission and its eligibility
    reduct unchanged. Original aggregate agreement suffices at this boundary. -/
theorem measured_head_in_context (M : Atoms A) (body : Formula A)
    (heads : List A) (eligible : A → Formula A) (conditions : Fin n → Formula A)
    (test : Mask n → Bool) (carrier : List (Mask n)) (complete : Complete carrier)
    (value : Bool) (constant : ∀ selected, test selected = value)
    (theory : Theory A) :
    Stable M (HeadMeasures.group body heads eligible
      (aggregate conditions test carrier) :: theory) ↔
      Stable M (HeadMeasures.group body heads eligible
        (GroundGuards.constant value) :: theory) := by
  exact HeadMeasures.stable_in_context M body heads eligible eligible _ _
    (fun head => equivalent_refl (eligible head))
    (constant_aggregate conditions test carrier complete value constant).1 theory

end Zetesis.OrderedBounds
