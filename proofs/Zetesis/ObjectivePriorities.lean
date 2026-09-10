import Zetesis.ObjectiveValues

/-!
# Correlated objective values and priority partitions

Each finite resolved row retains its weight, priority, tuple and eligibility
together. Optional integers denote total numeric projections of logical values;
none means nonnumeric, never failed evaluation. A contribution requires both
numeric fields from the same row. Its priority remains present when the numeric
weight is zero or the row is inactive in a particular interpretation.

Priority partitions retain those complete rows. Evaluating a slot from its own
partition preserves the full cost vector and optimum ties after the existing
direction normalization and key coalescing. The original Ferraris theory remains
unchanged; objectives do not supply answer-set acceptance or producer support.

Exact eligible grounding rows are an explicit premise, distinct from possible
support and model-relative activation. These laws do not prove source joins,
aggregate-generated priority presence, term evaluation, checked i32 arithmetic,
resource completion or Rust/GPU refinement.
-/

namespace Zetesis.ObjectivePriorities

open ObjectiveDirections ObjectiveValues

universe u v w
variable {σ : Type u} {χ : Type v} {B : Type w}

/-- Priority selection preserves every other field of the same resolved row. -/
def select (row : ResolvedEntry σ χ (Option Int)) : Option (ResolvedEntry σ χ) :=
  row.priority.map fun priority =>
    ⟨row.direction, row.weight, priority, row.tuple, row.condition⟩

def entries (rows : List (ResolvedEntry σ χ (Option Int))) : List (ResolvedEntry σ χ) :=
  rows.filterMap select

/-- Presence requires a correlated numeric weight/priority witness, not separate
    witnesses for the two fields and not an accepted answer set. -/
def Present (priority : Int) (rows : List (ResolvedEntry σ χ (Option Int))) : Prop :=
  ∃ row ∈ rows, row.priority = some priority ∧ row.weight.isSome = true

theorem selected_presence (priority : Int)
    (rows : List (ResolvedEntry σ χ (Option Int))) :
    ObjectiveValues.Present priority (entries rows) ↔ Present priority rows := by
  constructor
  · rintro ⟨entry, member, same, numeric⟩
    obtain ⟨row, inside, selected⟩ := List.mem_filterMap.mp member
    cases found : row.priority with
    | none => simp [select, found] at selected
    | some value =>
      have equal : (⟨row.direction, row.weight, value, row.tuple, row.condition⟩ :
          ResolvedEntry σ χ) = entry := by simpa [select, found] using selected
      subst entry
      exact ⟨row, inside, found.trans (congrArg some same), numeric⟩
  · rintro ⟨row, inside, found, numeric⟩
    refine ⟨⟨row.direction, row.weight, priority, row.tuple, row.condition⟩, ?_, rfl, numeric⟩
    exact List.mem_filterMap.mpr ⟨row, inside, by simp [select, found]⟩

/-- A complete eligible binding carrier supplies exactly the source witnesses
    of a priority. All fields are resolved at that one binding. -/
theorem completed_presence (priority : Int) (bindings : List B) (eligible : B → Prop)
    (complete : ∀ binding, binding ∈ bindings ↔ eligible binding)
    (resolve : B → ResolvedEntry σ χ (Option Int)) :
    Present priority (bindings.map resolve) ↔
      ∃ binding, eligible binding ∧ (resolve binding).priority = some priority ∧
        (resolve binding).weight.isSome = true := by
  simp only [Present, List.mem_map]
  constructor
  · rintro ⟨row, ⟨binding, inside, rfl⟩, atPriority, numeric⟩
    exact ⟨binding, (complete binding).mp inside, atPriority, numeric⟩
  · rintro ⟨binding, inside, atPriority, numeric⟩
    exact ⟨resolve binding, ⟨binding, (complete binding).mpr inside, rfl⟩, atPriority, numeric⟩

/-- A totally evaluated nonnumeric priority excludes its complete contribution,
    including a numeric sibling weight. No failed evaluation is represented here. -/
theorem ignored_priority_preserves_vector [DecidableEq σ]
    (priorities : List Int) (truth : χ → Bool)
    (row : ResolvedEntry σ χ (Option Int)) (rows : List (ResolvedEntry σ χ (Option Int)))
    (ignored : row.priority = none) :
    mixedVector priorities truth (numericEntries (entries (row :: rows))) =
      mixedVector priorities truth (numericEntries (entries rows)) := by
  simp [entries, select, ignored]

/-- Separate numeric-weight and priority witnesses can invent a priority that
    has no eligible numeric contribution. Keeping whole rows prevents that join. -/
theorem independent_fields_invent_priority :
    let rows : List (ResolvedEntry Unit Unit (Option Int)) :=
      [⟨.minimize, none, some 0, [], ()⟩, ⟨.minimize, some 1, some 1, [], ()⟩]
    (∃ row ∈ rows, row.priority = some 0) ∧
      (∃ row ∈ rows, row.weight.isSome = true) ∧ ¬ Present 0 rows := by
  simp [Present]

/-- Retain the complete numeric row, rather than independently projecting its
    weight, priority or condition before specialization. -/
def partition (priority : Int) (rows : List (Entry σ χ)) : List (Entry σ χ) :=
  rows.filter (fun row => decide (row.priority = priority))

/-- A contribution at another priority has zero value in this slot and cannot
    duplicate its keys: priority is part of complete contribution identity. -/
theorem raw_partition_cost [DecidableEq σ] (priority : Int) (rows : List (Entry σ χ)) :
    rawCost priority (partition priority rows) = rawCost priority rows := by
  induction rows with
  | nil => rfl
  | cons row rest ih =>
    by_cases atPriority : row.priority = priority
    · have retained : partition priority (row :: rest) = row :: partition priority rest := by
        simp [partition, atPriority]
      have witnesses :
          (partition priority rest).any (fun other => decide (SameContribution row other)) = true ↔
            rest.any (fun other => decide (SameContribution row other)) = true := by
        simp only [partition, List.any_eq_true, List.mem_filter, decide_eq_true_eq]
        constructor
        · rintro ⟨other, ⟨inside, _⟩, same⟩
          exact ⟨other, inside, same⟩
        · rintro ⟨other, inside, same⟩
          exact ⟨other, ⟨inside, same.1.symm.trans atPriority⟩, same⟩
      have duplicates :
          (partition priority rest).any (fun other => decide (SameContribution row other)) =
            rest.any (fun other => decide (SameContribution row other)) := by
        cases left : (partition priority rest).any
            (fun other => decide (SameContribution row other)) <;>
          cases right : rest.any (fun other => decide (SameContribution row other)) <;>
          simp_all
      rw [retained]
      simp only [rawCost, duplicates, ih]
    · have discarded : partition priority (row :: rest) = partition priority rest := by
        simp [partition, atPriority]
      rw [discarded, ih]
      simp [rawCost, contributionAt, atPriority]

/-- Model-relative eligibility is retained inside each priority partition. -/
theorem partition_cost [DecidableEq σ] (priority : Int) (truth : χ → Bool)
    (rows : List (Entry σ χ)) :
    mixedCost priority truth (partition priority rows) = mixedCost priority truth rows := by
  have active : (partition priority rows).filter (fun row => truth row.condition) =
      partition priority (rows.filter (fun row => truth row.condition)) := by
    simp only [partition, List.filter_filter]
    simp only [Bool.and_comm]
  exact (congrArg (rawCost priority) active).trans
    (raw_partition_cost priority (rows.filter (fun row => truth row.condition)))

/-- Every slot may evaluate its own complete partition. The supplied priority
    layout stays fixed across candidates, including its zero-valued slots. -/
theorem partition_vector [DecidableEq σ] (priorities : List Int) (truth : χ → Bool)
    (rows : List (Entry σ χ)) :
    priorities.map (fun priority => mixedCost priority truth (partition priority rows)) =
      mixedVector priorities truth rows := by
  simp only [partition_cost, mixedVector]

/-- Per-priority evaluation preserves the original answer sets and every
    optimum tie. This argument assumes no enumeration or search completion. -/
theorem partition_optima [DecidableEq σ] {A : Type w} (theory : Ferraris.Theory A)
    (priorities : List Int) (truth : Atoms A → χ → Bool) (rows : List (Entry σ χ))
    (M : Atoms A) :
    Optimal theory (fun candidate => priorities.map (fun priority =>
      mixedCost priority (truth candidate) (partition priority rows))) M ↔
      Optimal theory (fun candidate => mixedVector priorities (truth candidate) rows) M := by
  simp only [partition_vector]

end Zetesis.ObjectivePriorities
