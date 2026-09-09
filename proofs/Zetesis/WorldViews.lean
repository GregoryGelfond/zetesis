import Zetesis.Outcomes
import Zetesis.Optimization

/-!
# Complete original answer families

A world view is the family of every answer set of one original theory.
Membership receipts establish soundness; a complete candidate ledger and complete
capture establish the separate coverage obligation. Optimal answers are a
selection from that family, and need not represent the original world view.

These laws compose the existing ledger and reduct semantics. They characterize
finite collections extensionally: order and duplicate-free representation remain
separate requirements. Subject-token identity, actual session transitions,
allocation limits, scoring and the Rust collector are not verified here.
-/

namespace Zetesis.WorldViews
open Ferraris
universe u
variable {A : Type u}

/-- A finite list represents exactly the original theory's answer family.
    This says nothing about display projection, list order or duplicates. -/
def Represents (T : Theory A) (answers : List (Atoms A)) : Prop :=
  ∀ M, M ∈ answers ↔ Stable M T

/-- Capture has retained every accepted interpretation from the same ledger.
    A stream prefix only establishes the forward implication. -/
def Captures (s : BatchAccounting.Ledger (Atoms A))
    (answers : List (Atoms A)) : Prop :=
  ∀ M, M ∈ answers ↔ M ∈ s.accepted

/-- Complete original candidate coverage, exact classification and full capture
    establish the world view. Exhaustion of an objective-selected region cannot
    supply the original-coverage premise.

    The proof separates membership soundness from coverage: the former follows
    a retained receipt back to the ledger; the latter follows an arbitrary
    original answer through the carrier, completed ledger and capture.
-/
theorem completed_capture_represents_world_view (T : Theory A)
    (carrier : List (Atoms A)) (s : BatchAccounting.Ledger (Atoms A))
    (answers : List (Atoms A))
    (original : ∀ M, Stable M T → M ∈ carrier)
    (covered : BatchAccounting.Covers carrier s)
    (sound : BatchAccounting.Sound (fun M => Stable M T) s)
    (done : BatchAccounting.Complete s) (captured : Captures s answers) :
    Represents T answers := by
  have membership (M : Atoms A) :
      M ∈ s.accepted ↔ M ∈ carrier ∧ Stable M T :=
    BatchAccounting.completed_results_exact carrier s (fun N => Stable N T)
      covered sound done M
  have retained_sound (M : Atoms A) (retained : M ∈ answers) : Stable M T := by
    have accepted : M ∈ s.accepted := (captured M).mp retained
    exact ((membership M).mp accepted).2
  have retained_complete (M : Atoms A) (answer : Stable M T) : M ∈ answers := by
    have inside : M ∈ carrier := original M answer
    have accepted : M ∈ s.accepted := (membership M).mpr ⟨inside, answer⟩
    exact (captured M).mpr accepted
  show ∀ M, M ∈ answers ↔ Stable M T
  intro M
  exact ⟨retained_sound M, retained_complete M⟩

/-- An interrupted or bounded capture retains valid membership evidence even
    when it cannot supply a complete world view. -/
theorem partial_capture_has_only_answer_sets (T : Theory A)
    (s : BatchAccounting.Ledger (Atoms A)) (answers : List (Atoms A))
    (sound : BatchAccounting.Sound (fun M => Stable M T) s)
    (captured : Outcomes.DeliverySound s answers) :
    ∀ M, M ∈ answers → Stable M T :=
  Outcomes.delivered_values_are_valid (fun M => Stable M T) s answers sound captured

/-- An empty complete family means inconsistency. The singleton containing an
    empty interpretation is nonempty and therefore has a different meaning. -/
theorem world_view_empty_iff_inconsistent (T : Theory A) (answers : List (Atoms A))
    (exact : Represents T answers) :
    answers = [] ↔ ∀ M, ¬ Stable M T := by
  constructor
  · intro empty M answer
    have retained : M ∈ answers := (exact M).mpr answer
    exact List.not_mem_nil (empty ▸ retained)
  · intro inconsistent
    apply List.eq_nil_iff_forall_not_mem.mpr
    intro M retained
    exact inconsistent M ((exact M).mp retained)

/-- One missing original answer refutes a completeness claim, even if every
    retained interpretation has passed membership checking. -/
theorem missing_answer_prevents_world_view (T : Theory A)
    (answers : List (Atoms A)) (M : Atoms A)
    (answer : Stable M T) (missing : M ∉ answers) : ¬ Represents T answers := by
  intro exact
  exact missing ((exact M).mpr answer)

/-- List reordering or duplicate removal preserves the represented family when
    it preserves membership of full interpretations. Display equality alone
    cannot supply this premise. -/
theorem same_members_preserve_world_view (T : Theory A)
    (first second : List (Atoms A))
    (same : ∀ M, M ∈ first ↔ M ∈ second) (exact : Represents T first) :
    Represents T second := by
  intro M
  exact (same M).symm.trans (exact M)

/-- A complete family supports exact optimal-tie selection. Numeric ranking is
    applied after membership and does not change the original answer predicate. -/
theorem world_view_ties_are_exact (T : Theory A) (cost : Atoms A → Int)
    (answers : List (Atoms A)) (exact : Represents T answers) (M : Atoms A) :
    M ∈ Optimization.bestTies cost answers ↔ Optimization.Optimal T cost M :=
  Optimization.completed_ties_exact T cost answers
    (fun N retained => (exact N).mp retained)
    (fun N answer => (exact N).mpr answer) M

/-- Whenever one answer is strictly worse than another, a complete collection
    of optimal answers omits an original answer. Proved optimization is thus
    insufficient for constructing the unrestricted world view. -/
theorem optimal_family_omits_worse_answer (T : Theory A) (cost : Atoms A → Int)
    (selected : List (Atoms A))
    (only_optimal : ∀ M, M ∈ selected → Optimization.Optimal T cost M)
    (better worse : Atoms A) (better_answer : Stable better T)
    (worse_answer : Stable worse T) (improves : cost better < cost worse) :
    ¬ Represents T selected := by
  have missing : worse ∉ selected := by
    intro retained
    have minimum : cost worse ≤ cost better :=
      (only_optimal worse retained).2 better better_answer
    exact (Int.not_le_of_gt improves) minimum
  exact missing_answer_prevents_world_view T selected worse worse_answer missing

end Zetesis.WorldViews
