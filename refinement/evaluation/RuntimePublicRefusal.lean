import PublicRefusal
import RootRefusal
import SelectionRefusal
import CarryRefusal
import EvaluationRefusal
import QueryRefusal
import SearchRefusal
import RuntimeSubject

open Aeneas Aeneas.Std Result
open ZetesisExtract RuntimeEffects

/-!
# Public refusals and their reached causes

A refused public call determines its actual terminal phase and the terminal
read, quota test or rejected reservation within that phase. The reason is
unchanged and no observation follows the refused operation. The intermediate
phase laws retain complete loop-prefix and query/search receipts; this final
classification keeps the phase call, reached control/work/request and exact
leaf suffix instead of duplicating that entire state tree.

These laws require no represented-storage, formula, fixed-control or successful
allocation premise. They describe finite typed refusals of the source-checked
contexts; backend failure and nonreturn are not converted to a source Stop.
-/
namespace RuntimePublicRefusal

/-- Exact leaf reasons and observations. A control receipt distinguishes
cancellation from expiry and records their source precedence. Work and subset
limits retain the reached counters; allocation retains the rejected request. -/
inductive Cause (program : theory.Theory) (candidate : theory.Interpretation) :
    zetesis_cpu.cancellation.Stop → List Event → Prop where
  | owner (different : program.owner ≠ candidate.theory.owner) :
      Cause program candidate .WrongProgram []
  | control (control : zetesis_cpu.cancellation.Cancellation)
      (events : List Event) (reason : zetesis_cpu.cancellation.Stop)
      (receipt : RuntimeRefusal.PollRefusal control events reason) :
      Cause program candidate reason events
  | work (before : oracle.Work)
      (exhausted : before.limits.max_work.val ≤ before.statistics.work.val) :
      Cause program candidate .WorkLimit (RuntimeRefusal.clearReads before.cancellation)
  | subsets (before : oracle.Work)
      (exhausted : before.limits.max_subsets.val ≤ before.statistics.subsets.val) :
      Cause program candidate .CandidateLimit (RuntimeRefusal.clearReads before.cancellation)
  | allocation (count : Usize) (error : ReservationError) :
      Cause program candidate .Allocation [⟨.reserve 0 count.val, .rejected error⟩]

/-- A public typed refusal has both an actual terminal phase and an exact leaf
cause at the end of its history. Loop causes are obtained from actually reached
states, through the existing prefix receipts; no hypothetical failing inner call
is a premise. The two splits express that neither the phase nor the public
wrapper consumes observations after its refused leaf.

Proof: recover the terminal public phase. Compose evaluation, root, selection and
carry prefixes with their reached tick receipts. Compose query and search phase
origins in source order. Poll, quota, reservation and owner laws identify the
leaf reason. Every prefix is prepended without changing that reason or suffix. -/
theorem classified (program : theory.Theory) (candidate : theory.Interpretation)
    (limits : oracle.Limits) (control : zetesis_cpu.cancellation.Cancellation)
    (reason : zetesis_cpu.cancellation.Stop) (events : List Event)
    (run : Runs (ReferenceEvents.check program candidate limits control) events (.Err reason)) :
    ∃ earlier phaseEvents within leafEvents,
      events = earlier ++ phaseEvents ∧
      PublicRefusal.Site program candidate control reason phaseEvents ∧
      phaseEvents = within ++ leafEvents ∧ Cause program candidate reason leafEvents := by
  let Explained := fun history => ∃ before last,
    history = before ++ last ∧ Cause program candidate reason last
  have direct {history} (cause : Cause program candidate reason history) : Explained history :=
    ⟨[], history, rfl, cause⟩
  have prepend {whole before after} (partition : whole = before ++ after)
      (rest : Explained after) : Explained whole := by
    obtain ⟨earlierEvents, last, split, cause⟩ := rest
    exact ⟨before ++ earlierEvents, last, by rw [partition, split, List.append_assoc], cause⟩
  have poll {observedControl history}
      (called : Runs (ContextEvents.poll observedControl) history (.Err reason)) : Explained history :=
    direct (.control observedControl history reason
      (RuntimeRefusal.poll_refused observedControl history reason called))
  have tick {before after history}
      (called : Runs (ContextEvents.tick before) history (.Err reason, after)) : Explained history := by
    obtain ⟨_, receipt⟩ := RuntimeRefusal.tick_refused before after history reason called
    cases receipt with
    | control receipt => exact direct (.control before.cancellation _ reason receipt)
    | work exhausted => exact direct (.work before exhausted)
  have reservation {T : Type} {count history}
      (called : Runs (ReservationEvents.reserve T count) history (.Err reason)) : Explained history := by
    obtain ⟨stopped, error, exactEvents⟩ := ReservationEvents.refused_wrapper T count history reason called
    dsimp only [Explained]
    rw [stopped]
    exact ⟨[], [⟨.reserve 0 count.val, .rejected error⟩], by simpa only [List.nil_append] using exactEvents,
      Cause.allocation count error⟩
  have evaluation {source tested frozen input output before after history}
      (called : Runs (ReferenceEvents.evaluate source tested frozen input before)
        history (.Err reason, output, after)) : Explained history := by
    obtain ⟨earlierEvents, last, reached, split, _, _, refused⟩ :=
      EvaluationRefusal.evaluate_refused source tested frozen input output before after history reason called
    exact prepend split (tick refused)
  have roots {source values before after history}
      (called : Runs (ReferenceEvents.failedRoot source values before)
        history (.Err reason, after)) : Explained history := by
    obtain ⟨earlierEvents, last, reached, split, _, refused⟩ :=
      RootRefusal.refused source values before after history reason called
    exact prepend split (tick refused)
  have selection {source tested input output before after history}
      (called : Runs (ReferenceEvents.selectAtoms source tested input before)
        history (.Err reason, output, after)) : Explained history := by
    obtain ⟨earlierEvents, last, reached, split, _, _, refused⟩ :=
      SelectionRefusal.refused source tested input output before after history reason called
    exact prepend split (tick refused)
  have carry {selected subset output present count before after history}
      (called : Runs (ReferenceEvents.advanceSubset selected subset present before)
        history (.Err reason, output, count, after)) : Explained history := by
    obtain ⟨earlierEvents, last, reached, split, _, _, _, refused⟩ :=
      CarryRefusal.refused selected subset output present count before after history reason called
    exact prepend split (tick refused)
  have query {source subset frozen input output before after history}
      (called : Runs (ReferenceEvents.checkSubset source subset frozen input before)
        history (.Err reason, output, after)) : Explained history := by
    have origin := QueryRefusal.refused source subset frozen input output before after history reason called
    cases origin with
    | poll receipt => exact direct (.control before.cancellation _ reason receipt)
    | quota exhausted => exact direct (.subsets before exhausted)
    | evaluation increment remaining incremented observations values returned stopped evaluated =>
        exact prepend rfl (evaluation evaluated)
    | roots increment remaining incremented evaluationEvents rootEvents values middle returned stopped evaluated checked =>
        exact prepend (by rw [List.append_assoc]) (roots checked)
  have search {source frozen selected subset returned input output before after history}
      (called : Runs (ReferenceEvents.findCountermodel source frozen selected subset input before)
        history (.Err reason, returned, output, after)) : Explained history := by
    have loopRun := SearchRefusal.search_refused source frozen selected subset returned input output
      before after reason history called
    obtain ⟨earlierEvents, last, reached, split, _, receipt⟩ := SearchRefusal.loop_refused source frozen
      selected subset returned input output before after 0#usize reason history loopRun
    apply prepend split
    cases receipt with
    | query observations values returned proper refused => exact query refused
    | carry first second values middle finalWork next count proper queried refused =>
        exact prepend rfl (carry refused)
  obtain ⟨earlier, phaseEvents, publicHistory, site⟩ :=
    PublicRefusal.terminal_phase program candidate limits control reason events run
  have explained : Explained phaseEvents := by
    cases site with
    | owner called =>
        have different : program.owner ≠ candidate.theory.owner := by
          intro same
          have accepted := (OwnerChecks.identities_accept_iff program candidate).mpr same
          have impossible := Result.ok_injective (accepted.symm.trans called)
          cases impossible
        have stopped : reason = .WrongProgram := by
          have rejected := OwnerChecks.identities_reject program candidate different
          exact (core.result.Result.Err.inj (Result.ok_injective (rejected.symm.trans called))).symm
        dsimp only [Explained]
        rw [stopped]
        exact ⟨[], [], rfl, .owner different⟩
    | poll called => exact poll called
    | originalReservation called => exact reservation called
    | evaluation called => exact evaluation called
    | roots called => exact roots called
    | selectionReservation called => exact reservation called
    | selection called => exact selection called
    | wordReservation called => exact reservation called
    | scratchReservation called => exact reservation called
    | search called => exact search called
  obtain ⟨within, leafEvents, leafHistory, cause⟩ := explained
  exact ⟨earlier, phaseEvents, within, leafEvents, publicHistory, site, leafHistory, cause⟩

/-- The owned wrapper forwards the same public refusal and observations, so its
leaf classification has no extra owner, allocation or success premise. -/
theorem classified_subject (candidate : theory.Interpretation) (limits : oracle.Limits)
    (control : zetesis_cpu.cancellation.Cancellation) (reason : zetesis_cpu.cancellation.Stop)
    (events : List Event)
    (run : Runs (ReferenceEvents.checkInterpretation candidate limits control) events (.Err reason)) :
    ∃ earlier phaseEvents within leafEvents,
      events = earlier ++ phaseEvents ∧
      PublicRefusal.Site candidate.theory candidate control reason phaseEvents ∧
      phaseEvents = within ++ leafEvents ∧ Cause candidate.theory candidate reason leafEvents := by
  exact classified candidate.theory candidate limits control reason events
    ((RuntimeSubject.refused_subject candidate limits control events reason).mp run)

end RuntimePublicRefusal
