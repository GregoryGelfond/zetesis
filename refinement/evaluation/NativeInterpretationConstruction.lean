import Native.Funs
import UsizeCeiling
import VectorReservation

open Aeneas Aeneas.Std Result
open ZetesisNativeExtract

/-!
# The phases of packed interpretation construction

These laws follow the actual generated `Interpretation::new`: word counting,
fallible reservation, resize, conversion of the supplied iterable, the actual
insertion helper, and the final shared-theory clone. They preserve the generic
iterator dictionary; no concrete list or assumed iterator-correctness law
replaces that argument.

A successful constructor return supplies the inner-call equations rather than
assuming they succeeded. It retains the exact theory under the existing Arc
clone contract. This phase result does not yet establish the inserted population,
padding, iterator termination, or exact refused input position. Those require
the actual generic insertion-loop refinement. No logical zero-storage claim is
made here: such a claim also needs the explicit library contract that successful
reservation preserves its initially empty input, because resize retains an old
prefix. Physical capacity and destruction remain trusted runtime contracts.

Reservation uses a supplied operation for this invocation. No global instance,
allocation-success premise, or changing allocator history is introduced.
-/
namespace NativeInterpretationConstruction

/-- The actual generated clone retains the same shared theory under the explicit
native library clone model; no allocation or reference-count claim is added. -/
theorem theory_clone_exact (program : theory.Theory) :
    theory.Theory.Insts.CoreCloneClone.clone program = ok program := by
  simp [theory.Theory.Insts.CoreCloneClone.clone,
    ZetesisNativeExtract.alloc.sync.Arc.Insts.CoreCloneClone.clone]

variable [reservation : VectorReservation]
variable {Input Iter : Type}

/-- The constructor runs its actual phases in source order. A reservation
refusal becomes Allocation; an insertion refusal is returned unchanged. The
bind equation also retains backend failure and divergence of every phase.
The final shared-theory clone is discharged by its existing exact model law. -/
theorem phases_exact (source : core.iter.traits.collect.IntoIterator Input Usize Iter)
    (program : theory.Theory) (atoms : Input) :
    theory.Interpretation.new source program atoms =
      (do
        let count ← core.num.Usize.div_ceil program.value.atoms 64#usize
        let (reserved, words) ←
          alloc.vec.Vec.try_reserve_exact Global (alloc.vec.Vec.new U64) count
        match reserved with
        | .Err _ => ok (.Err theory.AdmissionError.Allocation)
        | .Ok _ =>
            let initialized ← alloc.vec.Vec.resize core.clone.CloneU64 words count 0#u64
            let iterator ← source.into_iter atoms
            let (inserted, stored) ← theory.insert_atoms source.iteratorInst program.value.atoms iterator
              initialized.slice
            match inserted with
            | .Err reason => ok (.Err reason)
            | .Ok _ => ok (.Ok { theory := program, words := { slice := stored } })) := by
  simp only [theory.Interpretation.new, theory.Theory.atom_count,
    alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref, bind_tc_ok]
  apply congrArg (fun next => core.num.Usize.div_ceil program.value.atoms 64#usize >>= next)
  funext count
  apply congrArg (fun next =>
    alloc.vec.Vec.try_reserve_exact Global (alloc.vec.Vec.new U64) count >>= next)
  funext returned
  rcases returned with ⟨reserved, words⟩
  cases reserved with
  | Err error =>
      simp [core.result.Result.map_err,
        theory.Interpretation.new.closure.Insts.CoreOpsFunctionFnOnceTupleTryReserveErrorAdmissionError.call_once,
        core.result.Result.Insts.CoreOpsTry.branch,
        core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]
  | Ok accepted =>
      cases accepted
      simp [core.result.Result.map_err, core.result.Result.Insts.CoreOpsTry.branch]
      apply congrArg (fun next => alloc.vec.Vec.resize core.clone.CloneU64 words count 0#u64 >>= next)
      funext initialized
      apply congrArg (fun next => source.into_iter atoms >>= next)
      funext iterator
      simp [alloc.vec.Vec.deref_mut, lift]
      apply congrArg (fun next => theory.insert_atoms source.iteratorInst program.value.atoms iterator
        initialized.slice >>= next)
      funext result
      rcases result with ⟨inserted, stored⟩
      cases inserted <;>
        simp [core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual,
          theory_clone_exact]

/-- A source reservation refusal is returned before iterable conversion or
insertion. The actual count and reservation equations suffice regardless of
the supplied iterator's behavior; no later phase is assumed to terminate. -/
theorem reservation_refused
    (source : core.iter.traits.collect.IntoIterator Input Usize Iter)
    (program : theory.Theory) (atoms : Input) (count : Usize)
    (error : alloc.collections.TryReserveError) (words : alloc.vec.Vec U64)
    (counted : core.num.Usize.div_ceil program.value.atoms 64#usize = ok count)
    (refused : alloc.vec.Vec.try_reserve_exact Global (alloc.vec.Vec.new U64) count =
      ok (.Err error, words)) :
    theory.Interpretation.new source program atoms = ok (.Err .Allocation) := by
  rw [phases_exact, counted]
  simp only [bind_tc_ok, refused]
  rfl

/-- A returned interpretation derives successful count, reservation, resize,
iterable conversion and actual insertion calls. Its theory is the supplied
theory, and its words are precisely the helper's returned slice. The count is
the shared packed word count; this statement makes no claim about the helper's
population or the old reserved vector's contents.

Proof: ceiling division by 64 always returns the exact count. Each subsequent
actual bind must return to produce constructor success. Refusal, backend failure
and divergence cannot produce that successful result. -/
theorem completed_phases
    (source : core.iter.traits.collect.IntoIterator Input Usize Iter)
    (program : theory.Theory) (atoms : Input) (result : theory.Interpretation)
    (completed : theory.Interpretation.new source program atoms = ok (.Ok result)) :
    ∃ count words initialized iterator stored,
      core.num.Usize.div_ceil program.value.atoms 64#usize = ok count ∧
      count.val = Zetesis.Refinement.PackedInterpretations.count64 program.value.atoms.val ∧
      alloc.vec.Vec.try_reserve_exact Global (alloc.vec.Vec.new U64) count =
        ok (.Ok (), words) ∧
      alloc.vec.Vec.resize core.clone.CloneU64 words count 0#u64 = ok initialized ∧
      source.into_iter atoms = ok iterator ∧
      theory.insert_atoms source.iteratorInst program.value.atoms iterator
        initialized.slice = ok (.Ok (), stored) ∧
      result = { theory := program, words := { slice := stored } } := by
  obtain ⟨count, counted, countValue⟩ := UsizeCeiling.word_count64 program.value.atoms
  rw [phases_exact, counted] at completed
  simp only [bind_tc_ok] at completed
  cases reserved : alloc.vec.Vec.try_reserve_exact Global (alloc.vec.Vec.new U64) count with
  | ret pair =>
      rcases pair with ⟨verdict, words⟩
      cases verdict with
      | Err error => simp [reserved] at completed
      | Ok accepted =>
          cases accepted
          simp [reserved] at completed
          cases resized : alloc.vec.Vec.resize core.clone.CloneU64 words count 0#u64 with
          | ret initialized =>
              simp [resized] at completed
              cases converted : source.into_iter atoms with
              | ret iterator =>
                  simp [converted] at completed
                  cases inserted : theory.insert_atoms source.iteratorInst program.value.atoms iterator
                      initialized.slice with
                  | ret pair =>
                      rcases pair with ⟨verdict, stored⟩
                      cases verdict with
                      | Err reason => simp [inserted] at completed
                      | Ok accepted =>
                          cases accepted
                          have same : { theory := program, words := { slice := stored } } = result := by
                            simpa [inserted] using completed
                          exact ⟨count, words, initialized, iterator, stored, counted,
                            countValue, reserved, resized, rfl, inserted, same.symm⟩
                  | vis effect continuation => simp [inserted] at completed
                  | div => simp [inserted] at completed
              | vis effect continuation => simp [converted] at completed
              | div => simp [converted] at completed
          | vis effect continuation => simp [resized] at completed
          | div => simp [resized] at completed
  | vis effect continuation => simp [reserved] at completed
  | div => simp [reserved] at completed

/-- Successful construction retains exactly the supplied shared theory,
including its owner and immutable data. This follows from the actual phases and
existing clone model; it is not a fresh-allocation or iterator-semantic claim. -/
theorem completed_theory
    (source : core.iter.traits.collect.IntoIterator Input Usize Iter)
    (program : theory.Theory) (atoms : Input) (result : theory.Interpretation)
    (completed : theory.Interpretation.new source program atoms = ok (.Ok result)) :
    result.theory = program := by
  obtain ⟨_, _, _, _, _, _, _, _, _, _, _, same⟩ :=
    completed_phases source program atoms result completed
  rw [same]

end NativeInterpretationConstruction
