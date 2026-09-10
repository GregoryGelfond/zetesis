import Zetesis.ObjectiveDirections

/-!
# Numeric selection from resolved objective values

The numeric projection of a resolved logical weight is either an integer or
absent. Nonnumeric values contribute neither a key nor a priority witness;
numeric zero remains a real contribution. Selection precedes direction
normalization and global complete-key coalescing. Completed grounding carriers
and model-relative activation remain separate inputs.

These laws justify numeric selection for already-resolved finite entries. They
do not reconstruct clingo's grounding carrier, prove Rust value conversion,
certify arithmetic bounds, or change the original Ferraris reduct subject.
-/

namespace Zetesis.ObjectiveValues

open ObjectiveDirections

universe u v w
variable {σ : Type u} {χ : Type v} {α : Type w}

/-- Numeric projection preserves absence rather than converting ignored logical
    weights to a zero contribution. The default priority is an integer; a prior
    selection stage may retain a different resolved priority representation. -/
structure ResolvedEntry (σ : Type u) (χ : Type v) (π : Type := Int) where
  direction : Direction
  weight : Option Int
  priority : π
  tuple : List σ
  condition : χ

def numeric (entry : ResolvedEntry σ χ) : Option (Entry σ χ) :=
  entry.weight.map fun weight =>
    ⟨entry.direction, weight, entry.priority, entry.tuple, entry.condition⟩

def numericEntries (entries : List (ResolvedEntry σ χ)) : List (Entry σ χ) :=
  entries.filterMap numeric

/-- Completed retained rows witness priority presence before any model is
    selected. This definition does not inspect active or optimal models. -/
def Present (priority : Int) (rows : List (ResolvedEntry σ χ)) : Prop :=
  ∃ entry, entry ∈ rows ∧ entry.priority = priority ∧ entry.weight.isSome = true

theorem ignored_entry_has_no_numeric_key (entry : ResolvedEntry σ χ)
    (ignored : entry.weight = none) : numeric entry = none := by
  simp [numeric, ignored]

/-- Deleting one ignored entry preserves every numeric entry, including zero
    weights and duplicate complete keys whose activation is coalesced later. -/
theorem numeric_entries_ignore (entry : ResolvedEntry σ χ)
    (rest : List (ResolvedEntry σ χ)) (ignored : entry.weight = none) :
    numericEntries (entry :: rest) = numericEntries rest := by
  simp [numericEntries, ignored_entry_has_no_numeric_key entry ignored]

theorem ignored_entry_preserves_presence (priority : Int)
    (entry : ResolvedEntry σ χ) (rest : List (ResolvedEntry σ χ))
    (ignored : entry.weight = none) :
    Present priority (entry :: rest) ↔ Present priority rest := by
  constructor
  · rintro ⟨witness, member, same, numeric⟩
    rcases List.mem_cons.mp member with equal | retained
    · subst witness
      simp [ignored] at numeric
    · exact ⟨witness, retained, same, numeric⟩
  · rintro ⟨witness, member, same, numeric⟩
    exact ⟨witness, List.mem_cons_of_mem entry member, same, numeric⟩

/-- Zero witnesses presence even if its activation is false in a particular
    model. Presence belongs to the completed retained carrier. -/
theorem zero_entry_retains_presence (entry : ResolvedEntry σ χ)
    (rows : List (ResolvedEntry σ χ)) (member : entry ∈ rows)
    (zero : entry.weight = some 0) : Present entry.priority rows := by
  have numeric : entry.weight.isSome = true := by simp [zero]
  exact ⟨entry, member, rfl, numeric⟩

/-- Numeric selection retains the original complete key before existing
    direction normalization and global duplicate elimination. -/
theorem selected_entry_preserves_key (entry : ResolvedEntry σ χ) (weight : Int)
    (selected : entry.weight = some weight) :
    (numeric entry).map normalizedKey =
      some (normalizedKey (⟨entry.direction, weight, entry.priority,
        entry.tuple, entry.condition⟩ : Entry σ χ)) := by
  simp [numeric, selected]

theorem ignored_entry_preserves_cost [DecidableEq σ] (priority : Int)
    (truth : χ → Bool) (entry : ResolvedEntry σ χ)
    (rest : List (ResolvedEntry σ χ)) (ignored : entry.weight = none) :
    mixedCost priority truth (numericEntries (entry :: rest)) =
      mixedCost priority truth (numericEntries rest) := by
  rw [numeric_entries_ignore entry rest ignored]

/-- The same immutable theory and same cost function determine the same optimal
    stable models. No ignored objective entry supplies reduct support. -/
theorem ignored_entry_preserves_optima [DecidableEq σ]
    (theory : Ferraris.Theory α) (priorities : List Int)
    (truth : Atoms α → χ → Bool) (entry : ResolvedEntry σ χ)
    (rest : List (ResolvedEntry σ χ)) (ignored : entry.weight = none)
    (model : Atoms α) :
    Optimal theory (fun candidate => mixedVector priorities (truth candidate)
      (numericEntries (entry :: rest))) model ↔
    Optimal theory (fun candidate => mixedVector priorities (truth candidate)
      (numericEntries rest)) model := by
  rw [numeric_entries_ignore entry rest ignored]

end Zetesis.ObjectiveValues
