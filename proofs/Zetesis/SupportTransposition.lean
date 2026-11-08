import Zetesis.NormalFerraris
import Zetesis.RuleFactorization

/-!
# Grouping necessary-support guards by condition

A head-indexed row requires every listed condition whenever its head is true.
A condition-indexed row requires that condition whenever any listed head is
true. Complete transposition of their incidences preserves these requirements.
Double negation makes either representation a candidate guard: its frozen truth
depends on the outer interpretation alone.

The formulas may be arbitrary, including recursive or default-negated bodies.
Empty rows and repeated incidences are allowed. The coverage premise is exact;
no subset relation between the outer and tested interpretations is required.
Nested conjunction laws also justify appending entailed requirements while
retaining the original guards. That extension needs entailment, rather than a
second complete incidence table. These semantic laws do not prove that a
compiler builds complete incidence rows, preserves source evidence, or respects
its storage and work bounds.
-/

namespace Zetesis.SupportTransposition

universe u
variable {α : Type u}
open Ferraris

/-- A row's first formula names its head; the remainder lists its requirements. -/
def headGuards (rows : List (Formula α × List (Formula α))) : Theory α :=
  rows.map (fun row => .imp row.1 (NormalFerraris.conjunction row.2))

/-- A column's first formula names a condition; the remainder lists its heads. -/
def conditionGuards (columns : List (Formula α × List (Formula α))) : Theory α :=
  columns.map (fun column => .imp (RuleFactorization.any column.2) column.1)

/-- Every head-condition incidence occurs in both orientations. Multiplicity
does not affect truth; implementations must still account for stored occurrences. -/
def SameIncidences (rows columns : List (Formula α × List (Formula α))) : Prop :=
  ∀ head condition,
    (∃ row ∈ rows, row.1 = head ∧ condition ∈ row.2) ↔
      (∃ column ∈ columns, column.1 = condition ∧ head ∈ column.2)

/-- Head-indexed guards state one implication per incidence. -/
theorem head_guards_exact (M : Atoms α)
    (rows : List (Formula α × List (Formula α))) :
    Models M (headGuards rows) ↔
      ∀ row ∈ rows, ∀ condition ∈ row.2,
        Satisfies M row.1 → Satisfies M condition := by
  constructor
  · intro models row member condition required headTrue
    have rowTrue : Satisfies M (.imp row.1 (NormalFerraris.conjunction row.2)) :=
      models _ (List.mem_map.mpr ⟨row, member, rfl⟩)
    exact (NormalFerraris.satisfies_conjunction M row.2).mp (rowTrue headTrue)
      condition required
  · intro incidences formula member
    obtain ⟨row, present, rfl⟩ := List.mem_map.mp member
    intro headTrue
    exact (NormalFerraris.satisfies_conjunction M row.2).mpr
      (fun condition required => incidences row present condition required headTrue)

/-- Condition-indexed guards state the same implication for every listed head. -/
theorem condition_guards_exact (M : Atoms α)
    (columns : List (Formula α × List (Formula α))) :
    Models M (conditionGuards columns) ↔
      ∀ column ∈ columns, ∀ head ∈ column.2,
        Satisfies M head → Satisfies M column.1 := by
  constructor
  · intro models column member head required headTrue
    have columnTrue : Satisfies M (.imp (RuleFactorization.any column.2) column.1) :=
      models _ (List.mem_map.mpr ⟨column, member, rfl⟩)
    exact columnTrue ((RuleFactorization.satisfies_any M column.2).mpr
      ⟨head, required, headTrue⟩)
  · intro incidences formula member
    obtain ⟨column, present, rfl⟩ := List.mem_map.mp member
    intro someHead
    obtain ⟨head, required, headTrue⟩ :=
      (RuleFactorization.satisfies_any M column.2).mp someHead
    exact incidences column present head required headTrue

/-- Complete transposition preserves all original requirements. Each direction
looks up the same incidence in the other orientation and applies its implication. -/
theorem requirements_equivalent (M : Atoms α)
    (rows columns : List (Formula α × List (Formula α)))
    (complete : SameIncidences rows columns) :
    Models M (headGuards rows) ↔ Models M (conditionGuards columns) := by
  rw [head_guards_exact, condition_guards_exact]
  constructor
  · intro original column member head required headTrue
    obtain ⟨row, present, sameHead, conditionRequired⟩ :=
      (complete head column.1).mpr ⟨column, member, rfl, required⟩
    exact original row present column.1 conditionRequired (sameHead.symm ▸ headTrue)
  · intro transposed row member condition required headTrue
    obtain ⟨column, present, sameCondition, headRequired⟩ :=
      (complete row.1 condition).mp ⟨row, member, rfl, required⟩
    exact sameCondition ▸ transposed column present row.1 headRequired headTrue

/-- A family of double-negated guards has the original truth of its requirements. -/
theorem guarded_original (M : Atoms α) (guards : Theory α) :
    Models M (GuardTheory guards) ↔ Models M guards := by
  simp only [Models, GuardTheory, List.mem_map, forall_exists_index, and_imp,
    forall_apply_eq_imp_iff₂, double_neg_satisfies]

/-- A frozen guard family has its outer truth for every tested interpretation. -/
theorem guarded_frozen (M J : Atoms α) (guards : Theory α) :
    Models J (ReductTheory M (GuardTheory guards)) ↔ Models M guards := by
  simp only [Models, ReductTheory, GuardTheory, List.mem_map, forall_exists_index,
    and_imp, forall_apply_eq_imp_iff₂, double_neg_formula_reduct]

/-- The two guard representations have identical original theory truth. -/
theorem original_exact (M : Atoms α)
    (rows columns : List (Formula α × List (Formula α)))
    (complete : SameIncidences rows columns) :
    Models M (GuardTheory (headGuards rows)) ↔
      Models M (GuardTheory (conditionGuards columns)) := by
  rw [guarded_original, guarded_original]
  exact requirements_equivalent M rows columns complete

/-- The two frozen guard theories agree for arbitrary outer and tested
interpretations. Double negation discharges every dependence on the tested one. -/
theorem frozen_exact (M J : Atoms α)
    (rows columns : List (Formula α × List (Formula α)))
    (complete : SameIncidences rows columns) :
    Models J (ReductTheory M (GuardTheory (headGuards rows))) ↔
      Models J (ReductTheory M (GuardTheory (conditionGuards columns))) := by
  rw [guarded_frozen, guarded_frozen]
  exact requirements_equivalent M rows columns complete

/-- Replacing the guard family preserves answer-set membership in every
unchanged surrounding theory. No necessary-support theorem is assumed here:
this replaces equivalent guards rather than omitting them. -/
theorem answer_sets_in_context (M : Atoms α) (context : Theory α)
    (rows columns : List (Formula α × List (Formula α)))
    (complete : SameIncidences rows columns) :
    Stable M (GuardTheory (headGuards rows) ++ context) ↔
      Stable M (GuardTheory (conditionGuards columns) ++ context) := by
  rw [stable_guard_theory_iff, stable_guard_theory_iff]
  exact and_congr Iff.rfl (requirements_equivalent M rows columns complete)

/-- A condition occurs beneath conjunctions only. Other connectives are opaque:
necessity must not be inferred by descending through a disjunction or implication. -/
inductive ConjunctOf (condition : Formula α) : Formula α → Prop where
  | here : ConjunctOf condition condition
  | left {first second : Formula α} :
      ConjunctOf condition first → ConjunctOf condition (.conj first second)
  | right {first second : Formula α} :
      ConjunctOf condition second → ConjunctOf condition (.conj first second)

/-- A true conjunction makes every nested conjunct true. Shared implementation
nodes and repeated paths do not change this formula-level implication. -/
theorem nested_conjunct_required (M : Atoms α) (condition body : Formula α)
    (occurs : ConjunctOf condition body) (bodyTrue : Satisfies M body) :
    Satisfies M condition := by
  induction occurs with
  | here => exact bodyTrue
  | left _ required => exact required bodyTrue.1
  | right _ required => exact required bodyTrue.2

/-- A necessary-support implication also requires any nested conjunct of its
body. Complete producer recognition is a premise of supplying that implication;
this law does not obtain support from an unfinished producer family. -/
theorem necessary_conjunct (M : Atoms α) (head condition body : Formula α)
    (occurs : ConjunctOf condition body)
    (necessary : Satisfies M (.imp head body)) :
    Satisfies M (.imp head condition) := by
  intro headTrue
  have bodyTrue : Satisfies M body := necessary headTrue
  exact nested_conjunct_required M condition body occurs bodyTrue

/-- Appending consequences of an existing guard family changes no original
truth. The original guards remain, so only the forward entailment is required. -/
theorem supplemental_original (M : Atoms α) (guards extra : Theory α)
    (entailed : Models M guards → Models M extra) :
    Models M (GuardTheory extra ++ GuardTheory guards) ↔
      Models M (GuardTheory guards) := by
  simp only [RuleFactorization.models_append, guarded_original]
  constructor
  · exact And.right
  · intro original
    exact ⟨entailed original, original⟩

/-- Double-negated consequences preserve frozen truth for arbitrary M and J.
No subset relation between the two interpretations is needed. -/
theorem supplemental_frozen (M J : Atoms α) (guards extra : Theory α)
    (entailed : Models M guards → Models M extra) :
    Models J (ReductTheory M (GuardTheory extra ++ GuardTheory guards)) ↔
      Models J (ReductTheory M (GuardTheory guards)) := by
  simp only [ReductTheory, List.map_append, RuleFactorization.models_append]
  change (Models J (ReductTheory M (GuardTheory extra)) ∧
    Models J (ReductTheory M (GuardTheory guards))) ↔
      Models J (ReductTheory M (GuardTheory guards))
  simp only [guarded_frozen]
  constructor
  · exact And.right
  · intro original
    exact ⟨entailed original, original⟩

/-- Supplementary necessary conditions preserve answer sets in an unchanged
context. This semantic law leaves concrete producer completeness, DAG traversal,
source evidence and resource accounting as separate implementation obligations. -/
theorem supplemental_answer_sets (M : Atoms α) (context guards extra : Theory α)
    (entailed : Models M guards → Models M extra) :
    Stable M (GuardTheory extra ++ (GuardTheory guards ++ context)) ↔
      Stable M (GuardTheory guards ++ context) := by
  rw [stable_guard_theory_iff]
  constructor
  · exact And.left
  · intro accepted
    have original : Models M guards :=
      ((stable_guard_theory_iff M context guards).mp accepted).2
    exact ⟨accepted, entailed original⟩

end Zetesis.SupportTransposition
