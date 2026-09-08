import Zetesis.Core

/-!
# Replacement positions in a binary clause

Two distinct watched positions exhaust a binary clause. Consequently the
generic search for an unwatched available position returns no replacement,
independently of literal truth and the order of inspection. A binary watch visit
can therefore proceed directly to its existing unit/conflict decision.

Rust must separately maintain valid, distinct watch indices and perform the
unit/conflict decision correctly. These laws do not prove the watch registry,
propagation schedule, charged work, cancellation, allocation, CNF encoding or
reduct construction. They justify only omitting an empty replacement search.
-/

namespace Zetesis.BinaryWatch

/-- Two different valid positions cover every occurrence of a binary clause. -/
theorem positions_exhausted (first second : Fin 2) (distinct : first ≠ second)
    (position : Fin 2) : position = first ∨ position = second := by
  by_cases selected : position = first
  · exact Or.inl selected
  · have distinctValues : first.val ≠ second.val := by
      intro same
      exact distinct (Fin.ext same)
    have notFirst : position.val ≠ first.val := by
      intro same
      exact selected (Fin.ext same)
    have firstInside : first.val < 2 := first.isLt
    have secondInside : second.val < 2 := second.isLt
    have positionInside : position.val < 2 := position.isLt
    have matchesSecond : position.val = second.val := by omega
    exact Or.inr (Fin.ext matchesSecond)

/-- Generic replacement search ignores both watched positions before asking
    whether an occurrence is available under the current partial assignment. -/
def replacement (first second : Fin 2) (available : Fin 2 → Bool)
    (positions : List (Fin 2)) : Option (Fin 2) :=
  positions.find? (fun position =>
    decide (position ≠ first ∧ position ≠ second) && available position)

/-- With distinct binary watches, no ordering or availability can yield a
    replacement. Omitting this search preserves its complete logical result. -/
theorem replacement_absent (first second : Fin 2) (distinct : first ≠ second)
    (available : Fin 2 → Bool) (positions : List (Fin 2)) :
    replacement first second available positions = none := by
  unfold replacement
  apply List.find?_eq_none.mpr
  intro position member
  obtain selected | selected := positions_exhausted first second distinct position
  · simp [selected]
  · simp [selected]

end Zetesis.BinaryWatch
