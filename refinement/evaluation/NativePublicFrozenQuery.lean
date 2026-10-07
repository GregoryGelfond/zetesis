import NativeFrozenConstruction
import NativeWorkInitialization
import NativeFrozenQuery
import NativeOwnerChecks
import VectorReservation

open Aeneas Aeneas.Std Result
open ZetesisNativeExtract

/-!
# The public frozen-reduct query

The generated public query checks theory identity, polls control, reserves a
workspace and invokes the proved private query with a fresh work counter. These
proofs recover that execution from an actual completed return. No subset or
original-model premise is imposed on the tested interpretation or candidate.

The supplied reservation operation describes this invocation only. The existing
fixed observation model is retained; changing runtime histories and the complete
public answer-set checker remain separate obligations.
-/
namespace NativePublicFrozenQuery

variable [reservation : VectorReservation]


/-- A foreign interpretation is refused before control or storage preparation,
even if its theory has equal formula contents. -/
theorem wrong_owner (frozen : reduct.FrozenReduct) (tested : theory.Interpretation)
    (limits : oracle.Limits) (cancellation : zetesis_cpu.cancellation.Cancellation)
    (different : frozen.candidate.theory.owner ≠ tested.theory.owner) :
    reduct.FrozenReduct.is_satisfied_by frozen tested limits cancellation =
      ok (.Err .WrongProgram) := by
  simp [reduct.FrozenReduct.is_satisfied_by, reduct.FrozenReduct.theory,
    theory.Interpretation.impl.theory,
    NativeOwnerChecks.identities_reject frozen.candidate.theory tested different,
    core.result.Result.Insts.CoreOpsTry.branch,
    core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]

/-- With matching ownership and a clear initial poll, the wrapper performs the
actual reservation and private query in order. The bind retains typed refusal,
backend failure and divergence; this equation asserts no completion. -/
theorem prepared (frozen : reduct.FrozenReduct) (tested : theory.Interpretation)
    (limits : oracle.Limits) (cancellation : zetesis_cpu.cancellation.Cancellation)
    (same : frozen.candidate.theory.owner = tested.theory.owner)
    (clear : NativeControl.observation cancellation = none) :
    reduct.FrozenReduct.is_satisfied_by frozen tested limits cancellation =
      (do
        let prepared ← oracle.reserve Bool frozen.truth.len
        match prepared with
        | .Err reason => ok (.Err reason)
        | .Ok values =>
            let (answer, _, _) ← reduct.FrozenReduct.satisfied_by frozen tested values
              (NativeWorkInitialization.initial limits cancellation)
            ok answer) := by
  have identity : oracle.identities frozen.candidate.theory tested = ok (.Ok ()) := by
    exact (NativeOwnerChecks.identities_accept_iff frozen.candidate.theory tested).mpr same
  simp only [reduct.FrozenReduct.is_satisfied_by, reduct.FrozenReduct.theory,
    theory.Interpretation.impl.theory, bind_tc_ok, identity,
    core.result.Result.Insts.CoreOpsTry.branch, NativeControl.poll_exact, clear,
    NativeWorkInitialization.statistics_default,
    NativeWorkInitialization.initial]
  apply congrArg (fun continuation => oracle.reserve Bool frozen.truth.len >>= continuation)
  funext result
  cases result <;>
    simp [core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]

/-- A completed public query derives its successful owner check, initial poll,
reservation and private query. Inner-call equations are conclusions, not caller
premises. No condition on the reserved vector's old contents is required. -/
theorem completed_phases (frozen : reduct.FrozenReduct) (tested : theory.Interpretation)
    (limits : oracle.Limits) (cancellation : zetesis_cpu.cancellation.Cancellation)
    (answer : Bool)
    (completed : reduct.FrozenReduct.is_satisfied_by frozen tested limits cancellation =
      ok (.Ok answer)) :
    frozen.candidate.theory.owner = tested.theory.owner ∧
      NativeControl.observation cancellation = none ∧
      ∃ old output after,
        oracle.reserve Bool frozen.truth.len = ok (.Ok old) ∧
        reduct.FrozenReduct.satisfied_by frozen tested old (NativeWorkInitialization.initial limits cancellation) =
          ok (.Ok answer, output, after) := by
  have same : frozen.candidate.theory.owner = tested.theory.owner := by
    by_contra different
    rw [wrong_owner frozen tested limits cancellation different] at completed
    simp at completed
  have clear : NativeControl.observation cancellation = none := by
    cases observed : NativeControl.observation cancellation with
    | none => rfl
    | some reason =>
        have identity : oracle.identities frozen.candidate.theory tested = ok (.Ok ()) := by
          exact (NativeOwnerChecks.identities_accept_iff frozen.candidate.theory tested).mpr same
        simp [reduct.FrozenReduct.is_satisfied_by, reduct.FrozenReduct.theory,
          theory.Interpretation.impl.theory, identity, NativeControl.poll_exact, observed,
          core.result.Result.Insts.CoreOpsTry.branch,
          core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]
          at completed
  rw [prepared frozen tested limits cancellation same clear] at completed
  have phases : ∃ old output after,
      oracle.reserve Bool frozen.truth.len = ok (.Ok old) ∧
      reduct.FrozenReduct.satisfied_by frozen tested old (NativeWorkInitialization.initial limits cancellation) =
        ok (.Ok answer, output, after) := by
    cases reserved : oracle.reserve Bool frozen.truth.len with
    | ret result =>
        cases result with
        | Err reason => simp [reserved] at completed
        | Ok old =>
            simp only [reserved, bind_tc_ok] at completed
            cases queried : reduct.FrozenReduct.satisfied_by frozen tested old
                (NativeWorkInitialization.initial limits cancellation) with
            | ret result =>
                rcases result with ⟨actual, output, after⟩
                have sameAnswer : actual = .Ok answer := by
                  simpa [queried] using completed
                exact ⟨old, output, after, rfl, by simpa only [sameAnswer] using queried⟩
            | vis error continuation => simp [queried] at completed
            | div => simp [queried] at completed
    | vis error continuation => simp [reserved] at completed
    | div => simp [reserved] at completed
  exact ⟨same, clear, phases⟩

/-- A completed public query decides satisfaction of the Ferraris reduct fixed
by the stored candidate. Its actual return supplies every setup and query call;
the previously proved private-query theorem supplies their semantic meaning. -/
theorem completed_satisfaction (frozen : reduct.FrozenReduct)
    (tested : theory.Interpretation) (limits : oracle.Limits)
    (cancellation : zetesis_cpu.cancellation.Cancellation)
    (represented : NativeFrozenQuery.Represents frozen) (stored : NativeMembership.Represented tested)
    (valid : NativeStructure.WellFormed frozen.candidate.theory.value.atoms.val
      (NativeExecution.view frozen.candidate.theory))
    (rootsBounded : ∀ root ∈ frozen.candidate.theory.value.roots.val,
      root.val < (NativeTable.rows (NativeExecution.view frozen.candidate.theory)).length)
    (answer : Bool)
    (completed : reduct.FrozenReduct.is_satisfied_by frozen tested limits cancellation =
      ok (.Ok answer)) :
    answer = true ↔ Zetesis.Ferraris.Models
      (Zetesis.TightEvaluation.interpretation (NativeMembership.denotes tested))
      (Zetesis.Ferraris.ReductTheory
        (Zetesis.TightEvaluation.interpretation (NativeMembership.denotes frozen.candidate))
        (NativeRootSemantics.assertions frozen.candidate.theory)) := by
  obtain ⟨_, _, old, output, after, _, queried⟩ :=
    completed_phases frozen tested limits cancellation answer completed
  exact NativeFrozenQuery.completed_satisfaction frozen tested old (NativeWorkInitialization.initial limits cancellation)
    represented valid stored rootsBounded answer output after queried

/-- Constructing and then querying a reduct computes the Ferraris satisfaction
relation for the original supplied candidate. The mask invariant and retained
candidate are derived from actual construction, rather than assumed separately.

Construction and query receive separate reservation providers, limits and
control observations. This composes their per-invocation calls; it
does not model an allocator or concurrent observation history. Neither original
modelhood of the candidate nor a tested-subset premise is required. -/
theorem constructed_satisfaction (construction : VectorReservation)
    (candidate tested : theory.Interpretation)
    (constructionLimits queryLimits : oracle.Limits)
    (constructionControl queryControl : zetesis_cpu.cancellation.Cancellation)
    (candidateStored : NativeMembership.Represented candidate)
    (testedStored : NativeMembership.Represented tested)
    (valid : NativeStructure.WellFormed candidate.theory.value.atoms.val
      (NativeExecution.view candidate.theory))
    (rootsBounded : ∀ root ∈ candidate.theory.value.roots.val,
      root.val < (NativeTable.rows (NativeExecution.view candidate.theory)).length)
    (frozen : reduct.FrozenReduct)
    (constructed : @reduct.FrozenReduct.new construction candidate
      constructionLimits constructionControl = ok (.Ok frozen))
    (answer : Bool)
    (queried : @reduct.FrozenReduct.is_satisfied_by reservation frozen tested queryLimits queryControl =
      ok (.Ok answer)) :
    answer = true ↔ Zetesis.Ferraris.Models
      (Zetesis.TightEvaluation.interpretation (NativeMembership.denotes tested))
      (Zetesis.Ferraris.ReductTheory
        (Zetesis.TightEvaluation.interpretation (NativeMembership.denotes candidate))
        (NativeRootSemantics.assertions candidate.theory)) := by
  obtain ⟨sameCandidate, represented, _⟩ :=
    NativeFrozenConstruction.new_represents (reservation := construction) candidate constructionLimits
      constructionControl valid candidateStored frozen constructed
  have storedValid : NativeStructure.WellFormed frozen.candidate.theory.value.atoms.val
      (NativeExecution.view frozen.candidate.theory) := by
    simpa only [sameCandidate] using valid
  have storedRoots : ∀ root ∈ frozen.candidate.theory.value.roots.val,
      root.val < (NativeTable.rows (NativeExecution.view frozen.candidate.theory)).length := by
    simpa only [sameCandidate] using rootsBounded
  have meaning : answer = true ↔ Zetesis.Ferraris.Models
      (Zetesis.TightEvaluation.interpretation (NativeMembership.denotes tested))
      (Zetesis.Ferraris.ReductTheory
        (Zetesis.TightEvaluation.interpretation (NativeMembership.denotes frozen.candidate))
        (NativeRootSemantics.assertions frozen.candidate.theory)) := by
    exact completed_satisfaction (reservation := reservation) frozen tested queryLimits queryControl represented
      testedStored storedValid storedRoots answer queried
  simpa only [sameCandidate] using meaning

end NativePublicFrozenQuery
