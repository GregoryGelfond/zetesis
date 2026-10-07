import NativeFixedSearch

open Aeneas Aeneas.Std Result
open ZetesisNativeExtract
open Zetesis Zetesis.Refinement

/-!
# The returned frame of actual countermodel search

The actual query and carry calls preserve the subset's theory and storage shape,
retain limits and fixed controls, and never reduce charged work or subsets.
Induction over the existing call sequence composes those facts, including typed
stops with partially changed words. No second search algorithm or rank argument
is introduced.

These are internal returned-state laws. The public checker does not return partial
buffers or statistics when it returns a stop. No closed-form search cost, changing
runtime observation history or allocation contract follows from this frame.
-/
namespace NativeSearchFrame

open NativeCountermodelTrace NativeCountermodelSemantics NativeFixedCountermodelSearch

/-- Fields preserved by search and the two monotone accounting counters. Word
contents and counters may change; a stopped carry need not be a complete successor. -/
structure Frame (beforeSubset : theory.Interpretation) (before : oracle.Work)
    (subset : theory.Interpretation) (after : oracle.Work) : Prop where
  theory : subset.theory = beforeSubset.theory
  words : subset.words.val.length = beforeSubset.words.val.length
  limits : after.limits = before.limits
  control : after.cancellation = before.cancellation
  subsets : before.statistics.subsets.val ≤ after.statistics.subsets.val
  work : before.statistics.work.val ≤ after.statistics.work.val

/-- Sequential returned frames preserve the original owner, shape and budget
configuration, with transitive growth of the two actual counters. -/
theorem frame_trans (first middle last : theory.Interpretation)
    (before between after : oracle.Work)
    (earlier : Frame first before middle between)
    (later : Frame middle between last after) : Frame first before last after := by
  exact ⟨later.theory.trans earlier.theory, later.words.trans earlier.words,
    later.limits.trans earlier.limits, later.control.trans earlier.control,
    earlier.subsets.trans later.subsets, earlier.work.trans later.work⟩

/-- Any actual typed carry return has the independently constructed carry report.
The counter invariant establishes the call's preconditions; the return equation
only identifies which constructed outcome was returned. Typed partial results
are retained rather than mistaken for completed increments. -/
theorem returned_carry {size : Nat} (atoms : List (Fin size)) (bits : List Bool)
    (selected : Slice Usize) (state : State)
    (coordinates : selected.val.map UScalar.val = atoms.map Fin.val)
    (unique : atoms.Nodup) (invariant : Invariant atoms bits state)
    (proper : state.present.val < selected.val.length)
    (before : oracle.Work) (answer : core.result.Result Unit zetesis_cpu.cancellation.Stop)
    (updated : theory.Interpretation) (count : Usize) (after : oracle.Work)
    (returned : oracle.advance_subset selected state.subset state.present before =
      ok (answer, updated, count, after)) :
    ∃ next : List Bool, SubsetCounter.increment bits = some next ∧
      NativeSubsetCarry.Report atoms bits next state.subset before answer updated count after := by
  obtain ⟨next, actualAnswer, actualSubset, actualCount, actualWork,
    incremented, actual, report⟩ :=
    NativeSubsetCarry.advance_typed atoms bits selected state.subset state.present before
      coordinates unique invariant.width invariant.represented invariant.counted proper
  have same : (actualAnswer, actualSubset, actualCount, actualWork) =
      (answer, updated, count, after) := by
    exact Result.ok_injective (actual.symm.trans returned)
  have sameAnswer : actualAnswer = answer := by
    exact congrArg (fun result => result.1) same
  have sameSubset : actualSubset = updated := by
    exact congrArg (fun result => result.2.1) same
  have sameCount : actualCount = count := by
    exact congrArg (fun result => result.2.2.1) same
  have sameWork : actualWork = after := by
    exact congrArg (fun result => result.2.2.2) same
  exact ⟨next, incremented, sameAnswer ▸ sameSubset ▸ sameCount ▸ sameWork ▸ report⟩

/-- An actual call sequence preserves the search frame from its initial valid
counter state, including typed query and carry stops.

Proof: a query's existing receipt supplies its budget frame. The existing carry
report supplies owner, shape and accounting even on refusal; on success it also
supplies the next counter invariant. Only that success branch recurses, and its
returned frame composes with the current query and carry frames. -/
theorem calls_frame (frozen : FrozenEvaluation) {size : Nat}
    (atoms : List (Fin size)) (selected : Slice Usize)
    (coordinates : selected.val.map UScalar.val = atoms.map Fin.val)
    (unique : atoms.Nodup) (bits : List Bool) (state : State)
    (invariant : Invariant atoms bits state) (outcome : Outcome)
    (calls : Calls frozen.program frozen.values.slice selected state outcome) :
    Frame state.subset state.work outcome.subset outcome.work := by
  have queryFrame (bits : List Bool) (state : State)
      (valid : Invariant atoms bits state)
      (answer : core.result.Result Bool zetesis_cpu.cancellation.Stop)
      (output : alloc.vec.Vec Bool) (after : oracle.Work)
      (queried : oracle.check_subset frozen.program state.subset frozen.values.slice
        state.values state.work = ok (answer, output, after)) :
      Frame state.subset state.work state.subset after := by
    have stored : NativeMembership.Represented state.subset := by
      exact NativeSearchRepresentation.stored _ state.subset valid.sizeExact valid.represented
    have receipt : NativeSubsetQueryTotal.Receipt frozen.program state.values state.work output after := by
      exact NativeSubsetQueryTotal.returned_receipt frozen.program state.subset
        frozen.values.slice state.values state.work stored frozen.valid
        (frozen_covered frozen) frozen.rootsBounded answer output after queried
    have subsets : state.work.statistics.subsets.val ≤ after.statistics.subsets.val := by
      rw [receipt.subsets]
      omega
    exact ⟨rfl, rfl, receipt.limits, receipt.control, subsets, receipt.workLower⟩
  have carryFrame (bits next : List Bool) (subset updated : theory.Interpretation)
      (before after : oracle.Work) (answer : core.result.Result Unit zetesis_cpu.cancellation.Stop)
      (count : Usize)
      (report : NativeSubsetCarry.Report atoms bits next subset before answer updated count after) :
      Frame subset before updated after := by
    have held : NativeRootScan.Frame before after := by
      exact report.2.2.1
    have subsets : before.statistics.subsets.val ≤ after.statistics.subsets.val := by
      exact Nat.le_of_eq (congrArg UScalar.val held.2.2).symm
    have work : before.statistics.work.val ≤ after.statistics.work.val := by
      cases answer with
      | Ok value =>
          have charged : after.statistics.work.val = before.statistics.work.val + NativeSubsetCarry.ticks bits := by
            exact report.2.2.2.2.2
          omega
      | Err reason => exact report.2.2.2.1
    exact ⟨report.1, report.2.1, held.1, held.2.1, subsets, work⟩
  induction calls generalizing bits with
  | exhausted state full =>
      exact ⟨rfl, rfl, rfl, rfl, Nat.le_refl _, Nat.le_refl _⟩
  | queryStopped state output after reason proper queried =>
      exact queryFrame bits state invariant (.Err reason) output after queried
  | witness state output after proper queried =>
      exact queryFrame bits state invariant (.Ok true) output after queried
  | carryStopped state output middle interrupted partialCount after reason proper queried carried =>
      have query : Frame state.subset state.work state.subset middle := by
        exact queryFrame bits state invariant (.Ok false) output middle queried
      obtain ⟨nextBits, _, receipt⟩ := returned_carry atoms bits selected state
        coordinates unique invariant proper middle (.Err reason) interrupted partialCount after carried
      have carry : Frame state.subset middle interrupted after := by
        exact carryFrame bits nextBits state.subset interrupted middle after
          (.Err reason) partialCount receipt
      exact frame_trans _ _ _ _ _ _ query carry
  | next state nextState middle outcome proper queried carried rest induction =>
      have query : Frame state.subset state.work state.subset middle := by
        exact queryFrame bits state invariant (.Ok false) nextState.values middle queried
      obtain ⟨nextBits, incremented, receipt⟩ := returned_carry atoms bits selected state
        coordinates unique invariant proper middle (.Ok ()) nextState.subset nextState.present
        nextState.work carried
      have carry : Frame state.subset middle nextState.subset nextState.work := by
        exact carryFrame bits nextBits state.subset nextState.subset middle nextState.work
          (.Ok ()) nextState.present receipt
      have nextInvariant : Invariant atoms nextBits nextState := by
        refine ⟨(SubsetCounter.increment_length bits nextBits incremented).trans invariant.width,
          ?_, receipt.2.2.2.1, receipt.2.2.2.2.1⟩
        exact (congrArg (fun program => program.value.atoms.val) receipt.1).trans
          invariant.sizeExact
      have suffix : Frame nextState.subset nextState.work outcome.subset outcome.work := by
        exact induction nextBits nextInvariant
      exact frame_trans _ _ _ _ _ _ (frame_trans _ _ _ _ _ _ query carry) suffix

/-- Every actual typed search return from zero storage preserves owner, exact
word count and budget configuration, and never decreases work or subset charges.

The existing finite-search theorem constructs the call sequence. Its actual
execution identifies the returned fields; this theorem assumes neither a search
frame nor successful completion. Internal stopped state is not a public API
promise to return partial buffers or statistics on error. -/
theorem returned_frame (frozen : FrozenEvaluation) {size : Nat}
    (atoms : List (Fin size)) (selected : Slice Usize)
    (coordinates : selected.val.map UScalar.val = atoms.map Fin.val)
    (unique : atoms.Nodup) (empty : theory.Interpretation)
    (old : alloc.vec.Vec Bool) (work : oracle.Work)
    (sizeExact : empty.theory.value.atoms.val = size)
    (zero : PackedSubsets.Represents (ScalarSubsets.raw empty.words) ([] : List (Fin size)))
    (answer : core.result.Result Bool zetesis_cpu.cancellation.Stop)
    (subset : theory.Interpretation) (output : alloc.vec.Vec Bool) (after : oracle.Work)
    (returned : oracle.find_countermodel frozen.program frozen.values.slice selected empty old work =
      ok (answer, subset, output, after)) :
    Frame empty work subset after := by
  have invariant : Invariant atoms (List.replicate atoms.length false)
      ⟨empty, old, work, 0#usize⟩ := by
    refine ⟨List.length_replicate, sizeExact, ?_, ?_⟩
    · simpa only [PackedSubsets.selected_false] using zero
    · simp only [PackedSubsets.population_zero, UScalar.ofNatCore_val_eq]
  obtain ⟨outcome, calls, _⟩ := calls_refine frozen atoms selected coordinates unique
    (List.replicate atoms.length false) ⟨empty, old, work, 0#usize⟩ invariant
  have frame : Frame empty work outcome.subset outcome.work := by
    exact calls_frame frozen atoms selected coordinates unique
      (List.replicate atoms.length false) ⟨empty, old, work, 0#usize⟩ invariant outcome calls
  have actual : oracle.find_countermodel frozen.program frozen.values.slice selected empty old work =
      ok (outcome.result, outcome.subset, outcome.values, outcome.work) := by
    exact entry_executes frozen.program frozen.values.slice selected empty old work outcome calls
  have same : (outcome.result, outcome.subset, outcome.values, outcome.work) =
      (answer, subset, output, after) := by
    exact Result.ok_injective (actual.symm.trans returned)
  have sameSubset : outcome.subset = subset := by
    exact congrArg (fun result => result.2.1) same
  have sameWork : outcome.work = after := by
    exact congrArg (fun result => result.2.2.2) same
  exact sameSubset ▸ sameWork ▸ frame

end NativeSearchFrame
