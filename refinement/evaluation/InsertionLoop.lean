import SliceInsertion

open Aeneas Aeneas.Std Result ControlFlow
open ZetesisExtract
open Zetesis.Refinement

/-!
# The actual generic packed insertion loop

The input contract describes finite observations of the supplied iterator's
actual `next` operation. It assumes neither insertion correctness nor an output
interpretation. The generated loop is related to packed insertion by induction
on those observations, with the remaining sequence decreasing after each valid
atom. Duplicates are permitted; behavior after the first None is irrelevant.

Storage coverage is explicit. Allocation, construction of initially zero words,
owner retention and correspondence of arbitrary runtime iterator effects to the
supplied dictionary belong to their separate boundaries. An invalid observed
atom is refused without consulting the tail, even when that tail has no finite
returning contract. The generic helper does not return its final iterator, so
these laws do not expose a Rust borrow-state suffix that its result does not carry.
-/
namespace InsertionLoop

variable {Iter : Type}

/-- A finite input is justified solely by actual iterator calls. The exhausted
case stops at the first None; it imposes no fusion condition on the returned
state. The yielded case supplies one actual value and a smaller remaining list.
Neither constructor mentions packed storage or the insertion algorithm. -/
inductive FiniteInput (iterator : core.iter.traits.iterator.Iterator Iter Usize) :
    Iter → List Usize → Prop where
  | exhausted (state after : Iter)
      (observed : iterator.next state = ok (none, after)) :
      FiniteInput iterator state []
  | yielded (state after : Iter) (atom : Usize) (remaining : List Usize)
      (observed : iterator.next state = ok (some atom, after))
      (tail : FiniteInput iterator after remaining) :
      FiniteInput iterator state (atom :: remaining)

/-- Read every physical word as its exact 64-bit value, without clipping or
repairing padding and without inventing missing words. -/
def raw (words : Slice U64) : List (BitVec 64) := words.val.map UScalar.bv

/-- Each coordinate in the declared universe has a stored word. -/
def Covers (size : Usize) (words : Slice U64) : Prop :=
  ∀ atom : Nat, atom < size.val → atom / 64 < words.val.length

/-- Input values before the first invalid coordinate, in their original order. -/
def validPrefix (size : Usize) (atoms : List Usize) : List Usize :=
  atoms.takeWhile (fun atom => decide (atom.val < size.val))

/-- The finite input is admitted exactly when every supplied coordinate is in
the declared universe. Otherwise the actual loop's only source error is Atom. -/
def verdict (size : Usize) (atoms : List Usize) :
    core.result.Result Unit theory.AdmissionError :=
  if atoms.all (fun atom => decide (atom.val < size.val)) then .Ok () else .Err .Atom

/-- Observing None ends the actual body with unchanged words. No property of
the iterator's subsequent behavior is required. -/
theorem body_exhausted (iterator : core.iter.traits.iterator.Iterator Iter Usize)
    (size : Usize) (state after : Iter) (words : Slice U64)
    (observed : iterator.next state = ok (none, after)) :
    theory.insert_atoms_loop.body iterator size state words =
      ok (.done (.Ok (), words)) := by
  simp [theory.insert_atoms_loop.body, observed]

/-- The body consumes the observed invalid atom and refuses it before reading
or writing storage. The returned iterator is never passed to another next call
on this branch; no contract for that tail is needed. -/
theorem body_refused (iterator : core.iter.traits.iterator.Iterator Iter Usize)
    (size : Usize) (state after : Iter) (atom : Usize) (words : Slice U64)
    (observed : iterator.next state = ok (some atom, after))
    (outside : size.val ≤ atom.val) :
    theory.insert_atoms_loop.body iterator size state words =
      ok (.done (.Err .Atom, words)) := by
  simp [theory.insert_atoms_loop.body, observed, outside]

/-- The actual loop follows exactly its body's returned state or final result. -/
theorem loop_unfold (iterator : core.iter.traits.iterator.Iterator Iter Usize)
    (state : Iter) (size : Usize) (words : Slice U64) :
    theory.insert_atoms_loop iterator state size words = (do
      let transition ← theory.insert_atoms_loop.body iterator size state words
      match transition with
      | .done result => ok result
      | .cont (after, updated) => theory.insert_atoms_loop iterator after size updated) := by
  rw [theory.insert_atoms_loop, loop]
  dsimp only [uncurry]
  congr 1
  funext transition
  cases transition with
  | done result => rfl
  | cont result =>
      rcases result with ⟨after, updated⟩
      rfl

/-- Under storage coverage, one bounded observed atom performs exactly the
packed insertion and continues with the iterator state returned by next. The
proof uses the actual checked Slice operation block; no word-update agreement
is assumed. -/
theorem body_inserted (iterator : core.iter.traits.iterator.Iterator Iter Usize)
    (size : Usize) (state after : Iter) (atom : Usize) (words : Slice U64)
    (observed : iterator.next state = ok (some atom, after))
    (inside : atom.val < size.val) (covered : Covers size words) :
    ∃ updated,
      theory.insert_atoms_loop.body iterator size state words =
        ok (.cont (after, updated)) ∧
      updated.val.length = words.val.length ∧
      raw updated = PackedInterpretations.insert (raw words) atom.val := by
  obtain ⟨index, offset, mask, value, updated, divided, remainder, shifted,
      indexed, written, length, contents⟩ :=
    SliceInsertion.word_operations words atom (covered atom.val inside)
  have body : theory.insert_atoms_loop.body iterator size state words =
      ok (.cont (after, updated)) := by
    simp [theory.insert_atoms_loop.body, observed, Nat.not_le_of_lt inside,
      remainder, shifted, divided, indexed, lift, written]
  exact ⟨updated, body, length, contents⟩

/-- A finite actual iterator trace makes the generated loop terminate. Its
result admits exactly an entirely bounded input; otherwise it refuses the
first invalid atom. The returned words contain precisely the insertions before
that atom, retaining the original word count.

Proof: induction follows the supplied next observations. None returns the
initial words. An invalid value returns immediately without another next call.
A valid value applies the checked single-word correspondence, preserves storage
coverage, and recurses on the strictly shorter remaining sequence. -/
theorem loop_exact (iterator : core.iter.traits.iterator.Iterator Iter Usize)
    (size : Usize) (state : Iter) (atoms : List Usize) (words : Slice U64)
    (input : FiniteInput iterator state atoms) (covered : Covers size words) :
    ∃ updated,
      theory.insert_atoms_loop iterator state size words =
        ok (verdict size atoms, updated) ∧
      updated.val.length = words.val.length ∧
      raw updated = PackedInterpretations.insertAll (raw words)
        ((validPrefix size atoms).map UScalar.val) := by
  induction input generalizing words with
  | exhausted state after observed =>
      refine ⟨words, ?_, rfl, ?_⟩
      · rw [loop_unfold, body_exhausted iterator size state after words observed]
        simp [verdict]
      · simp [validPrefix, PackedInterpretations.insertAll]
  | yielded state after atom remaining observed tail induction =>
      by_cases inside : atom.val < size.val
      · obtain ⟨nextWords, body, length, contents⟩ :=
          body_inserted iterator size state after atom words observed inside covered
        have nextCovered : Covers size nextWords := by
          intro tested bounded
          rw [length]
          exact covered tested bounded
        obtain ⟨updated, rest, restLength, restContents⟩ :=
          induction nextWords nextCovered
        have result : theory.insert_atoms_loop iterator state size words =
            ok (verdict size (atom :: remaining), updated) := by
          rw [loop_unfold, body]
          simp only [bind_tc_ok]
          simpa [verdict, inside] using rest
        have finalContents : raw updated = PackedInterpretations.insertAll (raw words)
            ((validPrefix size (atom :: remaining)).map UScalar.val) := by
          rw [restContents, contents]
          simp [validPrefix, inside, PackedInterpretations.insertAll]
        exact ⟨updated, result, restLength.trans length, finalContents⟩
      · have outside : size.val ≤ atom.val := Nat.le_of_not_gt inside
        refine ⟨words, ?_, rfl, ?_⟩
        · rw [loop_unfold, body_refused iterator size state after atom words observed outside]
          simp [verdict, inside]
        · simp [validPrefix, inside, PackedInterpretations.insertAll]

/-- The actual insertion helper has exactly the generated loop's finite-input
contract. This law supplies termination and partial storage on refusal, not
merely correctness conditional on an already-successful helper call. -/
theorem insert_exact (iterator : core.iter.traits.iterator.Iterator Iter Usize)
    (size : Usize) (state : Iter) (atoms : List Usize) (words : Slice U64)
    (input : FiniteInput iterator state atoms) (covered : Covers size words) :
    ∃ updated,
      theory.insert_atoms iterator size state words =
        ok (verdict size atoms, updated) ∧
      updated.val.length = words.val.length ∧
      raw updated = PackedInterpretations.insertAll (raw words)
        ((validPrefix size atoms).map UScalar.val) := by
  exact loop_exact iterator size state atoms words input covered

/-- An actual successful helper result establishes that every supplied atom was
bounded, keeps the original storage length, and inserts the entire input. Input
admission is a conclusion: the only assumptions concern actual iterator calls
and sufficient initial storage.

Proof: compare the supplied completed result with the independently derived
finite-loop result. An Err verdict cannot equal Ok. The resulting all-bounded
fact makes the maximal valid prefix equal the whole input. -/
theorem completed_exact (iterator : core.iter.traits.iterator.Iterator Iter Usize)
    (size : Usize) (state : Iter) (atoms : List Usize) (words updated : Slice U64)
    (input : FiniteInput iterator state atoms) (covered : Covers size words)
    (completed : theory.insert_atoms iterator size state words = ok (.Ok (), updated)) :
    (∀ atom ∈ atoms, atom.val < size.val) ∧
      updated.val.length = words.val.length ∧
      raw updated = PackedInterpretations.insertAll (raw words)
        (atoms.map UScalar.val) := by
  obtain ⟨returned, actual, length, contents⟩ :=
    insert_exact iterator size state atoms words input covered
  have same : verdict size atoms = .Ok () ∧ returned = updated := by
    simpa only [Result.ok.injEq, Prod.mk.injEq] using actual.symm.trans completed
  have inside : ∀ atom ∈ atoms, atom.val < size.val := by
    have all : atoms.all (fun atom => decide (atom.val < size.val)) = true := by
      by_cases checked : atoms.all (fun atom => decide (atom.val < size.val)) = true
      · exact checked
      · simp [verdict, checked] at same
    simpa only [List.all_eq_true, decide_eq_true_eq] using all
  have entire : validPrefix size atoms = atoms := by
    have boundedPrefix (values : List Usize) :
        (∀ value ∈ values, value.val < size.val) → validPrefix size values = values := by
      induction values with
      | nil => intro _; rfl
      | cons first rest induction =>
          intro bounded
          have firstInside : first.val < size.val := bounded first (by simp)
          have restInside : ∀ value ∈ rest, value.val < size.val := by
            intro value member
            exact bounded value (by simp [member])
          simpa [validPrefix, firstInside] using
            congrArg (first :: ·) (induction restInside)
    exact boundedPrefix atoms inside
  rcases same with ⟨_, rfl⟩
  exact ⟨inside, length, by simpa only [entire] using contents⟩

/-- Even a refused finite input preserves padding when the original padding was
zero. Only the bounded prefix is written; all physical words remain present.
This also applies to admitted inputs, with duplicates and arbitrary order.

Proof: each member of takeWhile is bounded. The shared packed insertion law
therefore expresses every output bit as its old value or membership in that
bounded prefix. Neither term can set a bit outside the declared universe. -/
theorem padding_preserved (iterator : core.iter.traits.iterator.Iterator Iter Usize)
    (size : Usize) (state : Iter) (atoms : List Usize) (words : Slice U64)
    (input : FiniteInput iterator state atoms) (covered : Covers size words)
    (padding : PackedInterpretations.ZeroPadding size.val (raw words)) :
    ∃ updated,
      theory.insert_atoms iterator size state words =
        ok (verdict size atoms, updated) ∧
      PackedInterpretations.ZeroPadding size.val (raw updated) := by
  obtain ⟨updated, actual, _, contents⟩ :=
    insert_exact iterator size state atoms words input covered
  have bounded : ∀ atom ∈ (validPrefix size atoms).map UScalar.val, atom < size.val := by
    intro atom member
    obtain ⟨value, inPrefix, rfl⟩ := List.mem_map.mp member
    have prefixInside (values : List Usize) :
        ∀ value ∈ validPrefix size values, value.val < size.val := by
      induction values with
      | nil => simp [validPrefix]
      | cons first rest induction =>
          intro value member
          by_cases firstInside : first.val < size.val
          · have selected : value = first ∨ value ∈ validPrefix size rest := by
              simpa [validPrefix, firstInside] using member
            rcases selected with rfl | selected
            · exact firstInside
            · exact induction value selected
          · simp [validPrefix, firstInside] at member
    exact prefixInside atoms value inPrefix
  have stored : ∀ atom ∈ (validPrefix size atoms).map UScalar.val,
      atom / 64 < (raw words).length := by
    intro atom member
    simpa only [raw, List.length_map] using covered atom (bounded atom member)
  have exactBits := (PackedInterpretations.insertAll_exact (raw words)
    ((validPrefix size atoms).map UScalar.val) stored).2
  have finalPadding : PackedInterpretations.ZeroPadding size.val (raw updated) := by
    intro atom outside
    rw [contents, exactBits, padding atom outside]
    have absent : atom ∉ (validPrefix size atoms).map UScalar.val := by
      intro member
      exact Nat.not_lt_of_ge outside (bounded atom member)
    simp [absent]
  exact ⟨updated, actual, finalPadding⟩

end InsertionLoop
