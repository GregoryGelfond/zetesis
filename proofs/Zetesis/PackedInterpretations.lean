import Zetesis.Core
import Init.Data.BitVec.Lemmas

/-!
# Packed interpretations and exact word export

This representation model constructs a finite interpretation in a list of
64-bit words. It starts with zero words and inserts each atom by setting its
least-significant-bit-first position. Export reads successive numeric low and
high 32-bit halves, stopping at the declared universe's final word.

The proofs establish storage, membership and padding from those operations;
they do not assume that construction or export denotes the intended set.
`denotes` connects the representation to the semantic library's predicate sets.

This is authored Lean, not a new extraction of Rust. The corresponding Rust
operations are `Interpretation::new`, `contains` and `words32` in
`zetesis-ferraris/src/theory.rs`. Allocation, machine-sized index arithmetic,
ownership, byte conversion, the iterator state machine and device execution
remain separate implementation obligations. The historical Aeneas extraction
and its toolchain are unchanged.
-/

namespace Zetesis.Refinement.PackedInterpretations

/-- Number of 64-bit words covering the finite atom universe. -/
def count64 (size : Nat) : Nat := (size + 63) / 64

/-- Number of 32-bit words covering that same universe. -/
def count32 (size : Nat) : Nat := (size + 31) / 32

/-- A missing word reads as zero. Successful construction proves that every
    declared atom has a stored word, independently of this total default. -/
def word {width : Nat} (words : List (BitVec width)) (index : Nat) : BitVec width :=
  words[index]?.getD 0

/-- Read a raw storage bit; unlike `contains`, this also observes padding. -/
def bit64 (words : List (BitVec 64)) (atom : Nat) : Bool :=
  (word words (atom / 64)).getLsbD (atom % 64)

/-- Membership first checks the universe bound, then reads the packed bit. -/
def contains (size : Nat) (words : List (BitVec 64)) (atom : Nat) : Bool :=
  if atom < size then
    (word words (atom / 64) &&& (1#64 <<< (atom % 64))) != 0#64
  else false

/-- Insert one atom by OR-ing its mask into the containing word. `List.set`
    changes no length; callers must establish that the target word exists. -/
def insert (words : List (BitVec 64)) (atom : Nat) : List (BitVec 64) :=
  words.set (atom / 64) (word words (atom / 64) ||| (1#64 <<< (atom % 64)))

/-- Process atoms in input order, just as successive writes to the word array. -/
def insertAll (words : List (BitVec 64)) (atoms : List Nat) : List (BitVec 64) :=
  atoms.foldl insert words

/-- Zero initialization followed by mask insertions. The correctness theorem
    requires every input atom to lie below the declared size; Rust checks
    that premise and refuses an outside atom before its write. -/
def pack (size : Nat) (atoms : List Nat) : List (BitVec 64) :=
  insertAll (List.replicate (count64 size) 0) atoms

/-- Raw padding is zero, not merely hidden by the membership bound check. -/
def ZeroPadding (size : Nat) (words : List (BitVec 64)) : Prop :=
  ∀ atom, size ≤ atom → bit64 words atom = false

/-- The one-bit mask tests the stored bit. This is the bit-vector part of the
    historical extracted membership proof, independent of its library models. -/
theorem mask_bit (value : BitVec 64) (offset : Nat) (inside : offset < 64) :
    (value &&& (1#64 <<< offset) != 0#64) = value.getLsbD offset := by
  rw [← BitVec.twoPow_eq, BitVec.and_twoPow]
  have nonzero : BitVec.twoPow 64 offset ≠ 0#64 := by
    intro zero
    have selected := congrArg (fun bits => bits.getLsbD offset) zero
    simp [inside] at selected
  cases present : value.getLsbD offset <;> simp [nonzero]

/-- A guarded mask test equals the raw bit read conjoined with the universe
    bound. This identity holds even for short storage or nonzero padding. -/
theorem contains_eq_bit (size : Nat) (words : List (BitVec 64)) (atom : Nat) :
    contains size words atom = (decide (atom < size) && bit64 words atom) := by
  have offset : atom % 64 < 64 := Nat.mod_lt atom (by decide)
  by_cases inside : atom < size
  · simpa [contains, inside, bit64] using
      mask_bit (word words (atom / 64)) (atom % 64) offset
  · simp [contains, inside]

/-- Inserting an admitted coordinate preserves storage length and sets exactly
    that atom's bit, preserving all other bits, including padding. The proof
    separates different words from different offsets in the same word. -/
theorem insert_exact (words : List (BitVec 64)) (atom : Nat)
    (stored : atom / 64 < words.length) :
    (insert words atom).length = words.length ∧
      ∀ tested, bit64 (insert words atom) tested =
        (bit64 words tested || decide (tested = atom)) := by
  have length : (insert words atom).length = words.length := by
    simp [insert]
  have membership : ∀ tested, bit64 (insert words atom) tested =
      (bit64 words tested || decide (tested = atom)) := by
    intro tested
    by_cases sameWord : atom / 64 = tested / 64
    · have sameAtom : tested % 64 = atom % 64 ↔ tested = atom := by
        omega
      simp only [bit64, insert, word]
      rw [← sameWord, List.getElem?_set_self stored]
      simp only [Option.getD_some, BitVec.getLsbD_or]
      rw [← BitVec.twoPow_eq]
      simp [Nat.mod_lt _ (by decide : 0 < 64),
        ← sameAtom, eq_comm]
    · have distinct : tested ≠ atom := by
        intro same
        exact sameWord (congrArg (fun value => value / 64) same.symm)
      simp [bit64, insert, word, List.getElem?_set_ne sameWord, distinct]
  exact ⟨length, membership⟩

/-- A completed sequence of admitted writes keeps the initial storage and
    denotes its old bits union the inserted coordinates. No input ordering or
    uniqueness premise is needed. -/
theorem insertAll_exact (words : List (BitVec 64)) (atoms : List Nat)
    (stored : ∀ atom ∈ atoms, atom / 64 < words.length) :
    (insertAll words atoms).length = words.length ∧
      ∀ tested, bit64 (insertAll words atoms) tested =
        (bit64 words tested || decide (tested ∈ atoms)) := by
  induction atoms generalizing words with
  | nil => simp [insertAll]
  | cons first rest induction =>
    have firstStored : first / 64 < words.length := stored first (by simp)
    obtain ⟨sameLength, firstExact⟩ := insert_exact words first firstStored
    have restStored : ∀ atom ∈ rest, atom / 64 < (insert words first).length := by
      intro atom member
      rw [sameLength]
      exact stored atom (by simp [member])
    obtain ⟨restLength, restExact⟩ := induction (insert words first) restStored
    have length : (insertAll words (first :: rest)).length = words.length := by
      exact restLength.trans sameLength
    have membership : ∀ tested, bit64 (insertAll words (first :: rest)) tested =
        (bit64 words tested || decide (tested ∈ first :: rest)) := by
      intro tested
      change bit64 (insertAll (insert words first) rest) tested = _
      rw [restExact, firstExact]
      simp [Bool.or_assoc]
    exact ⟨length, membership⟩

/-- Zero initialization and admitted mask writes establish the representation:
    exact storage length, exact input-set membership and zero padding. In
    particular duplicates and insertion order do not alter the denoted set. -/
theorem pack_exact (size : Nat) (atoms : List Nat)
    (inside : ∀ atom ∈ atoms, atom < size) :
    (pack size atoms).length = count64 size ∧
      (∀ atom, contains size (pack size atoms) atom = decide (atom ∈ atoms)) ∧
      ZeroPadding size (pack size atoms) := by
  have stored : ∀ atom ∈ atoms,
      atom / 64 < (List.replicate (count64 size) (0#64)).length := by
    intro atom member
    have bounded : atom < size := inside atom member
    simp only [List.length_replicate]
    unfold count64
    omega
  obtain ⟨sameLength, rawBits⟩ :=
    insertAll_exact (List.replicate (count64 size) 0) atoms stored
  have length : (pack size atoms).length = count64 size := by
    simpa [pack] using sameLength
  have zeroBits : ∀ count atom, bit64 (List.replicate count 0) atom = false := by
    intro count atom
    simp only [bit64, word, List.getElem?_replicate]
    split <;> simp
  have rawExact : ∀ atom, bit64 (pack size atoms) atom = decide (atom ∈ atoms) := by
    intro atom
    calc
      bit64 (pack size atoms) atom =
          (bit64 (List.replicate (count64 size) 0) atom || decide (atom ∈ atoms)) :=
        rawBits atom
      _ = decide (atom ∈ atoms) := by rw [zeroBits]; rfl
  have membership : ∀ atom,
      contains size (pack size atoms) atom = decide (atom ∈ atoms) := by
    intro atom
    rw [contains_eq_bit, rawExact]
    by_cases member : atom ∈ atoms
    · simp [member, inside atom member]
    · simp [member]
  have padding : ZeroPadding size (pack size atoms) := by
    intro atom outside
    have absent : atom ∉ atoms := by
      intro member
      have bounded := inside atom member
      omega
    simp [rawExact, absent]
  exact ⟨length, membership, padding⟩

/-- Read the numeric low or high half. This operation makes no host-endianness
    assumption; matching Rust's byte round trip is a separate obligation. -/
def half32 (words : List (BitVec 64)) (index : Nat) : BitVec 32 :=
  ((word words (index / 2)) >>> ((index % 2) * 32)).setWidth 32

/-- Export precisely the halves covering the size, including zero words. -/
def export32 (size : Nat) (words : List (BitVec 64)) : List (BitVec 32) :=
  List.ofFn (fun index : Fin (count32 size) => half32 words index.val)

/-- The exported representation uses the same low-bit-first convention. -/
def bit32 (words : List (BitVec 32)) (atom : Nat) : Bool :=
  (word words (atom / 32)).getLsbD (atom % 32)

/-- Export has exactly the required number of 32-bit words and preserves
    membership at every natural-number coordinate, with zero bits outside the
    declared size. Every half it reads belongs to an existing 64-bit word.

    The argument first locates each numeric half, then identifies its bit with
    the original word's bit. Inside coordinates are exported; outside ones are
    either zero padding or beyond the exported list. Storage and padding are
    explicit premises here and are established by `pack_exact` for construction.
    An empty size therefore exports an empty list, not a padding word. -/
theorem export32_exact (size : Nat) (words : List (BitVec 64))
    (stored : words.length = count64 size) (padding : ZeroPadding size words) :
    (export32 size words).length = count32 size ∧
      (∀ atom, bit32 (export32 size words) atom = contains size words atom) ∧
      (∀ index, index < count32 size → index / 2 < words.length) := by
  have halfBit : ∀ index offset, offset < 32 →
      (half32 words index).getLsbD offset = bit64 words (index * 32 + offset) := by
    intro index offset inside
    have containingWord : (index * 32 + offset) / 64 = index / 2 := by omega
    have containingBit : (index * 32 + offset) % 64 = (index % 2) * 32 + offset := by
      omega
    simp [half32, bit64, inside, containingWord, containingBit]
  have exportedBit : ∀ atom, bit32 (export32 size words) atom =
      if atom / 32 < count32 size then bit64 words atom else false := by
    intro atom
    simp only [bit32, word, export32, List.getElem?_ofFn]
    split
    · simp only [Option.getD_some]
      have offset : atom % 32 < 32 := Nat.mod_lt atom (by decide)
      have position : atom / 32 * 32 + atom % 32 = atom := by omega
      simpa only [position] using halfBit (atom / 32) (atom % 32) offset
    · simp
  have length : (export32 size words).length = count32 size := by
    simp [export32]
  have membership : ∀ atom,
      bit32 (export32 size words) atom = contains size words atom := by
    intro atom
    rw [exportedBit]
    by_cases inside : atom < size
    · have exported : atom / 32 < count32 size := by
        unfold count32
        omega
      simp [contains_eq_bit, inside, exported]
    · have absent : bit64 words atom = false := padding atom (by omega)
      simp [contains_eq_bit, inside, absent]
  have readsStored : ∀ index, index < count32 size → index / 2 < words.length := by
    intro index inside
    rw [stored]
    unfold count32 at inside
    unfold count64
    omega
  exact ⟨length, membership, readsStored⟩

/-- The semantic interpretation is independent of the word representation. -/
def denotes (size : Nat) (words : List (BitVec 64)) : Atoms (Fin size) :=
  fun atom => contains size words atom.val = true

/-- Successful bounded construction denotes exactly the supplied coordinate
    set. This equality, rather than an assumption of membership correctness,
    permits substitution into the general satisfaction and reduct definitions. -/
theorem pack_denotes (size : Nat) (atoms : List Nat)
    (inside : ∀ atom ∈ atoms, atom < size) :
    denotes size (pack size atoms) = (fun atom : Fin size => atom.val ∈ atoms) := by
  have membership := (pack_exact size atoms inside).2.1
  apply atoms_ext
  intro atom
  simp only [denotes, membership atom.val, decide_eq_true_eq]

/-- Changing word width does not change the semantic interpretation. This
    supplies representation equality to the semantic library, not a claim of
    stability, complete enumeration, or correct device execution. -/
theorem export32_denotes (size : Nat) (words : List (BitVec 64))
    (stored : words.length = count64 size) (padding : ZeroPadding size words) :
    (fun atom : Fin size => bit32 (export32 size words) atom.val = true) =
      denotes size words := by
  have membership := (export32_exact size words stored padding).2.1
  apply atoms_ext
  intro atom
  rw [membership atom.val]
  rfl

end Zetesis.Refinement.PackedInterpretations
