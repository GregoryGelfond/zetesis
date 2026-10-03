import PublicCheckBoundary

open Aeneas Aeneas.Std Result
open ZetesisExtract

/-!
# Actual phases of the public reference checker

A completed public check is decomposed into the actual calls made by its
extracted body. The original-evaluation phase is common. A failed original root
returns immediately; otherwise selection, zero resize and proper-subset search
retain their actual arguments, results and cumulative work records.

These records make no semantic claim about masks, selected coordinates or search
results. They supply the operational evidence consumed by those independent
refinement laws. One supplied reservation provider serves the entire invocation.
Backend failure/divergence and source Stop cannot satisfy a completed Check
premise; no allocation-success, sequence-preservation or termination law is
assumed here. Controls retain the package's fixed-observation interpretation.
-/
namespace PublicCheckPhases

variable [reservation : VectorReservation]

/-- Actual shared prefix of every completed public check. No root result or
semantic correctness of the returned truth vector is included in this record. -/
structure Original (program : theory.Theory) (candidate : theory.Interpretation)
    (limits : oracle.Limits) (control : zetesis_cpu.cancellation.Cancellation) where
  old : alloc.vec.Vec Bool
  values : alloc.vec.Vec Bool
  after : oracle.Work
  owned : oracle.identities program candidate = ok (.Ok ())
  polled : zetesis_cpu.cancellation.Cancellation.poll control = ok (.Ok ())
  reserved : oracle.reserve Bool (alloc.vec.Vec.len program.value.nodes) = ok (.Ok old)
  evaluated : oracle.evaluate program candidate none old (WorkInitialization.initial limits control) =
    ok (.Ok (), values, after)

/-- The original scan returned its concrete failed root and exactly the work
statistics published by the public NotModel result. Later setup is absent. -/
def Rejected {program : theory.Theory} {candidate : theory.Interpretation}
    {limits : oracle.Limits} {control : zetesis_cpu.cancellation.Cancellation}
    (original : Original program candidate limits control) (checked : oracle.Check) : Prop :=
  ∃ root : Usize, ∃ after : oracle.Work,
    oracle.failed_root program original.values.slice original.after =
      ok (.Ok (some root), after) ∧
    checked = { verdict := .NotModel root, statistics := after.statistics }

/-- Every actual setup and search call reached after original satisfaction. The
Theory clone is normalized by its established exact call law, so the initial
subset retains the supplied program. Empty reservation contents are not assumed;
that library contract is required by the later semantic composition. -/
structure Searched {program : theory.Theory} {candidate : theory.Interpretation}
    {limits : oracle.Limits} {control : zetesis_cpu.cancellation.Cancellation}
    (original : Original program candidate limits control) (checked : oracle.Check) where
  rootAfter : oracle.Work
  originalModel : oracle.failed_root program original.values.slice original.after =
    ok (.Ok none, rootAfter)
  destination : alloc.vec.Vec Usize
  selected : alloc.vec.Vec Usize
  selectionAfter : oracle.Work
  selectedReserved : oracle.reserve Usize program.value.atoms = ok (.Ok destination)
  selectedComplete : oracle.select_atoms program candidate destination rootAfter =
    ok (.Ok (), selected, selectionAfter)
  wordDestination : alloc.vec.Vec U64
  words : alloc.vec.Vec U64
  wordsReserved : oracle.reserve U64 (alloc.vec.Vec.len candidate.words) =
    ok (.Ok wordDestination)
  resized : alloc.vec.Vec.resize core.clone.CloneU64 wordDestination
    (alloc.vec.Vec.len candidate.words) (0#u64) = ok words
  old : alloc.vec.Vec Bool
  output : alloc.vec.Vec Bool
  subset : theory.Interpretation
  found : Bool
  after : oracle.Work
  valuesReserved : oracle.reserve Bool (alloc.vec.Vec.len program.value.nodes) = ok (.Ok old)
  completed : oracle.find_countermodel program original.values.slice selected.slice
    { theory := program, words } old selectionAfter = ok (.Ok found, subset, output, after)
  published : checked =
    { verdict := if found then .NonMinimal subset else .Stable, statistics := after.statistics }


/-- A completed actual public Check derives its original execution and one of
its two completed branches. No inner successful call, semantic oracle agreement,
or reservation-content property is a premise.

Proof: foreign ownership or a stopped initial poll contradicts the supplied
completion. Invert each subsequent actual bind: backend failure/divergence and
source refusal cannot return a Check. The original failed-root branch publishes
immediately; the other branch retains every setup call and final search result.
-/
theorem completed_phases (program : theory.Theory) (candidate : theory.Interpretation)
    (limits : oracle.Limits) (control : zetesis_cpu.cancellation.Cancellation)
    (checked : oracle.Check)
    (completed : oracle.check program candidate limits control = ok (.Ok checked)) :
    ∃ original : Original program candidate limits control,
      Rejected original checked ∨ Nonempty (Searched original checked) := by
  have takeReturn {A B : Type} (operation : Result A) (next : A → Result B)
      (result : B) (returned : (operation >>= next) = ok result) :
      ∃ value, operation = ok value ∧ next value = ok result := by
    cases actual : operation with
    | ret value => exact ⟨value, rfl, by simpa only [actual, bind_tc_ok] using returned⟩
    | vis effect continuation => simp only [actual, bind_tc_vis, vis_not_ok] at returned
    | div => simp only [actual, bind_tc_div, div_not_ok] at returned
  have takeSource {A : Type}
      (operation : Result (core.result.Result A zetesis_cpu.cancellation.Stop))
      (next : core.result.Result A zetesis_cpu.cancellation.Stop →
        Result (core.result.Result oracle.Check zetesis_cpu.cancellation.Stop))
      (refused : ∀ reason, next (.Err reason) = ok (.Err reason))
      (returned : (operation >>= next) = ok (.Ok checked)) :
      ∃ value, operation = ok (.Ok value) ∧ next (.Ok value) = ok (.Ok checked) := by
    obtain ⟨result, actual, continued⟩ := takeReturn operation next (core.result.Result.Ok checked) returned
    cases result with
    | Err reason => rw [refused] at continued; simp at continued
    | Ok value => exact ⟨value, actual, continued⟩
  have sliceExact {A : Type} (value : alloc.vec.Vec A) : value.deref = value.slice := by
    apply Slice.ext
    simp only [alloc.vec.Vec.deref, Slice.from_val, alloc.vec.Vec.val]
  have same : program.owner = candidate.theory.owner := by
    by_contra different
    rw [PublicCheckBoundary.wrong_owner program candidate limits control different] at completed
    simp at completed
  have clear : EvaluatorControl.observation control = none := by
    cases observed : EvaluatorControl.observation control with
    | none => rfl
    | some reason =>
        rw [PublicCheckBoundary.stopped program candidate limits control same reason observed]
          at completed
        simp at completed
  have owned : oracle.identities program candidate = ok (.Ok ()) :=
    (OwnerChecks.identities_accept_iff program candidate).mpr same
  have polled : zetesis_cpu.cancellation.Cancellation.poll control = ok (.Ok ()) := by
    simp [EvaluatorControl.poll_exact, clear]
  simp only [oracle.check, owned, polled, WorkInitialization.statistics_default,
    theory.Theory.nodes, theory.Theory.atom_count,
    alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref, OwnerChecks.theory_clone_exact,
    core.result.Result.Insts.CoreOpsTry.branch, bind_tc_ok,
    core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual,
    core.convert.FromSame.from] at completed
  obtain ⟨old, reserved, phase⟩ := takeSource (oracle.reserve Bool program.value.nodes.deref.len) _ (by intro reason; simp only [bind_tc_ok]) completed
  simp only [bind_tc_ok] at phase
  obtain ⟨evaluation, evaluated, phase⟩ := takeReturn
    (oracle.evaluate program candidate none old (WorkInitialization.initial limits control))
    _ (core.result.Result.Ok checked) phase
  rcases evaluation with ⟨answer, values, after⟩
  have success : answer = .Ok () := by
    cases answer with
    | Ok acceptedUnit => cases acceptedUnit; rfl
    | Err reason => simp at phase
  subst answer
  simp only [uncurry, bind_tc_ok] at phase
  obtain ⟨scan, scanned, phase⟩ := takeReturn
    (oracle.failed_root program values.deref after) _ (core.result.Result.Ok checked)
    (by simpa only [bind_tc_ok] using phase)
  rcases scan with ⟨answer, rootAfter⟩
  have scanSuccess : ∃ result, answer = .Ok result := by
    cases answer with
    | Ok result => exact ⟨result, rfl⟩
    | Err reason => simp at phase
  obtain ⟨root, rfl⟩ := scanSuccess
  simp only [uncurry, bind_tc_ok] at phase
  have reservedExact : oracle.reserve Bool (alloc.vec.Vec.len program.value.nodes) =
      ok (.Ok old) := by
    simpa only [alloc.vec.Vec.len, alloc.vec.Vec.deref, Slice.len, Slice.from_val] using reserved
  let original : Original program candidate limits control :=
    ⟨old, values, after, owned, polled, reservedExact, evaluated⟩
  refine ⟨original, ?_⟩
  cases root with
  | some root =>
      left
      refine ⟨root, rootAfter, ?_, ?_⟩
      · simpa only [original, sliceExact] using scanned
      · simpa only [Result.ok.injEq, core.result.Result.Ok.injEq] using phase.symm
  | none =>
      right
      obtain ⟨destination, selectedReserved, phase⟩ := takeSource (oracle.reserve Usize program.value.atoms) _ (by intro reason; simp only [bind_tc_ok]) phase
      simp only [bind_tc_ok] at phase
      obtain ⟨selection, selectedComplete, phase⟩ := takeReturn
        (oracle.select_atoms program candidate destination rootAfter)
        _ (core.result.Result.Ok checked) phase
      rcases selection with ⟨answer, selected, selectionAfter⟩
      have selectionSuccess : answer = .Ok () := by
        cases answer with
        | Ok acceptedUnit => cases acceptedUnit; rfl
        | Err reason => simp at phase
      subst answer
      simp only [uncurry, bind_tc_ok] at phase
      obtain ⟨wordDestination, wordsReserved, phase⟩ := takeSource (oracle.reserve U64 candidate.words.len) _
        (by intro reason; simp only [bind_tc_ok]) phase
      simp only [bind_tc_ok] at phase
      obtain ⟨words, resized, phase⟩ := takeReturn
        (alloc.vec.Vec.resize core.clone.CloneU64 wordDestination candidate.words.len (0#u64))
        _ (core.result.Result.Ok checked) phase
      obtain ⟨workspace, valuesReserved, phase⟩ := takeSource (oracle.reserve Bool program.value.nodes.deref.len) _ (by intro reason; simp only [bind_tc_ok]) phase
      simp only [bind_tc_ok] at phase
      obtain ⟨search, searched, phase⟩ := takeReturn
        (oracle.find_countermodel program values.deref selected.deref
          { theory := program, words } workspace selectionAfter)
        _ (core.result.Result.Ok checked) phase
      rcases search with ⟨answer, subset, output, searchAfter⟩
      have searchSuccess : ∃ result, answer = .Ok result := by
        cases answer with
        | Ok result => exact ⟨result, rfl⟩
        | Err reason => simp at phase
      obtain ⟨found, rfl⟩ := searchSuccess
      simp only [uncurry, bind_tc_ok] at phase
      have valuesReservedExact : oracle.reserve Bool (alloc.vec.Vec.len program.value.nodes) =
          ok (.Ok workspace) := by
        simpa only [alloc.vec.Vec.len, alloc.vec.Vec.deref, Slice.len, Slice.from_val] using valuesReserved
      have published : checked =
          { verdict := if found then .NonMinimal subset else .Stable,
            statistics := searchAfter.statistics } := by
        cases found <;> simpa only [Bool.false_eq_true, Bool.true_eq, if_false, if_true,
          Result.ok.injEq, core.result.Result.Ok.injEq] using phase.symm
      exact ⟨{
        rootAfter := rootAfter
        originalModel := by simpa only [original, sliceExact] using scanned
        destination := destination
        selected := selected
        selectionAfter := selectionAfter
        selectedReserved := selectedReserved
        selectedComplete := selectedComplete
        wordDestination := wordDestination
        words := words
        wordsReserved := wordsReserved
        resized := resized
        old := workspace
        output := output
        subset := subset
        found := found
        after := searchAfter
        valuesReserved := valuesReservedExact
        completed := by simpa only [original, sliceExact] using searched
        published := published }⟩

end PublicCheckPhases
