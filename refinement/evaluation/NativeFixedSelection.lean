import NativeSelectedAtoms
import NativeSubsetSteps

open Aeneas Aeneas.Std Result ControlFlow
open ZetesisNativeExtract

/-!
# The actual selected-atom loop

The generated selection helper visits the declared universe in increasing order.
Each admitted tick visits one coordinate, whether or not it appends an atom. Its
exact selected prefix and work receipt survive both completion and typed refusal.
The proof composes the actual generated body and loop with the existing range,
packed membership and vector operations; it defines no replacement scan.

These laws use the imported fixed-observation and sequence models. Their entry
premises include represented candidate words, agreement of universe dimensions,
and an empty destination. They do not establish the caller's pointer identity,
fallible reservation or concurrent atomic behavior. Candidate words are read only;
zero padding is unnecessary for these bounded reads.
-/
namespace NativeFixedSelection

/-- The returned vector is exactly the selected prefix of visited coordinates.
    Work counts visited universe coordinates rather than selected atoms. Success
    reaches the universe boundary; refusal leaves a strict prefix and the actual
    next tick refuses with the returned reason and no further charged work. -/
structure Report (candidate : theory.Interpretation) (first visited : Nat)
    (before : oracle.Work) (result : core.result.Result Unit zetesis_cpu.cancellation.Stop)
    (output : alloc.vec.Vec Usize) (after : oracle.Work) : Prop where
  monotone : first ≤ visited
  bounded : visited ≤ candidate.theory.value.atoms.val
  selection : NativeSelectedAtoms.SelectedPrefix candidate visited output
  limits : after.limits = before.limits
  control : after.cancellation = before.cancellation
  subsets : after.statistics.subsets = before.statistics.subsets
  work : after.statistics.work.val = before.statistics.work.val + (visited - first)
  boundary : match result with
    | .Ok _ => visited = candidate.theory.value.atoms.val
    | .Err reason => visited < candidate.theory.value.atoms.val ∧
        oracle.Work.tick after = ok (core.result.Result.Err reason, after)

/-- One admitted actual body step visits the next coordinate, preserves the
    selected-prefix invariant and charges exactly one tick. The generated body
    appends only when its real packed membership query is true. Range advancement,
    membership and push success are derived from their established invariants. -/
theorem body_advances (candidate : theory.Interpretation)
    (iter : core.ops.range.Range Usize) (selected : alloc.vec.Vec Usize) (work : oracle.Work)
    (sameUniverse : iter.end = candidate.theory.value.atoms)
    (stored : NativeMembership.Represented candidate)
    (selection : NativeSelectedAtoms.SelectedPrefix candidate iter.start.val selected)
    (inside : iter.start.val < iter.end.val)
    (clear : NativeControl.observation work.cancellation = none)
    (remaining : work.statistics.work.val < work.limits.max_work.val) :
    ∃ next : core.ops.range.Range Usize, ∃ output : alloc.vec.Vec Usize, ∃ charged : U64,
      next.start.val = iter.start.val + 1 ∧ next.end = candidate.theory.value.atoms ∧
      charged.val = work.statistics.work.val + 1 ∧
      oracle.select_atoms_loop.body candidate iter selected work =
        ok (.cont (next, output,
          { work with statistics := { work.statistics with work := charged } })) ∧
      NativeSelectedAtoms.SelectedPrefix candidate next.start.val output := by
  obtain ⟨next, output, advanced, nextStart, nextEnd, membership, appended, nextSelection⟩ :=
    NativeSelectedAtoms.advance candidate iter selected sameUniverse stored selection inside
  obtain ⟨charged, countExact, ticked⟩ := NativeControl.tick_advances work clear remaining
  refine ⟨next, output, charged, nextStart, nextEnd, countExact, ?_, nextSelection⟩
  cases bit : NativeMembership.denotes candidate iter.start.val with
  | false =>
      have unchanged : selected = output := by
        apply Result.ok_injective
        simpa only [bit, Bool.false_eq_true, ↓reduceIte] using appended
      subst output
      simp [oracle.select_atoms_loop.body, advanced, ticked,
        core.result.Result.Insts.CoreOpsTry.branch, membership, bit]
  | true =>
      have pushed : alloc.vec.Vec.push selected iter.start = ok output := by
        simpa only [bit, ↓reduceIte] using appended
      simp [oracle.select_atoms_loop.body, advanced, ticked,
        core.result.Result.Insts.CoreOpsTry.branch, membership, bit, pushed]

/-- One unfolding of the actual generated loop returns a done result or passes
    every returned continuation field, including the work/control record, into
    that same generated loop. No fresh observation is introduced. -/
theorem loop_unfold (candidate : theory.Interpretation) (iter : core.ops.range.Range Usize)
    (selected : alloc.vec.Vec Usize) (work : oracle.Work) :
    oracle.select_atoms_loop iter candidate selected work = (do
      let transition ← oracle.select_atoms_loop.body candidate iter selected work
      match transition with
      | .done result => ok result
      | .cont (next, output, returned) => oracle.select_atoms_loop next candidate output returned) := by
  rw [oracle.select_atoms_loop, loop]
  dsimp only [uncurry]
  congr 1
  funext transition
  cases transition with
  | done result => rfl
  | cont result =>
    obtain ⟨next, output, returned⟩ := result
    rfl

/-- Refusing the next tick terminates the actual loop with the existing selected
    prefix and no additional visited coordinate. The iterator fetch has happened,
    but the done result contains only the unchanged vector and work record. -/
theorem loop_refused (candidate : theory.Interpretation) (iter : core.ops.range.Range Usize)
    (selected : alloc.vec.Vec Usize) (work : oracle.Work)
    (sameUniverse : iter.end = candidate.theory.value.atoms)
    (selection : NativeSelectedAtoms.SelectedPrefix candidate iter.start.val selected)
    (inside : iter.start.val < iter.end.val) (reason : zetesis_cpu.cancellation.Stop)
    (refused : NativeControl.observation work.cancellation = some reason ∨
      (NativeControl.observation work.cancellation = none ∧
        work.limits.max_work.val ≤ work.statistics.work.val ∧ reason = .WorkLimit)) :
    oracle.select_atoms_loop iter candidate selected work =
      ok (core.result.Result.Err reason, selected, work) ∧
      Report candidate iter.start.val iter.start.val work (.Err reason) selected work := by
  have actual : oracle.select_atoms_loop iter candidate selected work =
      ok (core.result.Result.Err reason, selected, work) := by
    rw [loop_unfold, NativeSubsetSteps.selection_refused candidate iter selected work reason inside refused]
    simp only [bind_tc_ok]
  have beforeEnd : iter.start.val < candidate.theory.value.atoms.val := by
    simpa only [sameUniverse] using inside
  have tickRefused : oracle.Work.tick work = ok (core.result.Result.Err reason, work) := by
    rcases refused with observed | ⟨clear, exhausted, rfl⟩
    · exact NativeControl.tick_stopped work reason observed
    · exact NativeControl.tick_at_limit work clear exhausted
  exact ⟨actual, ⟨Nat.le_refl _, Nat.le_of_lt beforeEnd, selection, rfl, rfl, rfl,
    by omega, beforeEnd, tickRefused⟩⟩

/-- The actual generated loop terminates with a typed outcome and its exact
    visited-prefix receipt. At most the remaining universe coordinates can be
    visited. No successful-completion or available-budget premise is required.

    Proof: unfold the loop and distinguish exhaustion, a refused tick and an
    admitted coordinate. Only the last branch recurses. Its checked successor
    strictly decreases the number of unvisited coordinates, preserves selection,
    and contributes one unit to the recursively returned work receipt. The
    returned work/control value is the actual next invocation's input. -/
theorem loop_refines (candidate : theory.Interpretation) (iter : core.ops.range.Range Usize)
    (selected : alloc.vec.Vec Usize) (work : oracle.Work)
    (sameUniverse : iter.end = candidate.theory.value.atoms)
    (stored : NativeMembership.Represented candidate)
    (within : iter.start.val ≤ candidate.theory.value.atoms.val)
    (selection : NativeSelectedAtoms.SelectedPrefix candidate iter.start.val selected) :
    ∃ visited : Nat, ∃ result : core.result.Result Unit zetesis_cpu.cancellation.Stop,
      ∃ output : alloc.vec.Vec Usize, ∃ after : oracle.Work,
        oracle.select_atoms_loop iter candidate selected work = ok (result, output, after) ∧
        Report candidate iter.start.val visited work result output after := by
  have construct (remaining : Nat) :
      ∀ cursor : core.ops.range.Range Usize, ∀ currentSelected : alloc.vec.Vec Usize,
        ∀ current : oracle.Work,
        cursor.end = candidate.theory.value.atoms →
        cursor.start.val ≤ candidate.theory.value.atoms.val →
        NativeSelectedAtoms.SelectedPrefix candidate cursor.start.val currentSelected →
        candidate.theory.value.atoms.val - cursor.start.val = remaining →
        ∃ visited : Nat, ∃ result : core.result.Result Unit zetesis_cpu.cancellation.Stop,
          ∃ output : alloc.vec.Vec Usize, ∃ after : oracle.Work,
            oracle.select_atoms_loop cursor candidate currentSelected current =
              ok (result, output, after) ∧
            Report candidate cursor.start.val visited current result output after := by
    induction remaining using Nat.strong_induction_on with
    | h remaining ih =>
      intro cursor currentSelected current sameEnd bounded prefixExact unvisited
      by_cases inside : cursor.start.val < cursor.end.val
      · cases observed : NativeControl.observation current.cancellation with
        | some reason =>
            exact ⟨cursor.start.val, .Err reason, currentSelected, current,
              loop_refused candidate cursor currentSelected current sameEnd prefixExact
                inside reason (Or.inl observed)⟩
        | none =>
            by_cases exhausted : current.limits.max_work.val ≤ current.statistics.work.val
            · exact ⟨cursor.start.val, .Err .WorkLimit, currentSelected, current,
                loop_refused candidate cursor currentSelected current sameEnd prefixExact
                  inside .WorkLimit (Or.inr ⟨observed, exhausted, rfl⟩)⟩
            · obtain ⟨next, nextSelected, charged, nextStart, nextEnd, countExact,
                stepped, nextSelection⟩ := body_advances candidate cursor currentSelected
                  current sameEnd stored prefixExact inside observed (Nat.lt_of_not_ge exhausted)
              let nextWork : oracle.Work :=
                { current with statistics := { current.statistics with work := charged } }
              have nextWithin : next.start.val ≤ candidate.theory.value.atoms.val := by
                rw [sameEnd] at inside
                omega
              have fewer : candidate.theory.value.atoms.val - next.start.val < remaining := by
                omega
              obtain ⟨visited, result, output, after, returned, receipt⟩ :=
                ih (candidate.theory.value.atoms.val - next.start.val) fewer
                  next nextSelected nextWork nextEnd nextWithin nextSelection rfl
              have actual : oracle.select_atoms_loop cursor candidate currentSelected current =
                  ok (result, output, after) := by
                rw [loop_unfold, stepped]
                simpa only [bind_tc_ok] using returned
              have visitedAfter : cursor.start.val ≤ visited := by
                have progressed : next.start.val ≤ visited := by
                  exact receipt.monotone
                omega
              have exactWork : after.statistics.work.val =
                  current.statistics.work.val + (visited - cursor.start.val) := by
                have restWork : after.statistics.work.val =
                    charged.val + (visited - next.start.val) := by
                  exact receipt.work
                have progressed : next.start.val ≤ visited := by
                  exact receipt.monotone
                omega
              exact ⟨visited, result, output, after, actual,
                ⟨visitedAfter, receipt.bounded, receipt.selection, receipt.limits,
                  receipt.control, receipt.subsets, exactWork, receipt.boundary⟩⟩
      · have atEnd : cursor.start.val = candidate.theory.value.atoms.val := by
          rw [sameEnd] at inside
          omega
        have exhausted : cursor.end.val ≤ cursor.start.val := by
          exact Nat.le_of_not_gt inside
        have actual : oracle.select_atoms_loop cursor candidate currentSelected current =
            ok (core.result.Result.Ok (), currentSelected, current) := by
          rw [loop_unfold, NativeSubsetSteps.selection_exhausted candidate cursor currentSelected current exhausted]
          simp only [bind_tc_ok]
        exact ⟨cursor.start.val, .Ok (), currentSelected, current, actual,
          ⟨Nat.le_refl _, bounded, prefixExact, rfl, rfl, rfl, by omega, atEnd⟩⟩
  exact construct (candidate.theory.value.atoms.val - iter.start.val)
    iter selected work sameUniverse within selection rfl

/-- Starting from the caller's empty destination, the actual helper initializes
    the range at zero and returns an exact prefix with a finite work receipt.
    Universe agreement is numeric; it does not stand for Rust owner identity.
    The caller's reservation and physical vector capacity remain outside this theorem.
-/
theorem select_refines (program : theory.Theory) (candidate : theory.Interpretation)
    (selected : alloc.vec.Vec Usize) (work : oracle.Work)
    (sameUniverse : program.value.atoms = candidate.theory.value.atoms)
    (stored : NativeMembership.Represented candidate) (empty : selected.val = []) :
    ∃ visited : Nat, ∃ result : core.result.Result Unit zetesis_cpu.cancellation.Stop,
      ∃ output : alloc.vec.Vec Usize, ∃ after : oracle.Work,
        oracle.select_atoms program candidate selected work = ok (result, output, after) ∧
        Report candidate 0 visited work result output after := by
  let initial : core.ops.range.Range Usize := ⟨0#usize, program.value.atoms⟩
  have initialSelection : NativeSelectedAtoms.SelectedPrefix candidate initial.start.val selected := by
    simp [NativeSelectedAtoms.SelectedPrefix, initial, empty]
  obtain ⟨visited, result, output, after, returned, receipt⟩ := loop_refines
    candidate initial selected work sameUniverse stored (by simp [initial]) initialSelection
  refine ⟨visited, result, output, after, ?_, ?_⟩
  · simpa [oracle.select_atoms, theory.Theory.atom_count,
      alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref, initial] using returned
  · simpa only [initial, UScalar.ofNatCore_val_eq] using receipt

/-- Any actual typed helper return has the exact prefix receipt. Construct the
    actual terminating result and identify its fields with the supplied equation;
    no helper-correctness or selected-prefix premise is assumed at the output. -/
theorem returned_report (program : theory.Theory) (candidate : theory.Interpretation)
    (selected : alloc.vec.Vec Usize) (work : oracle.Work)
    (sameUniverse : program.value.atoms = candidate.theory.value.atoms)
    (stored : NativeMembership.Represented candidate) (empty : selected.val = [])
    (result : core.result.Result Unit zetesis_cpu.cancellation.Stop)
    (output : alloc.vec.Vec Usize) (after : oracle.Work)
    (returned : oracle.select_atoms program candidate selected work = ok (result, output, after)) :
    ∃ visited : Nat, Report candidate 0 visited work result output after := by
  obtain ⟨visited, actualResult, actualOutput, actualWork, executed, receipt⟩ :=
    select_refines program candidate selected work sameUniverse stored empty
  have same : (actualResult, actualOutput, actualWork) = (result, output, after) := by
    exact Result.ok_injective (executed.symm.trans returned)
  cases same
  exact ⟨visited, receipt⟩

/-- A successful actual helper return is exactly the shared packed selected-ID
    producer, in increasing order. Every true candidate coordinate is selected;
    no output coverage, distinctness or selection-agreement premise is supplied.
    The empty initial destination and represented input remain explicit. -/
theorem completed_selection (program : theory.Theory) (candidate : theory.Interpretation)
    (selected : alloc.vec.Vec Usize) (work : oracle.Work)
    (sameUniverse : program.value.atoms = candidate.theory.value.atoms)
    (stored : NativeMembership.Represented candidate) (empty : selected.val = [])
    (output : alloc.vec.Vec Usize) (after : oracle.Work)
    (completed : oracle.select_atoms program candidate selected work =
      ok (core.result.Result.Ok (), output, after)) :
    output.val.map UScalar.val =
      (Zetesis.Refinement.PackedSubsets.selectedAtoms candidate.theory.value.atoms.val
        (candidate.words.val.map UScalar.bv)).map Fin.val := by
  obtain ⟨visited, receipt⟩ := returned_report program candidate selected work
    sameUniverse stored empty (.Ok ()) output after completed
  have fullSelection : NativeSelectedAtoms.SelectedPrefix candidate candidate.theory.value.atoms.val output := by
    simpa only [receipt.boundary] using receipt.selection
  exact NativeSelectedAtoms.completed_selection candidate output fullSelection

/-- Every typed helper return charges at most one tick per declared atom and
    never reduces cumulative work. Success charges exactly the universe size;
    a stop may retain fewer visits even when the selected vector is unchanged.
-/
theorem returned_work_bound (program : theory.Theory) (candidate : theory.Interpretation)
    (selected : alloc.vec.Vec Usize) (work : oracle.Work)
    (sameUniverse : program.value.atoms = candidate.theory.value.atoms)
    (stored : NativeMembership.Represented candidate) (empty : selected.val = [])
    (result : core.result.Result Unit zetesis_cpu.cancellation.Stop)
    (output : alloc.vec.Vec Usize) (after : oracle.Work)
    (returned : oracle.select_atoms program candidate selected work = ok (result, output, after)) :
    work.statistics.work.val ≤ after.statistics.work.val ∧
      after.statistics.work.val ≤ work.statistics.work.val + program.value.atoms.val ∧
      (result = .Ok () →
        after.statistics.work.val = work.statistics.work.val + program.value.atoms.val) := by
  obtain ⟨visited, receipt⟩ := returned_report program candidate selected work
    sameUniverse stored empty result output after returned
  have exactWork : after.statistics.work.val = work.statistics.work.val + visited := by
    simpa only [Nat.sub_zero] using receipt.work
  have bounded : visited ≤ program.value.atoms.val := by
    simpa only [sameUniverse] using receipt.bounded
  refine ⟨by omega, by omega, ?_⟩
  intro succeeded
  have full : visited = program.value.atoms.val := by
    simpa only [succeeded, sameUniverse] using receipt.boundary
  omega

/-- Clear fixed observations and allowance for every universe coordinate suffice
    for an actual successful helper return. This is a constructive completion
    bound under the mathematical observation model, not a promise that runtime
    cancellation or deadline observations remain clear.

    Proof: termination supplies an actual typed result. A stopped receipt would
    leave fewer than all coordinates visited; its returned work must therefore
    still fit the allowance. The derived successful next tick contradicts that
    receipt's actual refused tick. -/
theorem select_completes (program : theory.Theory) (candidate : theory.Interpretation)
    (selected : alloc.vec.Vec Usize) (work : oracle.Work)
    (sameUniverse : program.value.atoms = candidate.theory.value.atoms)
    (stored : NativeMembership.Represented candidate) (empty : selected.val = [])
    (clear : NativeControl.observation work.cancellation = none)
    (allowance : work.statistics.work.val + program.value.atoms.val ≤ work.limits.max_work.val) :
    ∃ output : alloc.vec.Vec Usize, ∃ after : oracle.Work,
      oracle.select_atoms program candidate selected work =
        ok (core.result.Result.Ok (), output, after) := by
  obtain ⟨visited, result, output, after, executed, receipt⟩ :=
    select_refines program candidate selected work sameUniverse stored empty
  cases result with
  | Ok value =>
      cases value
      exact ⟨output, after, executed⟩
  | Err reason =>
      have clearAfter : NativeControl.observation after.cancellation = none := by
        rw [receipt.control]
        exact clear
      have remaining : after.statistics.work.val < after.limits.max_work.val := by
        have strictPrefix : visited < program.value.atoms.val := by
          simpa only [sameUniverse] using receipt.boundary.1
        have exactWork : after.statistics.work.val = work.statistics.work.val + visited := by
          simpa only [Nat.sub_zero] using receipt.work
        rw [receipt.limits]
        omega
      obtain ⟨next, _, ticked⟩ := NativeControl.tick_advances after clearAfter remaining
      have refused : oracle.Work.tick after = ok (core.result.Result.Err reason, after) := by
        exact receipt.boundary.2
      have incompatible : core.result.Result.Ok () = core.result.Result.Err reason := by
        exact congrArg Prod.fst (Result.ok_injective (ticked.symm.trans refused))
      cases incompatible

/-- The completed concrete selected IDs form a distinct bounded Nat carrier
    whose Boolean membership is exactly the original packed interpretation.
    This supplies the general subset counter directly, without a caller-provided
    uniqueness or coverage premise or a second finite-coordinate conversion. -/
theorem completed_carrier (program : theory.Theory) (candidate : theory.Interpretation)
    (selected : alloc.vec.Vec Usize) (work : oracle.Work)
    (sameUniverse : program.value.atoms = candidate.theory.value.atoms)
    (stored : NativeMembership.Represented candidate) (empty : selected.val = [])
    (output : alloc.vec.Vec Usize) (after : oracle.Work)
    (completed : oracle.select_atoms program candidate selected work =
      ok (core.result.Result.Ok (), output, after)) :
    (output.val.map UScalar.val).Nodup ∧
      (∀ atom ∈ output.val.map UScalar.val, atom < program.value.atoms.val) ∧
      Zetesis.FiniteMembership.candidate (output.val.map UScalar.val) = NativeMembership.denotes candidate := by
  obtain ⟨visited, receipt⟩ := returned_report program candidate selected work
    sameUniverse stored empty (.Ok ()) output after completed
  have fullSelection : NativeSelectedAtoms.SelectedPrefix candidate candidate.theory.value.atoms.val output := by
    simpa only [receipt.boundary] using receipt.selection
  have distinct : (output.val.map UScalar.val).Nodup := by
    exact NativeSelectedAtoms.selected_nodup candidate candidate.theory.value.atoms.val output fullSelection
  have bounded : ∀ atom ∈ output.val.map UScalar.val, atom < program.value.atoms.val := by
    intro atom member
    rw [fullSelection] at member
    have inRange : atom ∈ List.range candidate.theory.value.atoms.val := by
      exact (List.mem_filter.mp member).1
    simpa only [List.mem_range, sameUniverse] using inRange
  have exactMembership : Zetesis.FiniteMembership.candidate (output.val.map UScalar.val) =
      NativeMembership.denotes candidate := by
    funext atom
    rw [fullSelection]
    by_cases inside : atom < candidate.theory.value.atoms.val
    · simp [Zetesis.FiniteMembership.candidate, List.mem_filter, List.mem_range, inside]
    · have absent : NativeMembership.denotes candidate atom = false := by
        simp [NativeMembership.denotes, inside]
      simp [Zetesis.FiniteMembership.candidate, List.mem_filter, List.mem_range, inside, absent]
  exact ⟨distinct, bounded, exactMembership⟩

end NativeFixedSelection
