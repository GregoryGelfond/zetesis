import ReferenceEvents
import CheckedResults

open Aeneas Aeneas.Std Result Aeneas.Data.Coinductive
open ZetesisExtract RuntimeEffects

/-!
# The candidate retained by an eventful check

The public owned-candidate wrapper checks the candidate's own theory, retains
that exact candidate on a completed decision, and forwards a typed stop without
publishing a decision. This is operational composition; answer-set membership is
supplied by the public checker theorem, not by a stored verdict field alone.
-/
namespace RuntimeSubject

local instance : MonadLift Result Computation where
  monadLift := RuntimeEffects.embed

/-- The eventful owned wrapper obtains the candidate's own theory and maps only
the actual checker result. Its fixed setup consumes no runtime observation. -/
theorem subject_exact (candidate : theory.Interpretation) (limits : oracle.Limits)
    (control : zetesis_cpu.cancellation.Cancellation) :
    ReferenceEvents.checkInterpretation candidate limits control =
      ITree.bind (ReferenceEvents.check candidate.theory candidate limits control)
        (fun answer => ITree.ret (match answer with
          | .Ok checked => .Ok { candidate, check := checked }
          | .Err reason => .Err reason)) := by
  unfold ReferenceEvents.checkInterpretation CheckerContexts.checkInterpretation
  simp only [theory.Interpretation.impl.theory, ContextEvents.lift_result,
    embed_ok, Bind.bind, itree_ret_bind]
  congr 1
  funext answer
  cases answer with
  | Ok checked =>
      simp [core.result.Result.Insts.CoreOpsTry.branch, embed_ok, itree_ret_bind]
  | Err reason =>
      simp [core.result.Result.Insts.CoreOpsTry.branch,
        core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual,
        core.convert.FromSame.from, embed_ok, itree_ret_bind]

/-- A completed owned decision retains the exact input and recovers its actual
checker run, consuming precisely the same observations. -/
theorem completed_subject (candidate : theory.Interpretation) (limits : oracle.Limits)
    (control : zetesis_cpu.cancellation.Cancellation) (events : List Event)
    (decision : checked.CheckedInterpretation)
    (run : Runs (ReferenceEvents.checkInterpretation candidate limits control) events (.Ok decision)) :
    decision.candidate = candidate ∧
      Runs (ReferenceEvents.check candidate.theory candidate limits control) events (.Ok decision.check) := by
  rw [subject_exact] at run
  obtain ⟨before, after, answer, partition, checked, following⟩ :=
    RuntimeRuns.bind_inv _ _ _ _ run
  obtain ⟨same, silent⟩ := RuntimeRuns.returned_inv _ _ _ following
  have exactEvents : events = before := by simpa only [silent, List.append_nil] using partition
  rw [exactEvents]
  cases answer with
  | Err reason => cases same
  | Ok result =>
      have retained : {candidate := candidate, check := result} = decision :=
        core.result.Result.Ok.inj same
      cases retained
      exact ⟨rfl, checked⟩

/-- A typed refusal is exactly a refusal of the actual own-theory checker. The
wrapper adds no observation and cannot attach a candidate decision to that stop. -/
theorem refused_subject (candidate : theory.Interpretation) (limits : oracle.Limits)
    (control : zetesis_cpu.cancellation.Cancellation) (events : List Event)
    (reason : zetesis_cpu.cancellation.Stop) :
    Runs (ReferenceEvents.checkInterpretation candidate limits control) events (.Err reason) ↔
      Runs (ReferenceEvents.check candidate.theory candidate limits control) events (.Err reason) := by
  rw [subject_exact]
  constructor
  · intro run
    obtain ⟨before, after, answer, partition, checked, following⟩ :=
      RuntimeRuns.bind_inv _ _ _ _ run
    obtain ⟨same, silent⟩ := RuntimeRuns.returned_inv _ _ _ following
    have exactEvents : events = before := by simpa only [silent, List.append_nil] using partition
    rw [exactEvents]
    cases answer with
    | Ok result => cases same
    | Err stop =>
        have retained : stop = reason := core.result.Result.Err.inj same
        subst stop
        exact checked
  · intro run
    have complete := RuntimeEffects.runs_bind (next := fun answer => ITree.ret (match answer with
      | .Ok result => core.result.Result.Ok ({ candidate, check := result } : checked.CheckedInterpretation)
      | .Err stop => .Err stop)) run (Runs.returned (core.result.Result.Err reason))
    simpa only [List.append_nil] using complete

end RuntimeSubject
