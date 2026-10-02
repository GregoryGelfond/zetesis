import Zetesis.PackedInterpretations

/-!
# Checked iteration over numeric packed-word halves

This authored cursor algorithm exports numeric low and high 32-bit halves of
immutable 64-bit words. It refines `PackedInterpretations.export32`, deriving
individual returned values and the remaining list after any admitted sequence
of steps. A checked missing-word result is distinct from normal exhaustion.

The cursor invariant and exact input storage discharge that extra failure
branch. Construction and export padding are reused from PackedInterpretations;
no premise assumes that the iterator returns the intended list. This is a
representation refinement, not a Rust extraction: borrowing, exact theory
ownership, byte conversion, allocation and machine-sized arithmetic remain
implementation obligations for `InterpretationWords::next` in
`zetesis-ferraris/src/theory.rs`.
-/

namespace Zetesis.Refinement.PackedWordIterator

open PackedInterpretations

/-- The next half index and the fixed exclusive endpoint. -/
structure Cursor where
  next : Nat
  stop : Nat
  deriving DecidableEq, Repr

/-- Initialization records the declared export width, including zero for an
    empty atom universe. The underlying words remain an immutable argument. -/
def start (size : Nat) : Cursor := ⟨0, count32 size⟩

/-- The cursor has not passed its fixed endpoint, which is the exact number of
    32-bit words covering the declared atom universe. -/
def Invariant (size : Nat) (cursor : Cursor) : Prop :=
  cursor.next ≤ cursor.stop ∧ cursor.stop = count32 size

/-- The number of halves not yet returned. Under the cursor invariant this
    subtraction is exact, rather than truncated through an invalid position. -/
def remaining (cursor : Cursor) : Nat := cursor.stop - cursor.next

/-- Advancing changes only the next position. It is used only after a return. -/
def advance (cursor : Cursor) : Cursor := ⟨cursor.next + 1, cursor.stop⟩

/-- Normal exhaustion and malformed short storage are distinct outcomes. -/
inductive Outcome where
  | exhausted
  | missingWord
  | yielded (value : BitVec 32)
  deriving DecidableEq, Repr

/-- Check exhaustion by equality, then check the containing word before
    extracting its numeric half. Only a successful return advances the cursor.
    The checked lookup makes the storage obligation visible in the algorithm. -/
def step (words : List (BitVec 64)) (cursor : Cursor) : Outcome × Cursor :=
  if cursor.next = cursor.stop then (.exhausted, cursor)
  else match words[cursor.next / 2]? with
    | none => (.missingWord, cursor)
    | some value =>
      (.yielded ((value >>> ((cursor.next % 2) * 32)).setWidth 32), advance cursor)

/-- Initialization establishes the cursor invariant without a premise about
    the storage, which is checked separately when a word is read. -/
theorem start_invariant (size : Nat) : Invariant size (start size) := by
  simp [Invariant, start]

/-- Exhaustion is exactly endpoint equality, and it leaves the cursor intact.
    Even outside the invariant, a missing word cannot masquerade as exhaustion. -/
theorem exhausted_iff (words : List (BitVec 64)) (cursor : Cursor) :
    step words cursor = (.exhausted, cursor) ↔ cursor.next = cursor.stop := by
  by_cases finished : cursor.next = cursor.stop
  · simp [step, finished]
  · simp only [step, finished, ↓reduceIte]
    cases words[cursor.next / 2]? <;> simp

/-- Before the endpoint, valid storage contains the addressed 64-bit word.
    The checked algorithm therefore returns the specified numeric half and
    advances once. No zero-padding premise is needed for this access law. -/
theorem step_active (size : Nat) (words : List (BitVec 64)) (cursor : Cursor)
    (stored : words.length = count64 size) (valid : Invariant size cursor)
    (active : cursor.next < cursor.stop) :
    step words cursor = (.yielded (half32 words cursor.next), advance cursor) := by
  have available : cursor.next / 2 < words.length := by
    obtain ⟨_, endpoint⟩ := valid
    rw [stored]
    unfold count64
    unfold count32 at endpoint
    omega
  have checked : words[cursor.next / 2]? = some words[cursor.next / 2] :=
    List.getElem?_eq_getElem available
  have unfinished : cursor.next ≠ cursor.stop := by omega
  simp [step, unfinished, checked, half32, word]

/-- Every successful admitted step retains the endpoint and decreases the
    remaining count by exactly one. This is the finite progress measure. -/
theorem advance_invariant (size : Nat) (cursor : Cursor)
    (valid : Invariant size cursor) (active : cursor.next < cursor.stop) :
    Invariant size (advance cursor) ∧
      remaining cursor = remaining (advance cursor) + 1 := by
  obtain ⟨bounded, endpoint⟩ := valid
  constructor
  · exact ⟨by simp only [advance]; omega, endpoint⟩
  · simp only [remaining, advance]
    omega

/-- The unconsumed export begins with precisely the numeric half returned by
    an active step. Its tail is the export at the next cursor position. -/
theorem suffix_cons (size : Nat) (words : List (BitVec 64)) (cursor : Cursor)
    (valid : Invariant size cursor) (active : cursor.next < cursor.stop) :
    (export32 size words).drop cursor.next = half32 words cursor.next ::
      (export32 size words).drop (advance cursor).next := by
  have bounded : cursor.next < (export32 size words).length := by
    simp only [export32, List.length_ofFn]
    exact valid.2 ▸ active
  rw [List.drop_eq_getElem_cons bounded]
  simp [export32, advance]

/-- An admitted cursor step is exactly one list-iterator step on the remaining
    numeric export. Empty suffix means exhaustion; a nonempty suffix returns
    its head. This derives output correspondence from the checked operations. -/
theorem step_refines (size : Nat) (words : List (BitVec 64)) (cursor : Cursor)
    (stored : words.length = count64 size) (valid : Invariant size cursor) :
    step words cursor = match (export32 size words).drop cursor.next with
      | [] => (.exhausted, cursor)
      | value :: _ => (.yielded value, advance cursor) := by
  by_cases finished : cursor.next = cursor.stop
  · have empty : (export32 size words).drop cursor.next = [] := by
      apply List.drop_of_length_le
      simp only [export32, List.length_ofFn]
      have endpoint : cursor.stop = count32 size := valid.2
      omega
    rw [empty]
    exact (exhausted_iff words cursor).2 finished
  · have active : cursor.next < cursor.stop := by have bounded : cursor.next ≤ cursor.stop := valid.1; omega
    rw [suffix_cons size words cursor valid active]
    exact step_active size words cursor stored valid active

/-- The exact-size count equals the length of the remaining export, even when
    that suffix contains zero words. It is a count of words, not selected atoms. -/
theorem remaining_exact (size : Nat) (words : List (BitVec 64)) (cursor : Cursor)
    (valid : Invariant size cursor) :
    remaining cursor = ((export32 size words).drop cursor.next).length := by
  simp [remaining, export32, valid.2]

/-- Attempt at most `steps` returns. Reaching exhaustion stops normally;
    malformed storage returns `none`. Zero steps gives the unchanged partial
    cursor, not a claim that the export is complete. -/
def run : Nat → List (BitVec 64) → Cursor → Option (List (BitVec 32) × Cursor)
  | 0, _, cursor => some ([], cursor)
  | steps + 1, words, cursor =>
    match step words cursor with
    | (.exhausted, final) => some ([], final)
    | (.missingWord, _) => none
    | (.yielded value, next) =>
      (run steps words next).map (fun (values, final) => (value :: values, final))

/-- Any number of steps within the remaining count returns exactly that
    prefix of the unconsumed export, with the cursor advanced by that number.
    Induction composes the checked step and the corresponding suffix split;
    no completed-output equivalence is assumed. -/
theorem run_prefix (size : Nat) (words : List (BitVec 64)) (cursor : Cursor)
    (steps : Nat) (stored : words.length = count64 size)
    (valid : Invariant size cursor) (within : steps ≤ remaining cursor) :
    run steps words cursor = some
      (((export32 size words).drop cursor.next).take steps,
        ⟨cursor.next + steps, cursor.stop⟩) := by
  induction steps generalizing cursor with
  | zero => simp [run]
  | succ steps induction =>
    have active : cursor.next < cursor.stop := by
      unfold remaining at within
      omega
    obtain ⟨nextValid, progress⟩ := advance_invariant size cursor valid active
    have nextWithin : steps ≤ remaining (advance cursor) := by omega
    have rest : run steps words (advance cursor) = some
        (((export32 size words).drop (advance cursor).next).take steps,
          ⟨(advance cursor).next + steps, (advance cursor).stop⟩) :=
      induction (advance cursor) nextValid nextWithin
    simp only [run, step_active size words cursor stored valid active, rest, Option.map_some]
    rw [suffix_cons size words cursor valid active]
    simp [advance, Nat.add_assoc]
    omega

/-- Taking exactly the remaining number of steps returns the entire suffix and
    reaches the endpoint. One further call then reports genuine exhaustion.
    This is a constructive finite completion bound, including an empty suffix. -/
theorem run_complete (size : Nat) (words : List (BitVec 64)) (cursor : Cursor)
    (stored : words.length = count64 size) (valid : Invariant size cursor) :
    run (remaining cursor) words cursor =
      some ((export32 size words).drop cursor.next, ⟨cursor.stop, cursor.stop⟩) ∧
      step words ⟨cursor.stop, cursor.stop⟩ = (.exhausted, ⟨cursor.stop, cursor.stop⟩) := by
  have returned : run (remaining cursor) words cursor = some
      (((export32 size words).drop cursor.next).take (remaining cursor),
        ⟨cursor.next + remaining cursor, cursor.stop⟩) :=
    run_prefix size words cursor (remaining cursor) stored valid (Nat.le_refl _)
  have arrived : cursor.next + remaining cursor = cursor.stop := by
    unfold remaining
    have bounded : cursor.next ≤ cursor.stop := valid.1
    omega
  constructor
  · have whole : ((export32 size words).drop cursor.next).take (remaining cursor) =
        (export32 size words).drop cursor.next := by
      rw [remaining_exact size words cursor valid, List.take_length]
    rw [whole, arrived] at returned
    exact returned
  · exact (exhausted_iff words _).2 rfl

/-- Exhaustion is fused: arbitrarily many further attempted returns keep the
    empty output and the same endpoint cursor. -/
theorem run_exhausted (words : List (BitVec 64)) (cursor : Cursor) (steps : Nat)
    (finished : cursor.next = cursor.stop) :
    run steps words cursor = some ([], cursor) := by
  cases steps with
  | zero => rfl
  | succ steps => simp [run, (exhausted_iff words cursor).2 finished]

/-- From initialization, the checked iterator returns the whole numeric export
    and exactly its declared endpoint. Existing export correctness then gives
    bit-for-bit membership and zero padding of those actual returned words.
    The input storage and raw padding are explicit representation premises. -/
theorem initialized_exact (size : Nat) (words : List (BitVec 64))
    (stored : words.length = count64 size) (padding : ZeroPadding size words) :
    ∃ output, run (count32 size) words (start size) =
        some (output, ⟨count32 size, count32 size⟩) ∧
      output = export32 size words ∧ output.length = count32 size ∧
      (∀ atom, bit32 output atom = contains size words atom) ∧
      (∀ atom, size ≤ atom → bit32 output atom = false) := by
  obtain ⟨completed, _⟩ := run_complete size words (start size) stored (start_invariant size)
  obtain ⟨length, membership, _⟩ := export32_exact size words stored padding
  have zeros : ∀ atom, size ≤ atom → bit32 (export32 size words) atom = false := by
    intro atom outside
    rw [membership atom]
    simp [contains, show ¬ atom < size by omega]
  exact ⟨export32 size words, by simpa [start, remaining] using completed,
    rfl, length, membership, zeros⟩

/-- Composing bounded construction with the checked iterator removes the
    storage and padding premises: admitting the input atom coordinates suffices
    to derive the returned word count and exact raw set membership. Duplicate
    atoms and arbitrary insertion order are permitted. -/
theorem packed_exact (size : Nat) (atoms : List Nat)
    (inside : ∀ atom ∈ atoms, atom < size) :
    ∃ output, run (count32 size) (pack size atoms) (start size) =
        some (output, ⟨count32 size, count32 size⟩) ∧
      output.length = count32 size ∧
      (∀ atom, bit32 output atom = decide (atom ∈ atoms)) := by
  obtain ⟨stored, membership, padding⟩ := pack_exact size atoms inside
  obtain ⟨output, completed, _, length, exported, _⟩ :=
    initialized_exact size (pack size atoms) stored padding
  have exactSet : ∀ atom, bit32 output atom = decide (atom ∈ atoms) := by
    intro atom
    rw [exported atom, membership atom]
  exact ⟨output, completed, length, exactSet⟩

end Zetesis.Refinement.PackedWordIterator
