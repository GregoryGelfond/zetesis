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

theorem completed_membership_exact (T : Theory α) (M : Atoms α)
    (verdict : Verdict) (exact : Option Bool) (result : Bool)
    (sound : Sound T M verdict)
    (oracle : ∀ b, exact = some b → (b = true ↔ Stable M T))
    (done : complete verdict exact = some result) : result = true ↔ Stable M T := by
  cases verdict with
  | stable =>
    have h : result = true := by simpa [complete] using done.symm
    subst result
    exact ⟨fun _ => sound, fun _ => rfl⟩
  | notModel =>
    have h : result = false := by simpa [complete] using done.symm
    subst result
    constructor
    · intro h; cases h
    · intro stable; exact False.elim (sound stable.1)
  | residual => exact oracle result done
  | stopped => simp [complete] at done

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
