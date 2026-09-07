import Zetesis.GroundGuards

/-!
# Finite domains from conjunctive integer bounds

The executable transformer folds lower/upper bounds into an inclusive finite
interval. Its generated values are exactly the values in the original finite
interval satisfying every supplied bound and the complete total Boolean guard.
Mapping these values to arbitrary Ferraris formulas preserves original models,
every frozen M/J reduct, and stability in an unchanged surrounding theory.

The finite initial interval is an explicit premise of the representation; it
can be the machine integer carrier, but this module uses mathematical Int. Bound
recognition, source safety and scope, scheduler dependencies, tuple matching,
undefined scalar evaluation, i32 arithmetic, resource accounting, support
completion, objective presence and the Rust compiler/runtime remain unproved.
The guard is already total; an undefined operation is not a false result.
-/

namespace Zetesis.FiniteBindings

open Ferraris
open ChoiceIntervals (Interval interval mem_interval)

universe u
variable {α : Type u}

inductive Bound where
  | lower : Int → Bound
  | upper : Int → Bound

/-- Independent scalar meaning of one closed bound. -/
def accepts (value : Int) : Bound → Bool
  | .lower lower => decide (lower ≤ value)
  | .upper upper => decide (value ≤ upper)

/-- One executable intersection step; reversed intervals stay empty. -/
def tighten (range : Interval) : Bound → Interval
  | .lower lower => ⟨max range.lower lower, range.upper⟩
  | .upper upper => ⟨range.lower, min range.upper upper⟩

/-- Keep a finite carrier while intersecting all bounds, in source-list order. -/
def compile (range : Interval) (bounds : List Bound) : Interval :=
  bounds.foldl tighten range

theorem strict_endpoints (lower upper value : Int) :
    (accepts value (.lower (lower + 1)) = true ↔ lower < value) ∧
      (accepts value (.upper (upper - 1)) = true ↔ value < upper) := by
  simp only [accepts, decide_eq_true_eq]
  omega

theorem tighten_membership (range : Interval) (bound : Bound) (value : Int) :
    value ∈ interval (tighten range bound) ↔
      value ∈ interval range ∧ accepts value bound = true := by
  cases bound with
  | lower lower =>
    simp only [tighten, mem_interval, accepts, decide_eq_true_eq, Int.max_le]
    simp only [and_assoc, and_left_comm, and_comm]
  | upper upper =>
    simp only [tighten, mem_interval, accepts, decide_eq_true_eq, Int.le_min]
    simp only [and_assoc, and_left_comm, and_comm]

theorem compiled_membership (range : Interval) (bounds : List Bound) (value : Int) :
    value ∈ interval (compile range bounds) ↔
      value ∈ interval range ∧ bounds.all (accepts value) = true := by
  induction bounds generalizing range with
  | nil => simp [compile]
  | cons bound bounds ih =>
    simp only [compile, List.foldl_cons] at ih ⊢
    rw [ih, tighten_membership]
    simp only [List.all_cons, Bool.and_eq_true]
    simp only [and_assoc, and_left_comm, and_comm]

/-- The retained complete guard may remove proposals; it never invents values. -/
def candidates (range : Interval) (bounds : List Bound) (guard : Int → Bool) : List Int :=
  (interval (compile range bounds)).filter guard

/-- Independent finite specification: evaluate all bounds over the original
    carrier, without interval tightening or a dependency planner. -/
def specified (range : Interval) (bounds : List Bound) (guard : Int → Bool) : List Int :=
  (interval range).filter (fun value => bounds.all (accepts value) && guard value)

theorem candidate_membership (range : Interval) (bounds : List Bound)
    (guard : Int → Bool) (value : Int) :
    value ∈ candidates range bounds guard ↔
      range.lower ≤ value ∧ value ≤ range.upper ∧
        bounds.all (accepts value) = true ∧ guard value = true := by
  simp only [candidates, List.mem_filter, compiled_membership, mem_interval]
  simp only [and_assoc, and_left_comm, and_comm]

theorem candidate_specification (range : Interval) (bounds : List Bound)
    (guard : Int → Bool) (value : Int) :
    value ∈ candidates range bounds guard ↔ value ∈ specified range bounds guard := by
  simp only [candidates, specified, List.mem_filter, compiled_membership, Bool.and_eq_true]
  simp only [and_assoc, and_left_comm, and_comm]

/-- Every arbitrary instantiated formula is retained with the same meaning;
    this law permits duplicate instantiations and does not rewrite their bodies. -/
theorem formula_membership (range : Interval) (bounds : List Bound)
    (guard : Int → Bool) (statement : Int → Formula α) (F : Formula α) :
    F ∈ (candidates range bounds guard).map statement ↔
      F ∈ (specified range bounds guard).map statement := by
  simp only [List.mem_map, candidate_specification]

theorem original_in_context (M : Atoms α) (range : Interval) (bounds : List Bound)
    (guard : Int → Bool) (statement : Int → Formula α) (context : Theory α) :
    Models M ((candidates range bounds guard).map statement ++ context) ↔
      Models M ((specified range bounds guard).map statement ++ context) := by
  simp only [Models, List.mem_append, List.mem_map, candidate_specification]

/-- No J⊆M premise is needed: only the finite family representation changes. -/
theorem frozen_in_context (M J : Atoms α) (range : Interval) (bounds : List Bound)
    (guard : Int → Bool) (statement : Int → Formula α) (context : Theory α) :
    Models J (ReductTheory M ((candidates range bounds guard).map statement ++ context)) ↔
      Models J (ReductTheory M ((specified range bounds guard).map statement ++ context)) := by
  simp only [Models, ReductTheory, List.mem_map, List.mem_append, candidate_specification]

theorem stable_in_context (M : Atoms α) (range : Interval) (bounds : List Bound)
    (guard : Int → Bool) (statement : Int → Formula α) (context : Theory α) :
    Stable M ((candidates range bounds guard).map statement ++ context) ↔
      Stable M ((specified range bounds guard).map statement ++ context) := by
  simp only [Stable, original_in_context, frozen_in_context]

end Zetesis.FiniteBindings
