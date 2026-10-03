import Control

open Aeneas Aeneas.Std Result
open ZetesisExtract

/-!
# Work charged by a tight-plan inspection

The extracted tick polls first, then tests equality with its allowance, then
increments the counter. Its invariant is `used ≤ max`; it is not assumed to
recover malformed counters beyond that allowance. These laws concern the
supplied per-call observation model. They do not model changing runtime reads.
-/
namespace TightWork

/-- A control stop precedes the work check and preserves the complete record. -/
theorem stopped (work : tight.Work) (reason : zetesis_cpu.cancellation.Stop)
    (observed : EvaluatorControl.observation work.cancellation = some reason) :
    tight.Work.tick work = ok (.Err (.Stopped reason), work) := by
  simp [tight.Work.tick, EvaluatorControl.poll_exact, observed,
    core.result.Result.Insts.CoreOpsTry.branch,
    core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual,
    tight.TightError.Insts.CoreConvertFromStop.from]

/-- Reaching the allowance refuses the next inspection without charging it. -/
theorem at_limit (work : tight.Work)
    (clear : EvaluatorControl.observation work.cancellation = none)
    (exhausted : work.used = work.max) :
    tight.Work.tick work = ok (.Err (.Limit .Work), work) := by
  simp [tight.Work.tick, EvaluatorControl.poll_exact, clear, exhausted,
    core.result.Result.Insts.CoreOpsTry.branch]

/-- An allowed inspection increments exactly once. The stored limit supplies
the bound needed by machine addition; all other fields remain unchanged. -/
theorem advances (work : tight.Work)
    (clear : EvaluatorControl.observation work.cancellation = none)
    (remaining : work.used.val < work.max.val) :
    ∃ next : U64, next.val = work.used.val + 1 ∧
      tight.Work.tick work = ok (.Ok (), { work with used := next }) := by
  have fits : work.used.val + (1#u64).val ≤ U64.max := by
    have maximum := U64.le_max work.max
    simp only [U64.max_eq, UScalar.ofNatCore_val_eq] at *
    omega
  obtain ⟨next, added, incremented⟩ := WP.spec_imp_exists
    (U64.add_spec (x := work.used) (y := 1#u64) fits)
  have distinct : work.used ≠ work.max := by
    intro same
    rw [same] at remaining
    exact Nat.lt_irrefl _ remaining
  refine ⟨next, by simpa using incremented, ?_⟩
  simp [tight.Work.tick, EvaluatorControl.poll_exact, clear, distinct, added,
    core.result.Result.Insts.CoreOpsTry.branch]

/-- Each valid tick either preserves its record with the reached typed refusal,
or advances once while preserving its allowance and control. This is a result
of the actual tick, not a caller-supplied transition agreement. -/
theorem refines (work : tight.Work) (bounded : work.used.val ≤ work.max.val) :
    (∃ reason : tight.TightError,
      tight.Work.tick work = ok (.Err reason, work)) ∨
    ∃ next : U64, next.val = work.used.val + 1 ∧ next.val ≤ work.max.val ∧
      tight.Work.tick work = ok (.Ok (), { work with used := next }) := by
  cases observed : EvaluatorControl.observation work.cancellation with
  | some reason => exact Or.inl ⟨.Stopped reason, stopped work reason observed⟩
  | none =>
    by_cases reached : work.used = work.max
    · exact Or.inl ⟨.Limit .Work, at_limit work observed reached⟩
    · have remaining : work.used.val < work.max.val := by
        have different : work.used.val ≠ work.max.val := by
          intro equal
          exact reached (UScalar.eq_of_val_eq equal)
        omega
      obtain ⟨next, increment, actual⟩ := advances work observed remaining
      exact Or.inr ⟨next, increment, by omega, actual⟩

end TightWork
