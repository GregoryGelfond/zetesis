import Membership
import Iteration
import Aeneas.Std.RangeIter
import Zetesis.PackedSubsets

open Aeneas Aeneas.Std Result
open ZetesisExtract

/-!
# Selected-atom prefix preservation through library primitives

The source selected-atom scan uses a `usize` range, packed membership and vector
push. These laws compose those concrete imported operations around its prefix
invariant. The extracted membership function is reused unchanged. The selected-
atom helper is also extracted, but this file proves its constituent operations,
not execution of that helper or the whole `oracle::check` function.

The range ends at the candidate's declared universe. Represented storage
justifies membership reads; no padding premise is needed because the scan never
reads a coordinate outside that universe and never writes candidate words.
The vector result is conditional on the imported vector model, not a claim that
Rust allocation cannot fail. Work ticks and interruption remain separate from
this primitive composition.
-/
namespace SelectedAtoms

/-- The selected vector contains, in source order, exactly the true atoms below
    the next range coordinate. It carries no extra semantic membership cache. -/
def SelectedPrefix (candidate : theory.Interpretation) (count : Nat)
    (selected : alloc.vec.Vec Usize) : Prop :=
  selected.val.map UScalar.val = (List.range count).filter (Membership.denotes candidate)

/-- There cannot be more selected atoms than visited universe coordinates.
    This bound supplies room for a push whenever another coordinate remains. -/
theorem selected_length (candidate : theory.Interpretation) (count : Nat)
    (selected : alloc.vec.Vec Usize) (prefixExact : SelectedPrefix candidate count selected) :
    selected.val.length ≤ count := by
  have filteredLength :
      ((List.range count).filter (Membership.denotes candidate)).length ≤ count := by
    simpa only [List.length_range] using
      List.length_filter_le (Membership.denotes candidate) (List.range count)
  have sameLength : selected.val.length =
      ((List.range count).filter (Membership.denotes candidate)).length := by
    simpa only [List.length_map] using congrArg List.length prefixExact
  exact sameLength ▸ filteredLength

/-- A represented prefix is strictly increasing, so repeated selected atom IDs
    cannot be introduced by the primitive transition. -/
theorem selected_ordered (candidate : theory.Interpretation) (count : Nat)
    (selected : alloc.vec.Vec Usize) (prefixExact : SelectedPrefix candidate count selected) :
    (selected.val.map UScalar.val).Pairwise (· < ·) := by
  rw [prefixExact]
  exact List.pairwise_lt_range.filter _

/-- Strictly increasing selected IDs are distinct. -/
theorem selected_nodup (candidate : theory.Interpretation) (count : Nat)
    (selected : alloc.vec.Vec Usize) (prefixExact : SelectedPrefix candidate count selected) :
    (selected.val.map UScalar.val).Nodup := by
  exact (selected_ordered candidate count selected prefixExact).imp Nat.ne_of_lt

/-- From an in-range prefix, the actual range primitive advances by one, the
    generated membership query returns the packed bit, and the chosen push or
    unchanged-vector branch preserves the exact selected prefix. Room for the
    checked successor and push follows from the remaining range coordinate;
    neither successful operation is supplied as a caller assumption. -/
theorem advance (candidate : theory.Interpretation)
    (range : core.ops.range.Range Usize) (selected : alloc.vec.Vec Usize)
    (sameUniverse : range.end = candidate.theory.value.atoms)
    (represented : Membership.Represented candidate)
    (prefixExact : SelectedPrefix candidate range.start.val selected)
    (inside : range.start.val < range.end.val) :
    ∃ next : core.ops.range.Range Usize, ∃ result : alloc.vec.Vec Usize,
      core.iter.range.IteratorRange.next core.iter.range.StepUsize range =
        ok (some range.start, next) ∧
      next.start.val = range.start.val + 1 ∧ next.end = candidate.theory.value.atoms ∧
      theory.Interpretation.contains candidate range.start =
        ok (Membership.denotes candidate range.start.val) ∧
      (if Membership.denotes candidate range.start.val
        then alloc.vec.Vec.push selected range.start else ok selected) = ok result ∧
      SelectedPrefix candidate next.start.val result := by
  have cloneExact : ∀ value : Usize, core.clone.CloneUsize.clone value = ok value := by
    intro value
    rfl
  have comparisonExact : ∀ first second : Usize,
      core.cmp.PartialOrdUsize.lt first second = ok (decide (first.val < second.val)) := by
    intro first second
    rfl
  obtain ⟨⟨atom, next⟩, rangeStep, returned, increment, endSame⟩ := WP.spec_imp_exists
    (core.iter.range.IteratorRange.next_UScalar_some_spec cloneExact comparisonExact range inside)
  have membership : theory.Interpretation.contains candidate range.start =
      ok (Membership.denotes candidate range.start.val) :=
    Membership.contains_refines candidate range.start represented
  have lengthBound : selected.val.length ≤ range.start.val :=
    selected_length candidate range.start.val selected prefixExact
  have pushRoom : selected.val.length < Usize.max := by
    have endFits : range.end.val ≤ Usize.max := by scalar_tac
    omega
  have nextPrefix : (List.range next.start.val).filter (Membership.denotes candidate) =
      (List.range range.start.val).filter (Membership.denotes candidate) ++
        (if Membership.denotes candidate range.start.val then [range.start.val] else []) := by
    rw [increment, List.range_succ, List.filter_append]
    cases value : Membership.denotes candidate range.start.val <;> simp [value]
  cases bit : Membership.denotes candidate range.start.val with
  | false =>
      refine ⟨next, selected, ?_, increment, endSame.trans sameUniverse, ?_, ?_, ?_⟩
      · simpa only [returned] using rangeStep
      · simpa only [bit] using membership
      · rfl
      · change selected.val.map UScalar.val = _
        rw [nextPrefix, bit, if_neg Bool.false_ne_true, List.append_nil]
        exact prefixExact
  | true =>
      obtain ⟨result, append, appended⟩ := EvaluatorIteration.vector_append selected range.start pushRoom
      refine ⟨next, result, ?_, increment, endSame.trans sameUniverse, ?_, ?_, ?_⟩
      · simpa only [returned] using rangeStep
      · simpa only [bit] using membership
      · exact append
      · change result.val.map UScalar.val = _
        rw [appended, List.map_append, prefixExact, nextPrefix, bit]
        rfl

/-- Scalar-word denotation agrees with the general packed semantic library.
    This is a representation equation, independent of safe-read admission. -/
theorem packed_denotation (candidate : theory.Interpretation) (atom : Nat) :
    Membership.denotes candidate atom =
      Zetesis.Refinement.PackedInterpretations.contains candidate.theory.value.atoms.val
        (candidate.words.val.map UScalar.bv) atom := by
  rw [Zetesis.Refinement.PackedInterpretations.contains_eq_bit]
  unfold Membership.denotes Zetesis.Refinement.PackedInterpretations.bit64
    Zetesis.Refinement.PackedInterpretations.word
  simp only [List.getElem?_map]
  cases stored : candidate.words.val[atom / 64]? <;> simp

/-- At the universe boundary, the selected prefix is exactly the existing
    semantic producer's ascending atom list. This reuses its selection meaning
    without adding an assumed coverage premise or another scan algorithm. -/
theorem completed_selection (candidate : theory.Interpretation)
    (selected : alloc.vec.Vec Usize)
    (prefixExact : SelectedPrefix candidate candidate.theory.value.atoms.val selected) :
    selected.val.map UScalar.val =
      (Zetesis.Refinement.PackedSubsets.selectedAtoms candidate.theory.value.atoms.val
        (candidate.words.val.map UScalar.bv)).map Fin.val := by
  have universeList : (List.finRange candidate.theory.value.atoms.val).map Fin.val =
      List.range candidate.theory.value.atoms.val := by
    apply List.ext_getElem (by simp)
    intro index leftBound rightBound
    simp
  rw [prefixExact, ← universeList, List.filter_map]
  apply congrArg (List.map Fin.val)
  apply List.filter_congr
  intro atom _
  exact packed_denotation candidate atom.val

/-- An exhausted range returns no coordinate and leaves its cursor unchanged;
    an exact prefix at that point already has the complete semantic selection.
    No membership read or vector push is required. -/
theorem exhausted (candidate : theory.Interpretation)
    (range : core.ops.range.Range Usize) (selected : alloc.vec.Vec Usize)
    (sameUniverse : range.end = candidate.theory.value.atoms)
    (atEnd : range.start = range.end)
    (prefixExact : SelectedPrefix candidate range.start.val selected) :
    core.iter.range.IteratorRange.next core.iter.range.StepUsize range = ok (none, range) ∧
      selected.val.map UScalar.val =
        (Zetesis.Refinement.PackedSubsets.selectedAtoms candidate.theory.value.atoms.val
          (candidate.words.val.map UScalar.bv)).map Fin.val := by
  have comparisonExact : ∀ first second : Usize,
      core.cmp.PartialOrdUsize.lt first second = ok (decide (first.val < second.val)) := by
    intro first second
    rfl
  obtain ⟨⟨atom, next⟩, rangeStep, absent, unchanged⟩ := WP.spec_imp_exists
    (core.iter.range.IteratorRange.next_UScalar_none_spec
      (cloneInst := core.clone.CloneUsize) comparisonExact range (by simp [atEnd]))
  constructor
  · simpa only [absent, unchanged] using rangeStep
  · apply completed_selection candidate selected
    simpa only [atEnd, sameUniverse] using prefixExact

end SelectedAtoms
