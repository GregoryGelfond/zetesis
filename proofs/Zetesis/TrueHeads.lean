import Zetesis.GroundGuards
import Zetesis.NegativeHeads

/-!
# Explicitly true disjunct conditions over complete whole-rule families

Empty conditions and conjunctions of atom-free guards with total true evaluation
can be erased, preserving signed head occurrences in original truth and every
frozen M/J reduct. Complete finite rows retain the Cartesian family of whole
rules, including empty families. The positive-head producer law is unchanged.

This is the contract of the true-condition slice, not a translation of dynamic
conditional disjunctions. False/dynamic conditions, scope, source normalization,
machine ranges, provenance, resources and Rust refinement remain outside this
proof. Complete finite instantiation is a premise, never inferred from a single
candidate or a partially explored local domain.
-/

namespace Zetesis.TrueHeads

open Ferraris
open GroundGuards (Guard evaluate formula)
open ChoiceIntervals (Interval product Binds)

universe u
variable {α : Type u}

structure Occurrence (α : Type u) where
  literal : NegativeHeads.Literal α
  condition : List Guard

def admitted (occurrence : Occurrence α) : Prop :=
  ∀ guard ∈ occurrence.condition, evaluate guard = true

def condition : List Guard → Formula α
  | [] => .imp .bot .bot
  | guard :: rest => .conj (formula guard) (condition rest)

theorem condition_original (M : Atoms α) (guards : List Guard)
    (truth : ∀ guard ∈ guards, evaluate guard = true) :
    Satisfies M (condition guards) := by
  induction guards with
  | nil => simp [condition, Satisfies]
  | cons guard rest ih =>
    exact ⟨(GroundGuards.guard_original M guard).mpr (truth guard (by simp)),
      ih (fun member inside => truth member (by simp [inside]))⟩

theorem condition_frozen (M J : Atoms α) (guards : List Guard)
    (truth : ∀ guard ∈ guards, evaluate guard = true) :
    Satisfies J (Reduct M (condition guards)) := by
  induction guards with
  | nil => simp [condition, Reduct, Satisfies]
  | cons guard rest ih =>
    rw [condition, RuleFactorization.reduct_conj]
    exact ⟨(GroundGuards.guard_frozen M J guard).mpr (truth guard (by simp)),
      ih (fun member inside => truth member (by simp [inside]))⟩

/-- This implication is used only when the condition is true in every world;
    no formula law for arbitrary conditional-head conditions is asserted. -/
def guardedLiteral (occurrence : Occurrence α) : Formula α :=
  .imp (condition occurrence.condition) (NegativeHeads.literal occurrence.literal)

theorem literal_original (M : Atoms α) (occurrence : Occurrence α)
    (truth : admitted occurrence) :
    Satisfies M (guardedLiteral occurrence) ↔
      Satisfies M (NegativeHeads.literal occurrence.literal) := by
  simp only [guardedLiteral, Satisfies]
  exact ⟨fun holds => holds (condition_original M occurrence.condition truth),
    fun holds _ => holds⟩

theorem literal_frozen (M J : Atoms α) (occurrence : Occurrence α)
    (truth : admitted occurrence) :
    Satisfies J (Reduct M (guardedLiteral occurrence)) ↔
      Satisfies J (Reduct M (NegativeHeads.literal occurrence.literal)) := by
  rw [guardedLiteral, RuleFactorization.reduct_imp]
  constructor
  · intro holds
    exact holds.2 (condition_frozen M J occurrence.condition truth)
  · intro holds
    exact ⟨fun _ => NegativeHeads.reduct_truth_requires_original M J _ holds,
      fun _ => holds⟩

def guardedHead : List (Occurrence α) → Formula α
  | [] => .bot
  | occurrence :: rest => .disj (guardedLiteral occurrence) (guardedHead rest)

def erasedRule (body : Formula α) (occurrences : List (Occurrence α)) :
    NegativeHeads.Rule α := ⟨body, occurrences.map Occurrence.literal⟩

def sourceRule (body : Formula α) (occurrences : List (Occurrence α)) : Formula α :=
  .imp body (guardedHead occurrences)

theorem head_original (M : Atoms α) (occurrences : List (Occurrence α))
    (truth : ∀ occurrence ∈ occurrences, admitted occurrence) :
    Satisfies M (guardedHead occurrences) ↔
      Satisfies M (NegativeHeads.head (occurrences.map Occurrence.literal)) := by
  induction occurrences with
  | nil => rfl
  | cons occurrence rest ih =>
    simp only [guardedHead, Satisfies, List.map_cons, NegativeHeads.head,
      RuleFactorization.any]
    exact or_congr (literal_original M occurrence (truth occurrence (by simp)))
      (ih (fun member inside => truth member (by simp [inside])))

theorem head_frozen (M J : Atoms α) (occurrences : List (Occurrence α))
    (truth : ∀ occurrence ∈ occurrences, admitted occurrence) :
    Satisfies J (Reduct M (guardedHead occurrences)) ↔
      Satisfies J (Reduct M (NegativeHeads.head (occurrences.map Occurrence.literal))) := by
  induction occurrences with
  | nil => rfl
  | cons occurrence rest ih =>
    simp only [guardedHead, List.map_cons, NegativeHeads.head, RuleFactorization.any,
      RuleFactorization.reduct_disj]
    exact or_congr (literal_frozen M J occurrence (truth occurrence (by simp)))
      (ih (fun member inside => truth member (by simp [inside])))

theorem rule_original (M : Atoms α) (body : Formula α)
    (occurrences : List (Occurrence α))
    (truth : ∀ occurrence ∈ occurrences, admitted occurrence) :
    Satisfies M (sourceRule body occurrences) ↔
      Satisfies M (NegativeHeads.ruleFormula (erasedRule body occurrences)) := by
  exact imp_congr Iff.rfl (head_original M occurrences truth)

theorem rule_frozen (M J : Atoms α) (body : Formula α)
    (occurrences : List (Occurrence α))
    (truth : ∀ occurrence ∈ occurrences, admitted occurrence) :
    Satisfies J (Reduct M (sourceRule body occurrences)) ↔
      Satisfies J (Reduct M (NegativeHeads.ruleFormula (erasedRule body occurrences))) := by
  simp only [sourceRule, NegativeHeads.ruleFormula, erasedRule,
    RuleFactorization.reduct_imp, Satisfies]
  exact and_congr (imp_congr Iff.rfl (head_original M occurrences truth))
    (imp_congr Iff.rfl (head_frozen M J occurrences truth))

/-- Each row is one completed whole-rule substitution, not a flattened head. -/
def sourceFamily (body : Formula α) (rows : List (List (Occurrence α))) : Theory α :=
  rows.map (sourceRule body)

def erasedRules (body : Formula α) (rows : List (List (Occurrence α))) :
    List (NegativeHeads.Rule α) := rows.map (erasedRule body)

def erasedFamily (body : Formula α) (rows : List (List (Occurrence α))) : Theory α :=
  NegativeHeads.theory (erasedRules body rows)

theorem family_original (M : Atoms α) (body : Formula α)
    (rows : List (List (Occurrence α)))
    (truth : ∀ row ∈ rows, ∀ occurrence ∈ row, admitted occurrence) :
    Models M (sourceFamily body rows) ↔ Models M (erasedFamily body rows) := by
  simp only [sourceFamily, erasedFamily, erasedRules, NegativeHeads.theory,
    List.map_map, Function.comp_def, Models, List.forall_mem_map]
  apply forall_congr'
  intro row
  apply forall_congr'
  intro member
  exact rule_original M body row (truth row member)

theorem family_frozen (M J : Atoms α) (body : Formula α)
    (rows : List (List (Occurrence α)))
    (truth : ∀ row ∈ rows, ∀ occurrence ∈ row, admitted occurrence) :
    Models J (ReductTheory M (sourceFamily body rows)) ↔
      Models J (ReductTheory M (erasedFamily body rows)) := by
  simp only [sourceFamily, erasedFamily, erasedRules, NegativeHeads.theory,
    ReductTheory, List.map_map, Function.comp_def, Models, List.forall_mem_map]
  apply forall_congr'
  intro row
  apply forall_congr'
  intro member
  exact rule_frozen M J body row (truth row member)

theorem stable_in_context (M : Atoms α) (body : Formula α)
    (rows : List (List (Occurrence α)))
    (truth : ∀ row ∈ rows, ∀ occurrence ∈ row, admitted occurrence)
    (context : Theory α) :
    Stable M (sourceFamily body rows ++ context) ↔
      Stable M (erasedFamily body rows ++ context) := by
  have frozen (J : Atoms α) :
      Models J ((sourceFamily body rows).map (Reduct M)) ↔
        Models J ((erasedFamily body rows).map (Reduct M)) :=
    family_frozen M J body rows truth
  simp only [Stable, ReductTheory, List.map_append, RuleFactorization.models_append,
    family_original M body rows truth, frozen]

def instances (ranges : List Interval) (head : List Int → List (Occurrence α)) :
    List (List (Occurrence α)) := (product ranges).map head

theorem instances_complete (ranges : List Interval)
    (head : List Int → List (Occurrence α)) (row : List (Occurrence α)) :
    row ∈ instances ranges head ↔ ∃ values, Binds ranges values ∧ head values = row := by
  simp [instances, ChoiceIntervals.product_complete]

theorem empty_range_erases_whole_family (before after : List Interval) (range : Interval)
    (reversed : range.upper < range.lower) (head : List Int → List (Occurrence α))
    (body : Formula α) :
    sourceFamily body (instances (before ++ range :: after) head) = [] := by
  simp [sourceFamily, instances, ChoiceIntervals.product_empty_at before after range reversed]

/-- Neither negative polarity supplies a producer after erasing true guards. -/
theorem stable_has_positive_producer (M : Atoms α) (body : Formula α)
    (rows : List (List (Occurrence α)))
    (truth : ∀ row ∈ rows, ∀ occurrence ∈ row, admitted occurrence)
    (stable : Stable M (sourceFamily body rows)) (atom : α) (present : M atom) :
    NegativeHeads.HasProducer M (erasedRules body rows) atom := by
  have erased : Stable M (erasedFamily body rows) := by
    simpa using (stable_in_context M body rows truth []).mp (by simpa using stable)
  exact NegativeHeads.stable_has_positive_producer M (erasedRules body rows) erased atom present

end Zetesis.TrueHeads
