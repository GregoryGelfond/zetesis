import ScalarSubsets
import SubsetSteps
import RootScan

open Aeneas Aeneas.Std Result ControlFlow
open ZetesisExtract
open Zetesis.Refinement

/-!
# The actual packed carry

The generated carry clears leading true coordinates and sets the first false
coordinate. Its successful branch uses the checked backend operations connected
to shared packed insertion and clearing by `ScalarSubsets`. The positional
population supplies the arithmetic bounds. Fixed observation tokens, the backend
sequence model and extraction remain the existing correspondence boundaries.
The bounded coordinate carrier describes raw storage; identifying its size with
the theory atom count is a separate premise of semantic composition. Equality
of the extracted theory field does not establish Rust pointer-owner identity.

The mathematical increment has no successor for an all-true input; Rust clears
that input and returns success. The successor theorem therefore applies only to
a proper positional selection. No overflow case is silently identified with a
mathematical successor.
-/
namespace SubsetCarry

/-- Setting the first false coordinate is an actual successful generated-body
step. It charges the supplied admitted tick, preserves storage length, and
performs exactly packed insertion and one checked population increment. -/
theorem body_set (iter : core.slice.iter.Iter Usize)
    (subset : theory.Interpretation) (present : Usize) (work returned : oracle.Work)
    (inside : iter.i < iter.slice.val.length)
    (stored : iter.slice.val[iter.i].val / 64 < subset.words.val.length)
    (absent : PackedInterpretations.bit64 (ScalarSubsets.raw subset.words)
      iter.slice.val[iter.i].val = false)
    (room : present.val < Usize.max)
    (ticked : oracle.Work.tick work = ok (core.result.Result.Ok (), returned)) :
    ∃ words : alloc.vec.Vec U64, ∃ count : Usize,
      oracle.advance_subset_loop.body iter subset present work =
        ok (.done (core.result.Result.Ok (), { subset with words := words }, count, returned)) ∧
      ScalarSubsets.raw words = PackedInterpretations.insert
        (ScalarSubsets.raw subset.words) iter.slice.val[iter.i].val ∧
      words.val.length = subset.words.val.length ∧ count.val = present.val + 1 := by
  obtain ⟨index, offset, mask, value, replace, divided, remainder, shifted,
    indexed, tested, inserted, _⟩ := ScalarSubsets.word_operations subset.words _ stored
  simp only [alloc.vec.Vec.index_mut_slice_index] at indexed
  have zero : value &&& mask = 0#u64 := by
    simpa [absent] using tested
  have fits : present.val + (1#usize).val ≤ Usize.max := by
    change present.val + 1 ≤ Usize.max
    omega
  obtain ⟨count, added, incremented⟩ := WP.spec_imp_exists
    (Usize.add_spec (x := present) (y := 1#usize) fits)
  have length : (replace (value ||| mask)).val.length = subset.words.val.length := by
    have same := congrArg List.length inserted
    simpa [ScalarSubsets.raw, PackedInterpretations.insert] using same
  refine ⟨replace (value ||| mask), count, ?_, inserted, length, ?_⟩
  · simp [oracle.advance_subset_loop.body,
      EvaluatorIteration.slice_next_present iter inside, ticked,
      core.result.Result.Insts.CoreOpsTry.branch, divided, indexed,
      remainder, shifted, lift, zero, added]
  · simpa using incremented

/-- Clearing a true coordinate is an actual continuing generated-body step.
The positive population prevents underflow. Its output retains the updated
words and decremented count for the next actual loop iteration. -/
theorem body_clear (iter : core.slice.iter.Iter Usize)
    (subset : theory.Interpretation) (present : Usize) (work returned : oracle.Work)
    (inside : iter.i < iter.slice.val.length)
    (stored : iter.slice.val[iter.i].val / 64 < subset.words.val.length)
    (presentBit : PackedInterpretations.bit64 (ScalarSubsets.raw subset.words)
      iter.slice.val[iter.i].val = true)
    (positive : 0 < present.val)
    (ticked : oracle.Work.tick work = ok (core.result.Result.Ok (), returned)) :
    ∃ words : alloc.vec.Vec U64, ∃ count : Usize,
      oracle.advance_subset_loop.body iter subset present work =
        ok (.cont ({ iter with i := iter.i + 1 },
          { subset with words := words }, count, returned)) ∧
      ScalarSubsets.raw words = PackedSubsets.clear
        (ScalarSubsets.raw subset.words) iter.slice.val[iter.i].val ∧
      words.val.length = subset.words.val.length ∧ count.val = present.val - 1 := by
  obtain ⟨index, offset, mask, value, replace, divided, remainder, shifted,
    indexed, tested, _, cleared⟩ := ScalarSubsets.word_operations subset.words _ stored
  simp only [alloc.vec.Vec.index_mut_slice_index] at indexed
  have nonzero : value &&& mask ≠ 0#u64 := by
    simpa [presentBit] using tested
  have fits : (1#usize).val ≤ present.val := by
    change 1 ≤ present.val
    omega
  obtain ⟨count, subtracted, decremented, _⟩ := WP.spec_imp_exists
    (Usize.sub_spec (x := present) (y := 1#usize) fits)
  have length : (replace (value &&& ~~~mask)).val.length = subset.words.val.length := by
    have same := congrArg List.length cleared
    simpa [ScalarSubsets.raw, PackedSubsets.clear] using same
  refine ⟨replace (value &&& ~~~mask), count, ?_, cleared, length, ?_⟩
  · simp [oracle.advance_subset_loop.body,
      EvaluatorIteration.slice_next_present iter inside, ticked,
      core.result.Result.Insts.CoreOpsTry.branch, divided, indexed,
      remainder, shifted, lift, nonzero, subtracted]
  · simpa using decremented

/-- Unfolding the actual loop threads the complete iterator, packed storage,
population and work returned by the actual body. -/
theorem loop_unfold (iter : core.slice.iter.Iter Usize)
    (subset : theory.Interpretation) (present : Usize) (work : oracle.Work) :
    oracle.advance_subset_loop iter subset present work = (do
      let transition ← oracle.advance_subset_loop.body iter subset present work
      match transition with
      | .done result => ok result
      | .cont (next, updated, count, returned) =>
          oracle.advance_subset_loop next updated count returned) := by
  rw [oracle.advance_subset_loop, loop]
  dsimp only [uncurry]
  congr 1
  funext transition
  cases transition with
  | done result => rfl
  | cont result =>
    obtain ⟨next, updated, count, returned⟩ := result
    rfl


/-- Number of charged coordinates through the first false bit. For a proper
selection it is positive and no larger than the number of selected positions. -/
def ticks : List Bool → Nat
  | [] => 0
  | false :: _ => 1
  | true :: tail => 1 + ticks tail

/-- A refused next tick stops the whole actual carry loop without modifying its
current words, population or work. This statement also applies to a state reached
after earlier clears: those changes are retained, not rolled back or identified
with a completed counter successor. No storage premise is needed for the unread
word. The iterator must still have a coordinate to visit. -/
theorem loop_refused (iter : core.slice.iter.Iter Usize)
    (subset : theory.Interpretation) (present : Usize) (work : oracle.Work)
    (reason : zetesis_cpu.cancellation.Stop) (inside : iter.i < iter.slice.val.length)
    (refused : EvaluatorControl.observation work.cancellation = some reason ∨
      (EvaluatorControl.observation work.cancellation = none ∧
        work.limits.max_work.val ≤ work.statistics.work.val ∧ reason = .WorkLimit)) :
    oracle.advance_subset_loop iter subset present work =
      ok (core.result.Result.Err reason, subset, present, work) := by
  rw [loop_unfold, SubsetSteps.carry_refused iter subset present work reason inside refused]
  simp only [bind_tc_ok]


/-- A returned carry retains the theory field and word-array length. A completed
result has the positional successor and exact work; a typed stop is at the next
refused tick after fewer visits, with the actual partial words/count retained.
No completed population invariant is asserted for the stopped state. -/
def Report {size : Nat} (atoms : List (Fin size)) (bits next : List Bool)
    (beforeSubset : theory.Interpretation) (beforeWork : oracle.Work)
    (answer : core.result.Result Unit zetesis_cpu.cancellation.Stop)
    (updated : theory.Interpretation) (count : Usize) (returned : oracle.Work) : Prop :=
  updated.theory = beforeSubset.theory ∧
  updated.words.val.length = beforeSubset.words.val.length ∧
  FixedRootScan.Frame beforeWork returned ∧
  match answer with
  | .Ok _ =>
      PackedSubsets.Represents (ScalarSubsets.raw updated.words)
        (Zetesis.SubsetCounter.selected atoms next) ∧
      count.val = Zetesis.SubsetCounter.population next ∧
      returned.statistics.work.val = beforeWork.statistics.work.val + ticks bits
  | .Err reason =>
      beforeWork.statistics.work.val ≤ returned.statistics.work.val ∧
      returned.statistics.work.val < beforeWork.statistics.work.val + ticks bits ∧
      oracle.Work.tick returned = ok (core.result.Result.Err reason, returned)

/-- Every proper carry returns either its exact successor or a typed stop.
This is constructive execution of the actual generated loop without a control
or work-admission assumption. A stop retains the actual partially cleared state;
only the theory field, storage shape and the work frame are carried through that branch.

Proof: derive the next tick from the supplied controls and allowance. Refusal
finishes with unchanged current state. Otherwise set a false coordinate or clear
a true one and recurse on the shorter suffix. The nonoverflowing mathematical
increment excludes reaching exhaustion after clearing the entire input.
-/
theorem loop_typed {size : Nat} (atoms : List (Fin size)) (bits next : List Bool)
    (iter : core.slice.iter.Iter Usize) (subset : theory.Interpretation)
    (present : Usize) (work : oracle.Work)
    (remaining : (iter.slice.val.drop iter.i).map UScalar.val = atoms.map Fin.val)
    (unique : atoms.Nodup) (width : bits.length = atoms.length)
    (fits : atoms.length ≤ Usize.max)
    (represented : PackedSubsets.Represents (ScalarSubsets.raw subset.words)
      (Zetesis.SubsetCounter.selected atoms bits))
    (counted : present.val = Zetesis.SubsetCounter.population bits)
    (advanced : Zetesis.SubsetCounter.increment bits = some next) :
    ∃ answer : core.result.Result Unit zetesis_cpu.cancellation.Stop,
      ∃ updated : theory.Interpretation, ∃ count : Usize, ∃ returned : oracle.Work,
      oracle.advance_subset_loop iter subset present work = ok (answer, updated, count, returned) ∧
      Report atoms bits next subset work answer updated count returned := by
  induction atoms generalizing bits next iter subset present work with
  | nil =>
      cases bits with
      | nil => simp [Zetesis.SubsetCounter.increment] at advanced
      | cons bit tail => simp at width
  | cons atom rest ih =>
      cases bits with
      | nil => simp at width
      | cons bit tail =>
          have inside : iter.i < iter.slice.val.length := by
            by_contra outside
            have empty : iter.slice.val.drop iter.i = [] :=
              List.drop_eq_nil_of_le (Nat.le_of_not_gt outside)
            simp [empty] at remaining
          have coordinates : iter.slice.val[iter.i].val = atom.val ∧
              (iter.slice.val.drop (iter.i + 1)).map UScalar.val = rest.map Fin.val := by
            rw [List.drop_eq_getElem_cons inside, List.map_cons, List.map_cons] at remaining
            exact List.cons.inj remaining
          have tailWidth : tail.length = rest.length := Nat.succ.inj width
          have tailUnique : rest.Nodup := (List.nodup_cons.mp unique).2
          have absent := PackedSubsets.absent_selected atom rest tail
            (List.nodup_cons.mp unique).1 tailWidth
          have stored : iter.slice.val[iter.i].val / 64 < subset.words.val.length := by
            rw [coordinates.1]
            have covered := PackedSubsets.coordinate_stored (ScalarSubsets.raw subset.words)
              (Zetesis.SubsetCounter.selected (atom :: rest) (bit :: tail)) represented atom
            simpa [ScalarSubsets.raw] using covered
          have halt (reason : zetesis_cpu.cancellation.Stop)
              (stopped : oracle.Work.tick work = ok (core.result.Result.Err reason, work)) :
              ∃ answer : core.result.Result Unit zetesis_cpu.cancellation.Stop,
                ∃ updated : theory.Interpretation, ∃ count : Usize, ∃ returned : oracle.Work,
                oracle.advance_subset_loop iter subset present work = ok (answer, updated, count, returned) ∧
                Report (atom :: rest) (bit :: tail) next subset work answer updated count returned := by
            refine ⟨.Err reason, subset, present, work, ?_, rfl, rfl,
              by simp [FixedRootScan.Frame], Nat.le_refl _, ?_, stopped⟩
            · rw [loop_unfold]
              simp [oracle.advance_subset_loop.body,
                EvaluatorIteration.slice_next_present iter inside, stopped,
                core.result.Result.Insts.CoreOpsTry.branch,
                core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]
            · cases bit <;> simp [ticks]
          cases observed : EvaluatorControl.observation work.cancellation with
          | some reason => exact halt reason (EvaluatorControl.tick_stopped work reason observed)
          | none =>
              by_cases room : work.statistics.work.val < work.limits.max_work.val
              · obtain ⟨charged, incremented, ticked⟩ :=
                  EvaluatorControl.tick_advances work observed room
                let middle : oracle.Work :=
                  { work with statistics := { work.statistics with work := charged } }
                have frame : FixedRootScan.Frame work middle := by simp [FixedRootScan.Frame, middle]
                cases bit with
                | false =>
                    have current : PackedSubsets.Represents (ScalarSubsets.raw subset.words)
                        (Zetesis.SubsetCounter.selected rest tail) := represented
                    have bitFalse : PackedInterpretations.bit64 (ScalarSubsets.raw subset.words)
                        iter.slice.val[iter.i].val = false := by
                      rw [coordinates.1, PackedSubsets.bit_member _ _ current]
                      simp [absent]
                    have populationRoom : present.val < Usize.max := by
                      have bound := Zetesis.SubsetCounter.population_bound tail
                      simp only [Zetesis.SubsetCounter.population] at counted
                      simp only [List.length_cons] at fits
                      omega
                    obtain ⟨words, count, written, inserted, length, countValue⟩ :=
                      body_set iter subset present work middle inside stored bitFalse populationRoom ticked
                    have same : true :: tail = next := Option.some.inj advanced
                    subst next
                    have outputRepresented : PackedSubsets.Represents (ScalarSubsets.raw words)
                        (Zetesis.SubsetCounter.selected (atom :: rest) (true :: tail)) := by
                      rw [inserted, coordinates.1]
                      exact PackedSubsets.insert_represents _ _ current atom
                    refine ⟨.Ok (), { subset with words := words }, count, middle, ?_, rfl,
                      length, frame, outputRepresented, ?_, ?_⟩
                    · rw [loop_unfold, written]
                      simp only [bind_tc_ok]
                    · simpa [Zetesis.SubsetCounter.population, counted] using countValue
                    · simpa [middle, ticks] using incremented
                | true =>
                    have current : PackedSubsets.Represents (ScalarSubsets.raw subset.words)
                        (atom :: Zetesis.SubsetCounter.selected rest tail) := represented
                    have bitTrue : PackedInterpretations.bit64 (ScalarSubsets.raw subset.words)
                        iter.slice.val[iter.i].val = true := by
                      rw [coordinates.1, PackedSubsets.bit_member _ _ current]
                      simp
                    have positive : 0 < present.val := by
                      simp only [Zetesis.SubsetCounter.population] at counted
                      omega
                    obtain ⟨words, count, written, cleared, length, countValue⟩ :=
                      body_clear iter subset present work middle inside stored bitTrue positive ticked
                    have outputRepresented : PackedSubsets.Represents (ScalarSubsets.raw words)
                        (Zetesis.SubsetCounter.selected rest tail) := by
                      rw [cleared, coordinates.1]
                      exact PackedSubsets.clear_represents _ atom _ current absent
                    have outputCount : count.val = Zetesis.SubsetCounter.population tail := by
                      simp only [Zetesis.SubsetCounter.population] at counted
                      omega
                    cases carried : Zetesis.SubsetCounter.increment tail with
                    | none => simp [Zetesis.SubsetCounter.increment, carried] at advanced
                    | some successor =>
                        have same : false :: successor = next := by
                          simpa only [Zetesis.SubsetCounter.increment, carried, Option.map_some,
                            Option.some.injEq] using advanced
                        subst next
                        obtain ⟨answer, updated, finalCount, returned, executed,
                          theoryFrame, shape, workFrame, result⟩ :=
                          ih tail successor { iter with i := iter.i + 1 }
                            { subset with words := words } count middle coordinates.2 tailUnique
                            tailWidth (by simp only [List.length_cons] at fits; omega)
                            outputRepresented outputCount carried
                        refine ⟨answer, updated, finalCount, returned, ?_, theoryFrame,
                          shape.trans length, ?_, ?_⟩
                        · rw [loop_unfold, written]
                          simpa only [bind_tc_ok] using executed
                        · exact ⟨workFrame.1.trans frame.1, workFrame.2.1.trans frame.2.1,
                            workFrame.2.2.trans frame.2.2⟩
                        · cases answer with
                          | Ok value =>
                              obtain ⟨finalRepresentation, finalPopulation, workCount⟩ := result
                              refine ⟨finalRepresentation, finalPopulation, ?_⟩
                              change returned.statistics.work.val = work.statistics.work.val + (1 + ticks tail)
                              change returned.statistics.work.val = charged.val + ticks tail at workCount
                              omega
                          | Err reason =>
                              obtain ⟨lower, upper, stopped⟩ := result
                              refine ⟨?_, ?_, stopped⟩
                              · change charged.val ≤ returned.statistics.work.val at lower
                                omega
                              · change returned.statistics.work.val < work.statistics.work.val + (1 + ticks tail)
                                change returned.statistics.work.val < charged.val + ticks tail at upper
                                omega
              · exact halt .WorkLimit (EvaluatorControl.tick_at_limit work observed (Nat.le_of_not_gt room))

/-- The complete actual carry helper always produces a typed result for a proper
represented positional selection. Success establishes the next counter state;
a stop preserves the theory field, word shape and work frame while retaining
the actual partially updated output. No work or control success is assumed. -/
theorem advance_typed {size : Nat} (atoms : List (Fin size)) (bits : List Bool)
    (selected : Slice Usize) (subset : theory.Interpretation)
    (present : Usize) (work : oracle.Work)
    (coordinates : selected.val.map UScalar.val = atoms.map Fin.val)
    (unique : atoms.Nodup) (width : bits.length = atoms.length)
    (represented : PackedSubsets.Represents (ScalarSubsets.raw subset.words)
      (Zetesis.SubsetCounter.selected atoms bits))
    (counted : present.val = Zetesis.SubsetCounter.population bits)
    (proper : present.val < selected.val.length) :
    ∃ next : List Bool, ∃ answer : core.result.Result Unit zetesis_cpu.cancellation.Stop,
      ∃ updated : theory.Interpretation, ∃ count : Usize, ∃ returned : oracle.Work,
      Zetesis.SubsetCounter.increment bits = some next ∧
      oracle.advance_subset selected subset present work = ok (answer, updated, count, returned) ∧
      Report atoms bits next subset work answer updated count returned := by
  have sameLength : selected.val.length = atoms.length := by
    simpa only [List.length_map] using congrArg List.length coordinates
  have notFull : Zetesis.SubsetCounter.full bits ≠ true := by
    apply (Zetesis.SubsetCounter.guard_exact bits present.val counted).mp
    simpa only [width, sameLength] using proper
  have nextExists : ∃ next, Zetesis.SubsetCounter.increment bits = some next := by
    cases advanced : Zetesis.SubsetCounter.increment bits with
    | none => exact False.elim (notFull ((Zetesis.SubsetCounter.increment_none_iff bits).mp advanced))
    | some next => exact ⟨next, rfl⟩
  obtain ⟨next, advanced⟩ := nextExists
  have fits : atoms.length ≤ Usize.max := by
    rw [← sameLength]
    exact Slice.length_ineq selected
  obtain ⟨answer, updated, count, returned, executed, report⟩ :=
    loop_typed atoms bits next { slice := selected, i := 0 } subset present work
      (by simpa using coordinates) unique width fits represented counted advanced
  refine ⟨next, answer, updated, count, returned, advanced, ?_, report⟩
  simpa [oracle.advance_subset,
    SharedSlice.Insts.CoreIterTraitsCollectIntoIteratorSharedIter.into_iter] using executed

/-- A nonoverflowing positional increment is implemented by the actual carry
loop, with exact packed truth, population and charged work. The iterator's
remaining coordinates must be the distinct bounded positions of the counter.
Clear supplied controls and enough work for this carry establish completion;
no successful body or loop equation is assumed.

Proof: use the all-typed execution theorem. A refused tick would occur before
the required carry work is consumed; the supplied allowance and clear controls
then establish that the same tick succeeds, a contradiction.
-/
theorem loop_success {size : Nat} (atoms : List (Fin size)) (bits next : List Bool)
    (iter : core.slice.iter.Iter Usize) (subset : theory.Interpretation)
    (present : Usize) (work : oracle.Work)
    (remaining : (iter.slice.val.drop iter.i).map UScalar.val = atoms.map Fin.val)
    (unique : atoms.Nodup) (width : bits.length = atoms.length)
    (fits : atoms.length ≤ Usize.max)
    (represented : PackedSubsets.Represents (ScalarSubsets.raw subset.words)
      (Zetesis.SubsetCounter.selected atoms bits))
    (counted : present.val = Zetesis.SubsetCounter.population bits)
    (advanced : Zetesis.SubsetCounter.increment bits = some next)
    (clear : EvaluatorControl.observation work.cancellation = none)
    (allowance : work.statistics.work.val + ticks bits ≤ work.limits.max_work.val) :
    ∃ updated : theory.Interpretation, ∃ count : Usize, ∃ returned : oracle.Work,
      oracle.advance_subset_loop iter subset present work =
        ok (core.result.Result.Ok (), updated, count, returned) ∧
      PackedSubsets.Represents (ScalarSubsets.raw updated.words)
        (Zetesis.SubsetCounter.selected atoms next) ∧
      count.val = Zetesis.SubsetCounter.population next ∧
      updated.theory = subset.theory ∧
      FixedRootScan.Frame work returned ∧
      returned.statistics.work.val = work.statistics.work.val + ticks bits := by
  obtain ⟨answer, updated, count, returned, executed,
    theoryFrame, _, workFrame, report⟩ :=
    loop_typed atoms bits next iter subset present work remaining unique width fits
      represented counted advanced
  cases answer with
  | Ok value =>
      cases value
      exact ⟨updated, count, returned, executed, report.1, report.2.1,
        theoryFrame, workFrame, report.2.2⟩
  | Err reason =>
      obtain ⟨_, upper, stopped⟩ := report
      have controlsClear : EvaluatorControl.observation returned.cancellation = none := by
        rw [workFrame.2.1]
        exact clear
      have room : returned.statistics.work.val < returned.limits.max_work.val := by
        rw [workFrame.1]
        omega
      obtain ⟨_, _, ticked⟩ := EvaluatorControl.tick_advances returned controlsClear room
      rw [stopped] at ticked
      have impossible := Result.ok_injective ticked
      simp at impossible

/-- The actual helper, on a proper positional selection, computes the next
binary-counter state. Distinct bounded coordinates and exact raw representation
supply word safety and zero padding. The population guard rules out the all-true
case where the Rust carry wraps but the mathematical increment reports overflow.
The admitted slice supplies the machine bound on the number of positions.

The conclusion includes the new represented subset, exact population and exact
carry work, retaining the theory field and all other work fields. The work premise
is exactly the number of coordinates this carry visits, not the whole universe.
-/
theorem advance_success {size : Nat} (atoms : List (Fin size)) (bits : List Bool)
    (selected : Slice Usize) (subset : theory.Interpretation)
    (present : Usize) (work : oracle.Work)
    (coordinates : selected.val.map UScalar.val = atoms.map Fin.val)
    (unique : atoms.Nodup) (width : bits.length = atoms.length)
    (represented : PackedSubsets.Represents (ScalarSubsets.raw subset.words)
      (Zetesis.SubsetCounter.selected atoms bits))
    (counted : present.val = Zetesis.SubsetCounter.population bits)
    (proper : present.val < selected.val.length)
    (clear : EvaluatorControl.observation work.cancellation = none)
    (allowance : work.statistics.work.val + ticks bits ≤ work.limits.max_work.val) :
    ∃ next : List Bool, ∃ updated : theory.Interpretation,
      ∃ count : Usize, ∃ returned : oracle.Work,
      Zetesis.SubsetCounter.increment bits = some next ∧
      oracle.advance_subset selected subset present work =
        ok (core.result.Result.Ok (), updated, count, returned) ∧
      PackedSubsets.Represents (ScalarSubsets.raw updated.words)
        (Zetesis.SubsetCounter.selected atoms next) ∧
      count.val = Zetesis.SubsetCounter.population next ∧
      updated.theory = subset.theory ∧
      FixedRootScan.Frame work returned ∧
      returned.statistics.work.val = work.statistics.work.val + ticks bits := by
  have sameLength : selected.val.length = atoms.length := by
    simpa only [List.length_map] using congrArg List.length coordinates
  have notFull : Zetesis.SubsetCounter.full bits ≠ true := by
    apply (Zetesis.SubsetCounter.guard_exact bits present.val counted).mp
    simpa only [width, sameLength] using proper
  have nextExists : ∃ next, Zetesis.SubsetCounter.increment bits = some next := by
    cases advanced : Zetesis.SubsetCounter.increment bits with
    | none => exact False.elim (notFull ((Zetesis.SubsetCounter.increment_none_iff bits).mp advanced))
    | some next => exact ⟨next, rfl⟩
  obtain ⟨next, advanced⟩ := nextExists
  have fits : atoms.length ≤ Usize.max := by
    rw [← sameLength]
    exact Slice.length_ineq selected
  obtain ⟨updated, count, returned, executed, result⟩ :=
    loop_success atoms bits next { slice := selected, i := 0 } subset present work
      (by simpa using coordinates) unique width fits represented counted advanced clear allowance
  refine ⟨next, updated, count, returned, advanced, ?_, result⟩
  simpa [oracle.advance_subset,
    SharedSlice.Insts.CoreIterTraitsCollectIntoIteratorSharedIter.into_iter] using executed

end SubsetCarry
