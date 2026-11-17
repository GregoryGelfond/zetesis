import Zetesis.ObjectiveEligibility

/-!
# Reusing already selected source comparisons

The source-activity algebra combines finite literal activities with conjunction.
A completed comparison is required for a binding that its original join selected.
Replacing only those occurrences by required leaves the entire ordered activity
fold unchanged. Other literals remain live; optional source activity is not a
claim of a simultaneous model or an answer-set membership certificate.

The selection premise is explicit. Rust must establish it from the actual join,
unchanged source expressions and binding prefix. Canonical value admission,
resource receipts, cancellation checkpoints and diagnostic correspondence are
outside this pure law. In particular it does not assert equal interner histories.
The dependency is the three-valued activity algebra in `ObjectiveEligibility`.
This module proves the conditional fold law, not the Rust selection premise or
an end-to-end grounding refinement.
-/

namespace Zetesis.SelectedProducerComparisons

open ObjectiveEligibility

universe u
variable {Literal : Type u}

/-- Ordered finite conjunction of the activities read at literal occurrences. -/
def fold (read : Literal → Activity) : List Literal → Activity
  | [] => .required
  | literal :: rest => conjunction (read literal) (fold read rest)

/-- Only ordinary comparisons covered by the selected-row premise are reused. -/
def selectedRead (comparison : Literal → Bool) (read : Literal → Activity)
    (literal : Literal) : Activity :=
  if comparison literal then .required else read literal

/-- Reusing completed true comparisons preserves the complete activity fold.
The proof preserves the head's exact activity using the premise, then applies
induction to the remaining original occurrences. No short-circuit premise is
assumed for the rest of the body. -/
theorem selected_fold_exact (comparison : Literal → Bool)
    (read : Literal → Activity) (body : List Literal)
    (selected : ∀ literal ∈ body, comparison literal = true → read literal = .required) :
    fold (selectedRead comparison read) body = fold read body := by
  induction body with
  | nil => rfl
  | cons literal rest induction =>
    have head_same : selectedRead comparison read literal = read literal := by
      cases decided : comparison literal with
      | false => simp [selectedRead, decided]
      | true =>
        have required : read literal = .required := selected literal (by simp) decided
        simp [selectedRead, decided, required]
    have tail_selected : ∀ member ∈ rest,
        comparison member = true → read member = .required := by
      intro member present decided
      exact selected member (by simp [present]) decided
    have tail_same : fold (selectedRead comparison read) rest = fold read rest :=
      induction tail_selected
    simp only [fold, head_same, tail_same]

end Zetesis.SelectedProducerComparisons
