import Zetesis.Core

/-!
# Completed Boolean witness validation by short-circuit clause scans

For a fixed complete interpretation, literal truth is a total Boolean query.
A clause scan stops at the first true literal; an outer validator still requires
all clauses. The executable scans agree with independent existential/universal
acceptance and inspect no more literal occurrences than the supplied CNF.

This is a law for completed classical candidate/countermodel validation. It
assumes total, fixed literal truth. It does not prove assignment construction,
CNF index validity, Rust arrays, counters, allocation, cancellation, work-budget
interruptions, SAT search, or Ferraris reduct construction. In particular it
cannot justify rewriting an original formula before its reduct is frozen.
-/

namespace Zetesis.ClauseValidation

universe u
variable {α : Type u}

structure Result where
  accepted : Bool
  inspected : Nat
  deriving DecidableEq

/-- Count only queried literals, including the first satisfying occurrence. -/
def scan (truth : α → Bool) : List α → Result
  | [] => ⟨false, 0⟩
  | literal :: rest =>
      if truth literal then ⟨true, 1⟩
      else let suffix := scan truth rest
           ⟨suffix.accepted, suffix.inspected + 1⟩

theorem scan_empty (truth : α → Bool) : scan truth [] = ⟨false, 0⟩ := rfl

/-- The reference does not mention the implementation's scan or stopping rule. -/
theorem scan_accepts_iff (truth : α → Bool) (clause : List α) :
    (scan truth clause).accepted = true ↔ ∃ literal ∈ clause, truth literal = true := by
  induction clause with
  | nil => simp [scan]
  | cons literal rest ih =>
    cases h : truth literal <;> simp [scan, h, ih]

theorem scan_work_bound (truth : α → Bool) (clause : List α) :
    (scan truth clause).inspected ≤ clause.length := by
  induction clause with
  | nil => simp [scan]
  | cons literal rest ih =>
    cases h : truth literal <;> simp [scan, h] <;> omega

theorem first_true_stops (truth : α → Bool) (literal : α) (rest : List α)
    (satisfied : truth literal = true) : scan truth (literal :: rest) = ⟨true, 1⟩ := by
  simp [scan, satisfied]

/-- A false clause must inspect every occurrence before it can be rejected. -/
theorem all_false_exact (truth : α → Bool) (clause : List α)
    (falseClause : ∀ literal ∈ clause, truth literal = false) :
    scan truth clause = ⟨false, clause.length⟩ := by
  induction clause with
  | nil => rfl
  | cons literal rest ih =>
    have first := falseClause literal (by simp)
    have tail := ih (fun value member => falseClause value (List.mem_cons_of_mem _ member))
    simp [scan, first, tail]

/-- The exact work is the false prefix plus the satisfying occurrence; the
    remaining suffix contributes neither a query nor an acceptance obligation. -/
theorem first_true_after_prefix (truth : α → Bool) (before : List α) (literal : α)
    (after : List α) (falsePrefix : ∀ value ∈ before, truth value = false)
    (satisfied : truth literal = true) :
    scan truth (before ++ literal :: after) = ⟨true, before.length + 1⟩ := by
  induction before with
  | nil => simpa using first_true_stops truth literal after satisfied
  | cons first before ih =>
    have firstFalse := falsePrefix first (by simp)
    have tail := ih (fun value member => falsePrefix value (List.mem_cons_of_mem _ member))
    simp [scan, firstFalse, tail]

theorem nonempty_suffix_strictly_saves_work (truth : α → Bool) (before : List α)
    (literal : α) (after : List α) (falsePrefix : ∀ value ∈ before, truth value = false)
    (satisfied : truth literal = true) (remaining : after ≠ []) :
    (scan truth (before ++ literal :: after)).inspected <
      (before ++ literal :: after).length := by
  rw [first_true_after_prefix truth before literal after falsePrefix satisfied]
  cases after with
  | nil => contradiction
  | cons first rest => simp

/-- Short-circuiting one clause never authorizes skipping the following clauses. -/
def validate (truth : α → Bool) : List (List α) → Result
  | [] => ⟨true, 0⟩
  | clause :: clauses =>
      let current := scan truth clause
      if current.accepted then
        let rest := validate truth clauses
        ⟨rest.accepted, current.inspected + rest.inspected⟩
      else current

def occurrences : List (List α) → Nat
  | [] => 0
  | clause :: clauses => clause.length + occurrences clauses

theorem validate_accepts_iff (truth : α → Bool) (clauses : List (List α)) :
    (validate truth clauses).accepted = true ↔
      ∀ clause ∈ clauses, ∃ literal ∈ clause, truth literal = true := by
  induction clauses with
  | nil => simp [validate]
  | cons clause clauses ih =>
    cases h : (scan truth clause).accepted
    · have refused : ¬ ∃ literal ∈ clause, truth literal = true := by
        rw [← scan_accepts_iff, h]
        decide
      simp [validate, h, refused]
    · have accepted := (scan_accepts_iff truth clause).mp h
      simp [validate, h, ih, accepted]

theorem validate_work_bound (truth : α → Bool) (clauses : List (List α)) :
    (validate truth clauses).inspected ≤ occurrences clauses := by
  induction clauses with
  | nil => simp [validate, occurrences]
  | cons clause clauses ih =>
    have bound := scan_work_bound truth clause
    cases h : (scan truth clause).accepted <;> simp [validate, h, occurrences] <;> omega

theorem empty_clause_rejects (truth : α → Bool) (before after : List (List α)) :
    (validate truth (before ++ [] :: after)).accepted = false := by
  have rejected : ¬ (validate truth (before ++ [] :: after)).accepted = true := by
    rw [validate_accepts_iff]
    intro accepted
    simpa using accepted [] (by simp)
  cases h : (validate truth (before ++ [] :: after)).accepted
  · rfl
  · exact False.elim (rejected h)

end Zetesis.ClauseValidation
