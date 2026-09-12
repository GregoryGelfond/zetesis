import Zetesis.TightPlans
import Zetesis.BatchAccounting

/-!
# Checked support followed by exact residual completion

The original theory stays fixed when candidate restrictions change. Ranked
support supplies a sufficient certificate; failed support requests exact
completion, and an interrupted attempt supplies no membership result. Completed
classifications can therefore use the existing batch-commit soundness law.

These laws state semantic composition and cumulative natural-number quotas.
They do not refine Rust counters, allocation, formula extraction, scheduling,
source coverage or output delivery. The existing TightPlans module proves the
ranked-support premise used here. Actual machine work remains separately tested.
-/
namespace Zetesis.CertifiedExecution
universe u
variable {α : Type u}
open Ferraris TightPlans

inductive Verdict where
  | stable
  | notModel
  | residual
  | stopped

def Sound (T : Theory α) (M : Atoms α) : Verdict → Prop
  | .stable => Stable M T
  | .notModel => ¬ Models M T
  | .residual | .stopped => True

def complete : Verdict → Option Bool → Option Bool
  | .stable, _ => some true
  | .notModel, _ => some false
  | .residual, exact => exact
  | .stopped, _ => none

def supportVerdict (original supported : Bool) : Verdict :=
  if original then if supported then .stable else .residual else .notModel

theorem ranked_support_verdict_sound (T : Theory α) (M : Atoms α)
    (rules : List (Producer α)) (rank : α → Nat) (original supported : Bool)
    (roots : OriginalProducers T rules) (ranked : Ranked rank rules)
    (model : original = true ↔ Models M T)
    (support : supported = true → Supported M rules) :
    Sound T M (supportVerdict original supported) := by
  cases original with
  | false =>
    simp only [supportVerdict, Bool.false_eq_true, ↓reduceIte, Sound]
    intro h
    have impossible := model.mpr h
    cases impossible
  | true =>
    cases supported with
    | false => trivial
    | true =>
      exact ranked_support_stable M T rules rank roots ranked (model.mp rfl) (support rfl)

/-- A completed classification agrees with stability in the original theory.

The four scoped obligations use certificate soundness, original-model necessity,
exact residual completion, and the impossibility of stopped completion. The final
case split composes them; no termination or successful completion is asserted.
-/
-- ANCHOR: completed_membership_exact
theorem completed_membership_exact (T : Theory α) (M : Atoms α)
    (verdict : Verdict) (exact : Option Bool) (result : Bool)
    (sound : Sound T M verdict)
    (oracle : ∀ b, exact = some b → (b = true ↔ Stable M T))
    (done : complete verdict exact = some result) : result = true ↔ Stable M T := by
  have certified_case (certified : verdict = .stable) : result = true ↔ Stable M T := by
    have accepted : result = true := by simpa [certified, complete] using done.symm
    have stable_model : Stable M T := by simpa only [certified, Sound] using sound
    exact ⟨fun _ => stable_model, fun _ => accepted⟩

  have rejected_case (rejected : verdict = .notModel) : result = true ↔ Stable M T := by
    have not_accepted : result = false := by simpa [rejected, complete] using done.symm
    have not_original_model : ¬ Models M T := by simpa only [rejected, Sound] using sound
    have not_stable : ¬ Stable M T := by
      intro stable_model
      exact not_original_model stable_model.1
    simpa [not_accepted] using not_stable

  have residual_case (residual : verdict = .residual) : result = true ↔ Stable M T := by
    have exact_result : exact = some result := by simpa only [residual, complete] using done
    exact oracle result exact_result

  have stopped_case (stopped : verdict = .stopped) : False := by
    have no_completed_result : complete .stopped exact ≠ some result := by simp [complete]
    exact no_completed_result (stopped ▸ done)

  -- QED: the four verdict cases exhaust every completed classification.
  show result = true ↔ Stable M T
  cases verdict with
  | stable => exact certified_case rfl
  | notModel => exact rejected_case rfl
  | residual => exact residual_case rfl
  | stopped => exact False.elim (stopped_case rfl)
-- ANCHOR_END: completed_membership_exact

theorem interruption_cannot_accept (exact : Option Bool) :
    complete .stopped exact ≠ some true := by simp [complete]

theorem unsupported_requires_completion (exact : Option Bool) :
    complete (supportVerdict true false) exact = exact := by rfl

/-- A failed optional attempt still consumes quota before exact completion. -/
theorem fallback_quota_is_cumulative (limit prior attempted residual : Nat)
    (before : prior ≤ limit) (attempt : attempted ≤ limit - prior)
    (finish : residual ≤ limit - (prior + attempted)) :
    prior + attempted + residual ≤ limit := by omega

/-- Candidate restrictions filter membership results; they do not replace T. -/
theorem restricted_result_original (T : Theory α) (M : Atoms α)
    (region : Atoms α → Prop) (inside : region M)
    (verdict : Verdict) (exact : Option Bool)
    (sound : Sound T M verdict)
    (oracle : ∀ b, exact = some b → (b = true ↔ Stable M T))
    (accepted : complete verdict exact = some true) : region M ∧ Stable M T := by
  exact ⟨inside, (completed_membership_exact T M verdict exact true sound oracle accepted).mp rfl⟩

end Zetesis.CertifiedExecution
