import Zetesis.Core

/-!
# Replacement positions in a ternary clause

Two distinct watched positions leave exactly one position in a three-position
clause. Its index is `3 - first.val - second.val`. A generic search that includes
this position therefore has the same result as testing its availability once,
regardless of inspection order, repetitions or the other positions' availability.

These laws concern finite positions and a fixed Boolean availability predicate.
Rust must separately establish valid distinct watches and preserve the current
assignment while testing availability. Watch storage, replacement updates,
charged work, cancellation, allocation, CNF encoding and reduct construction
remain separate implementation obligations.
-/

namespace Zetesis.TernaryWatch

/-- The third position, computed from the two distinct watched indices.
Their values lie between zero and two. Distinctness bounds their sum between
one and three. Neither subtraction underflows, and the final index is below
three. -/
def remaining (first second : Fin 3) (distinct : first ≠ second) : Fin 3 :=
  ⟨3 - first.val - second.val, by
    have different : first.val ≠ second.val := by
      intro same
      exact distinct (Fin.ext same)
    have firstInside : first.val < 3 := first.isLt
    have secondInside : second.val < 3 := second.isLt
    omega⟩

/-- A position differs from both watches exactly when it is the computed third
position. The forward direction uses the three index bounds and the three
pairwise distinctions. The reverse direction substitutes the computed value;
equality with either watch would violate its distinction from the other. -/
theorem remaining_unique (first second : Fin 3) (distinct : first ≠ second)
    (position : Fin 3) :
    (position ≠ first ∧ position ≠ second) ↔ position = remaining first second distinct := by
  have different : first.val ≠ second.val := by
    intro same
    exact distinct (Fin.ext same)
  have firstInside : first.val < 3 := first.isLt
  have secondInside : second.val < 3 := second.isLt
  have positionInside : position.val < 3 := position.isLt
  constructor
  · rintro ⟨notFirst, notSecond⟩
    have notFirstValue : position.val ≠ first.val := by
      intro same
      exact notFirst (Fin.ext same)
    have notSecondValue : position.val ≠ second.val := by
      intro same
      exact notSecond (Fin.ext same)
    have computed : position.val = 3 - first.val - second.val := by omega
    exact Fin.ext computed
  · intro selected
    have computed : position.val = 3 - first.val - second.val :=
      congrArg Fin.val selected
    constructor
    · intro same
      have sameValue : position.val = first.val := congrArg Fin.val same
      omega
    · intro same
      have sameValue : position.val = second.val := congrArg Fin.val same
      omega

/-- Generic replacement search ignores both watches before testing availability. -/
def replacement (first second : Fin 3) (available : Fin 3 → Bool)
    (positions : List (Fin 3)) : Option (Fin 3) :=
  positions.find? (fun position =>
    decide (position ≠ first ∧ position ≠ second) && available position)

/-- If the inspected list includes the third position, replacement search equals
one availability test there. Other positions are ineligible by uniqueness. An
induction over the list skips those positions until the third is encountered;
if it is unavailable, every remaining occurrence is ineligible as well. -/
theorem replacement_exact (first second : Fin 3) (distinct : first ≠ second)
    (available : Fin 3 → Bool) (positions : List (Fin 3))
    (covered : remaining first second distinct ∈ positions) :
    replacement first second available positions =
      if available (remaining first second distinct) then
        some (remaining first second distinct)
      else none := by
  have predicateExact (position : Fin 3) :
      (decide (position ≠ first ∧ position ≠ second) && available position) =
        (decide (position = remaining first second distinct) &&
          available (remaining first second distinct)) := by
    by_cases selected : position = remaining first second distinct
    · have unwatched : position ≠ first ∧ position ≠ second :=
        (remaining_unique first second distinct position).mpr selected
      rw [decide_eq_true unwatched, Bool.true_and]
      simp [selected]
    · have watched : ¬ (position ≠ first ∧ position ≠ second) := by
        intro unwatched
        exact selected ((remaining_unique first second distinct position).mp unwatched)
      simp [watched, selected]
  have unavailable (positions : List (Fin 3))
      (absent : available (remaining first second distinct) = false) :
      replacement first second available positions = none := by
    unfold replacement
    apply List.find?_eq_none.mpr
    intro position member
    rw [predicateExact]
    simp [absent]
  cases availability : available (remaining first second distinct) with
  | false => simpa only [availability, Bool.false_eq_true, ↓reduceIte] using
      unavailable positions availability
  | true =>
    have found : replacement first second available positions =
        some (remaining first second distinct) := by
      revert covered
      induction positions with
      | nil =>
        intro covered
        simp at covered
      | cons position tail inductionHypothesis =>
        intro covered
        by_cases selected : position = remaining first second distinct
        · unfold replacement
          rw [List.find?_cons, predicateExact]
          simp [selected, availability]
        · have inTail : remaining first second distinct ∈ tail := by
            simpa only [List.mem_cons, Ne.symm selected, false_or] using covered
          unfold replacement
          rw [List.find?_cons, predicateExact]
          simpa only [replacement, selected, decide_false, Bool.false_and] using
            inductionHypothesis inTail
    simpa only [availability, ↓reduceIte] using found

end Zetesis.TernaryWatch
