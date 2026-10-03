import ReferenceEvents
import PublicCheckPhases

open Aeneas Aeneas.Std Result Aeneas.Data.Coinductive
open ZetesisExtract RuntimeEffects

/-!
# The terminal source phase of a public refusal

A returned Stop is recovered from an actual call in the source-checked public
context. Its phase consumes a terminal suffix of the public event history: the
wrapper does not continue to another phase or publish a decision after that Err.
The receipt identifies the phase, its actual arguments and its returned reason.
It does not reconstruct every earlier workspace. Phase-specific laws establish
the leaf read, reservation rejection or quota test causing that refusal.
-/
namespace PublicRefusal

local instance : MonadLift Result Computation where
  monadLift := RuntimeEffects.embed

/-- An actual terminal call of the public context. Fixed inputs stay explicit;
internal buffers and work records are recovered by execution inversion. -/
inductive Site (program : theory.Theory) (candidate : theory.Interpretation)
    (control : zetesis_cpu.cancellation.Cancellation) (reason : zetesis_cpu.cancellation.Stop) :
    List Event → Prop where
  | owner (called : oracle.identities program candidate = ok (.Err reason)) : Site program candidate control reason []
  | poll {events} (called : Runs (ContextEvents.poll control) events (.Err reason)) :
      Site program candidate control reason events
  | originalReservation {events} (called : Runs
      (ReservationEvents.reserve Bool program.value.nodes.deref.len) events (.Err reason)) :
      Site program candidate control reason events
  | evaluation {input before output after events} (called : Runs
      (ReferenceEvents.evaluate program candidate none input before) events (.Err reason, output, after)) :
      Site program candidate control reason events
  | roots {values before after events} (called : Runs
      (ReferenceEvents.failedRoot program values before) events (.Err reason, after)) :
      Site program candidate control reason events
  | selectionReservation {events} (called : Runs
      (ReservationEvents.reserve Usize program.value.atoms) events (.Err reason)) :
      Site program candidate control reason events
  | selection {input before output after events} (called : Runs
      (ReferenceEvents.selectAtoms program candidate input before) events (.Err reason, output, after)) :
      Site program candidate control reason events
  | wordReservation {events} (called : Runs
      (ReservationEvents.reserve U64 (alloc.vec.Vec.len candidate.words)) events (.Err reason)) :
      Site program candidate control reason events
  | scratchReservation {events} (called : Runs
      (ReservationEvents.reserve Bool program.value.nodes.deref.len) events (.Err reason)) :
      Site program candidate control reason events
  | search {frozen selected subset input before returned output after events} (called : Runs
      (ReferenceEvents.findCountermodel program frozen selected subset input before)
        events (.Err reason, returned, output, after)) : Site program candidate control reason events

/-- A completed typed refusal has an actual terminal phase returning that same
reason, and consumes no observations after that phase. Neither a successful
phase nor a backend nonreturn is relabeled as this source Err.

Proof: invert the context's binds in order. A phase Err reaches only the source
return branch; a phase success exposes the next call. Pure setup consumes no
observations. The two successful verdict branches contradict the supplied Err.
-/
theorem terminal_phase (program : theory.Theory) (candidate : theory.Interpretation)
    (limits : oracle.Limits) (control : zetesis_cpu.cancellation.Cancellation)
    (reason : zetesis_cpu.cancellation.Stop) (events : List Event)
    (run : Runs (ReferenceEvents.check program candidate limits control) events (.Err reason)) :
    ∃ earlier terminal, events = earlier ++ terminal ∧ Site program candidate control reason terminal := by
  let Receipt := fun history => ∃ earlier terminal,
    history = earlier ++ terminal ∧ Site program candidate control reason terminal
  have atSite {history} (site : Site program candidate control reason history) : Receipt history :=
    ⟨[], history, rfl, site⟩
  have preceded {whole before after} (partition : whole = before ++ after)
      (rest : Receipt after) : Receipt whole := by
    obtain ⟨earlier, terminal, split, site⟩ := rest
    exact ⟨before ++ earlier, terminal, by rw [partition, split, List.append_assoc], site⟩
  have takeSource {A : Type}
      (operation : Computation (core.result.Result A zetesis_cpu.cancellation.Stop))
      (next : core.result.Result A zetesis_cpu.cancellation.Stop →
        Computation (core.result.Result oracle.Check zetesis_cpu.cancellation.Stop))
      (history : List Event)
      (refused : ∀ stop, next (.Err stop) = ITree.ret (.Err stop))
      (returned : Runs (ITree.bind operation next) history (.Err reason)) :
      Runs operation history (.Err reason) ∨
        ∃ value before after, history = before ++ after ∧
          Runs operation before (.Ok value) ∧ Runs (next (.Ok value)) after (.Err reason) := by
    obtain ⟨before, after, answer, partition, called, following⟩ := RuntimeRuns.bind_inv _ _ _ _ returned
    cases answer with
    | Err stop =>
        rw [refused] at following
        obtain ⟨same, silent⟩ := RuntimeRuns.returned_inv _ _ _ following
        have sameReason : stop = reason := core.result.Result.Err.inj same
        have sameHistory : history = before := by simpa only [silent, List.append_nil] using partition
        subst stop
        exact .inl (by simpa only [sameHistory] using called)
    | Ok value => exact .inr ⟨value, before, after, partition, called, following⟩
  have takePhase {A Frame : Type}
      (operation : Computation ((core.result.Result A zetesis_cpu.cancellation.Stop) × Frame))
      (next : ((core.result.Result A zetesis_cpu.cancellation.Stop) × Frame) →
        Computation (core.result.Result oracle.Check zetesis_cpu.cancellation.Stop))
      (history : List Event)
      (refused : ∀ stop frame, next (.Err stop, frame) = ITree.ret (.Err stop))
      (returned : Runs (ITree.bind operation next) history (.Err reason)) :
      (∃ frame, Runs operation history (.Err reason, frame)) ∨
        ∃ value frame before after, history = before ++ after ∧
          Runs operation before (.Ok value, frame) ∧ Runs (next (.Ok value, frame)) after (.Err reason) := by
    obtain ⟨before, after, answer, partition, called, following⟩ := RuntimeRuns.bind_inv _ _ _ _ returned
    rcases answer with ⟨answer, frame⟩
    cases answer with
    | Err stop =>
        rw [refused] at following
        obtain ⟨same, silent⟩ := RuntimeRuns.returned_inv _ _ _ following
        have sameReason : stop = reason := core.result.Result.Err.inj same
        have sameHistory : history = before := by simpa only [silent, List.append_nil] using partition
        subst stop
        exact .inl ⟨frame, by simpa only [sameHistory] using called⟩
    | Ok value => exact .inr ⟨value, frame, before, after, partition, called, following⟩
  unfold ReferenceEvents.check CheckerContexts.check at run
  change Runs (ITree.bind (embed (oracle.identities program candidate)) _) _ _ at run
  obtain ⟨ownership, owned, phase⟩ := RuntimeRuns.embedded_bind_inv _ _ _ _ run
  cases ownership with
  | Err stop =>
      simp [core.result.Result.Insts.CoreOpsTry.branch,
        core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual,
        core.convert.FromSame.from, ContextEvents.lift_result, embed_ok, Bind.bind,
        itree_ret_bind] at phase
      obtain ⟨same, silent⟩ := RuntimeRuns.returned_inv _ _ _ phase
      have sameReason : stop = reason := core.result.Result.Err.inj same
      subst stop
      subst events
      exact atSite (.owner owned)
  | Ok accepted =>
      cases accepted
      simp only [core.result.Result.Insts.CoreOpsTry.branch,
        ContextEvents.lift_result, embed_ok, Bind.bind, itree_ret_bind] at phase
      rcases takeSource (ContextEvents.poll control) _ _
        (by intro stop; simp [core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual,
          core.convert.FromSame.from, embed_ok, Bind.bind, itree_ret_bind]) phase with stopped | continued
      · exact atSite (.poll stopped)
      obtain ⟨polled, _, _, partition, _, phase⟩ := continued
      apply preceded partition
      cases polled
      simp only [WorkInitialization.statistics_default, theory.Theory.nodes, theory.Theory.atom_count,
        alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref, OwnerChecks.theory_clone_exact,
        embed_ok, bind_ok, Bind.bind, itree_ret_bind] at phase
      rcases takeSource (ReservationEvents.reserve Bool program.value.nodes.deref.len) _ _
        (by intro stop; simp [core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual,
          core.convert.FromSame.from, embed_ok, itree_ret_bind]) phase with stopped | continued
      · exact atSite (.originalReservation stopped)
      obtain ⟨old, _, _, partition, _, phase⟩ := continued
      apply preceded partition
      simp only [embed_ok, itree_ret_bind] at phase
      rcases takePhase (ReferenceEvents.evaluate program candidate none old
        (WorkInitialization.initial limits control)) _ _
        (by
          intro stop frame
          rcases frame with ⟨values, work⟩
          simp [core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual,
            core.convert.FromSame.from, embed_ok, itree_ret_bind]) phase with stopped | continued
      · obtain ⟨frame, called⟩ := stopped
        rcases frame with ⟨output, after⟩
        exact atSite (.evaluation called)
      obtain ⟨evaluated, frame, _, _, partition, _, phase⟩ := continued
      apply preceded partition
      cases evaluated
      rcases frame with ⟨values, evaluationWork⟩
      simp only [embed_ok, itree_ret_bind] at phase
      rcases takePhase (ReferenceEvents.failedRoot program values.deref evaluationWork) _ _
        (by intro stop frame; simp [core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual,
          core.convert.FromSame.from, embed_ok, itree_ret_bind]) phase with stopped | continued
      · obtain ⟨after, called⟩ := stopped
        exact atSite (.roots called)
      obtain ⟨failed, rootWork, _, _, partition, _, phase⟩ := continued
      apply preceded partition
      cases failed with
      | some root =>
          simp only [embed_ok, itree_ret_bind] at phase
          have impossible := (RuntimeRuns.returned_inv _ _ _ phase).1
          cases impossible
      | none =>
          simp only [embed_ok, itree_ret_bind] at phase
          rcases takeSource (ReservationEvents.reserve Usize program.value.atoms) _ _
            (by intro stop; simp [core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual,
              core.convert.FromSame.from, embed_ok, itree_ret_bind]) phase with stopped | continued
          · exact atSite (.selectionReservation stopped)
          obtain ⟨destination, _, _, partition, _, phase⟩ := continued
          apply preceded partition
          simp only [embed_ok, itree_ret_bind] at phase
          rcases takePhase (ReferenceEvents.selectAtoms program candidate destination rootWork) _ _
            (by
              intro stop frame
              rcases frame with ⟨selected, work⟩
              simp [core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual,
                core.convert.FromSame.from, embed_ok, itree_ret_bind]) phase with stopped | continued
          · obtain ⟨frame, called⟩ := stopped
            rcases frame with ⟨output, after⟩
            exact atSite (.selection called)
          obtain ⟨selectedAnswer, frame, _, _, partition, _, phase⟩ := continued
          apply preceded partition
          cases selectedAnswer
          rcases frame with ⟨selected, selectionWork⟩
          simp only [embed_ok, itree_ret_bind] at phase
          rcases takeSource (ReservationEvents.reserve U64 (alloc.vec.Vec.len candidate.words)) _ _
            (by intro stop; simp [core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual,
              core.convert.FromSame.from, embed_ok, itree_ret_bind]) phase with stopped | continued
          · exact atSite (.wordReservation stopped)
          obtain ⟨wordDestination, _, _, partition, _, phase⟩ := continued
          apply preceded partition
          simp only [embed_ok, itree_ret_bind] at phase
          obtain ⟨words, _, phase⟩ := RuntimeRuns.embedded_bind_inv _ _ _ _ phase
          rcases takeSource (ReservationEvents.reserve Bool program.value.nodes.deref.len) _ _
            (by intro stop; simp [core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual,
              core.convert.FromSame.from, embed_ok, itree_ret_bind]) phase with stopped | continued
          · exact atSite (.scratchReservation stopped)
          obtain ⟨scratch, _, _, partition, _, phase⟩ := continued
          apply preceded partition
          simp only [embed_ok, itree_ret_bind] at phase
          rcases takePhase (ReferenceEvents.findCountermodel program values.deref selected.deref
            { theory := program, words } scratch selectionWork) _ _
            (by
              intro stop frame
              rcases frame with ⟨subset, output, work⟩
              simp [core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual,
                core.convert.FromSame.from, embed_ok, itree_ret_bind]) phase with stopped | continued
          · obtain ⟨frame, called⟩ := stopped
            rcases frame with ⟨returned, output, after⟩
            exact atSite (.search called)
          obtain ⟨found, frame, _, _, _, _, phase⟩ := continued
          rcases frame with ⟨subset, output, finalWork⟩
          simp only [embed_ok, itree_ret_bind] at phase
          cases found <;>
            have impossible := (RuntimeRuns.returned_inv _ _ _ phase).1 <;>
            cases impossible

end PublicRefusal
