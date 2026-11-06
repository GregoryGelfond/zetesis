import Zetesis.Core
import Init.Data.BitVec.Lemmas

/-!
# Packed interpretations and exact word export

This representation model gives width-general physical bit updates and common-
prefix lane combinations, then constructs a finite interpretation in a list of
64-bit words. It starts with zero words and inserts each atom by setting its
least-significant-bit-first position. Export reads successive numeric low and
high 32-bit halves, stopping at the declared universe's final word.

The proofs establish storage, membership and padding from those operations;
they do not assume that construction or export denotes the intended set.
`denotes` connects the representation to the semantic library's predicate sets.

This is authored Lean, not a new extraction of Rust. The corresponding Rust
operations are `Interpretation::new`, `contains` and `words32` in
`zetesis-ferraris/src/theory.rs`. Allocation, machine-sized index arithmetic,
ownership, byte conversion, actual Rust iterator correspondence and device execution
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

/-- Read a physical storage bit at any positive lane width. Missing lanes read
    as zero; no logical universe or zero-padding convention is imposed. -/
def bit {width : Nat} (words : List (BitVec width)) (atom : Nat) : Bool :=
  (word words (atom / width)).getLsbD (atom % width)

/-- Insert a physical coordinate by OR-ing its one-bit mask. Exactness requires
    a positive lane width and an existing destination lane. -/
def insertBit {width : Nat} (words : List (BitVec width)) (atom : Nat) : List (BitVec width) :=
  words.set (atom / width) (word words (atom / width) ||| (1#width <<< (atom % width)))

/-- Clear a physical coordinate by AND-ing the complement of its one-bit mask.
    Like insertion, it neither changes storage shape nor normalizes padding. -/
def clearBit {width : Nat} (words : List (BitVec width)) (atom : Nat) : List (BitVec width) :=
  words.set (atom / width) (word words (atom / width) &&& ~~~(1#width <<< (atom % width)))

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
theorem mask_bit {width : Nat} (value : BitVec width) (offset : Nat) (inside : offset < width) :
    (value &&& (1#width <<< offset) != 0#width) = value.getLsbD offset := by
  rw [← BitVec.twoPow_eq, BitVec.and_twoPow]
  have nonzero : BitVec.twoPow width offset ≠ 0#width := by
    intro zero
    have selected := congrArg (fun bits => bits.getLsbD offset) zero
    simp [inside] at selected
  cases present : value.getLsbD offset <;> simp [nonzero]

/-- At every positive lane width, an admitted mask insertion preserves length
    and changes exactly its physical coordinate. Other coordinates, including
    caller-owned padding, retain their old bits.

    The proof first separates lanes. Within the destination lane, equal
    quotient and remainder identify the coordinate; the one-bit mask therefore
    selects exactly that coordinate. -/
theorem insert_bit_exact {width : Nat} (positive : 0 < width)
    (words : List (BitVec width)) (atom : Nat) (stored : atom / width < words.length) :
    (insertBit words atom).length = words.length ∧
      ∀ tested, bit (insertBit words atom) tested =
        (bit words tested || decide (tested = atom)) := by
  have length : (insertBit words atom).length = words.length := by simp [insertBit]
  have membership : ∀ tested, bit (insertBit words atom) tested =
      (bit words tested || decide (tested = atom)) := by
    intro tested
    by_cases sameWord : atom / width = tested / width
    · have sameAtom : tested % width = atom % width ↔ tested = atom := by
        constructor
        · intro sameOffset
          calc
            tested = tested % width + width * (tested / width) := (Nat.mod_add_div _ _).symm
            _ = atom % width + width * (atom / width) := by rw [sameOffset, ← sameWord]
            _ = atom := Nat.mod_add_div _ _
        · intro same
          exact congrArg (fun value => value % width) same
      simp only [bit, insertBit, word]
      rw [← sameWord, List.getElem?_set_self stored]
      simp only [Option.getD_some, BitVec.getLsbD_or]
      rw [← BitVec.twoPow_eq]
      simp [Nat.mod_lt _ positive, ← sameAtom, eq_comm]
    · have distinct : tested ≠ atom := by
        intro same
        exact sameWord (congrArg (fun value => value / width) same.symm)
      simp [bit, insertBit, word, List.getElem?_set_ne sameWord, distinct]
  exact ⟨length, membership⟩

/-- At every positive lane width, an admitted complement-mask clear preserves
    length and removes exactly its physical coordinate. No zero-padding or
    set-membership premise is required. The lane/offset argument is the same
    as for insertion; the complemented mask changes union to exclusion. -/
theorem clear_bit_exact {width : Nat} (positive : 0 < width)
    (words : List (BitVec width)) (atom : Nat) (stored : atom / width < words.length) :
    (clearBit words atom).length = words.length ∧
      ∀ tested, bit (clearBit words atom) tested =
        (bit words tested && decide (tested ≠ atom)) := by
  have length : (clearBit words atom).length = words.length := by simp [clearBit]
  have membership : ∀ tested, bit (clearBit words atom) tested =
      (bit words tested && decide (tested ≠ atom)) := by
    intro tested
    by_cases sameWord : atom / width = tested / width
    · have sameAtom : tested % width = atom % width ↔ tested = atom := by
        constructor
        · intro sameOffset
          calc
            tested = tested % width + width * (tested / width) := (Nat.mod_add_div _ _).symm
            _ = atom % width + width * (atom / width) := by rw [sameOffset, ← sameWord]
            _ = atom := Nat.mod_add_div _ _
        · intro same
          exact congrArg (fun value => value % width) same
      simp only [bit, clearBit, word]
      rw [← sameWord, List.getElem?_set_self stored]
      simp only [Option.getD_some, BitVec.getLsbD_and, BitVec.getLsbD_not]
      rw [← BitVec.twoPow_eq]
      simp [Nat.mod_lt _ positive, ← sameAtom, eq_comm]
    · have distinct : tested ≠ atom := by
        intro same
        exact sameWord (congrArg (fun value => value / width) same.symm)
      simp [bit, clearBit, word, List.getElem?_set_ne sameWord, distinct]
  exact ⟨length, membership⟩

/-- Combine at most `steps` paired lanes, preserving all unmatched destination
    lanes. `steps` counts successful writes, not work units or attempted
    callbacks. This representation model therefore names both a completed
    common-prefix operation and the stored prefix after an interrupted one. -/
def combinePrefix {α : Type} (combine : α → α → α) : Nat → List α → List α → List α
  | 0, left, _ => left
  | _ + 1, [], _ => []
  | _ + 1, left, [] => left
  | steps + 1, first :: left, second :: right =>
      combine first second :: combinePrefix combine steps left right

/-- Common-prefix combination preserves destination length. Exactly those
    paired lanes below the successful-write count are combined; every other
    destination lane is unchanged. In particular, empty inputs, shorter source
    storage and a refused next write do not silently zero the remaining suffix.

    The proof follows paired heads; the induction hypothesis describes every
    tail position. Applying the existing `BitVec.getLsbD_or`, `getLsbD_and` or
    `getLsbD_xor` law then supplies the lane's bitwise set interpretation. -/
theorem combine_prefix_exact {α : Type} (combine : α → α → α)
    (steps : Nat) (left right : List α) :
    (combinePrefix combine steps left right).length = left.length ∧
      ∀ index, (combinePrefix combine steps left right)[index]? =
        if index < steps then
          match left[index]?, right[index]? with
          | some first, some second => some (combine first second)
          | original, _ => original
        else left[index]? := by
  induction steps generalizing left right with
  | zero => simp [combinePrefix]
  | succ steps induction =>
    cases left with
    | nil => simp [combinePrefix]
    | cons first left =>
      cases right with
      | nil =>
        constructor
        · rfl
        · intro index
          by_cases active : index < steps + 1
          · cases observed : (first :: left)[index]? <;>
              simp [combinePrefix, active, observed]
          · simp [combinePrefix, active]
      | cons second right =>
        obtain ⟨sameLength, positions⟩ := induction left right
        constructor
        · simp [combinePrefix, sameLength]
        · intro index
          cases index with
          | zero => simp [combinePrefix]
          | succ index =>
            simpa only [combinePrefix, List.getElem?_cons_succ, Nat.succ_lt_succ_iff] using
              positions index

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
  simpa only [insertBit, bit, insert, bit64] using
    insert_bit_exact (by decide : 0 < 64) words atom stored

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

/-- In a rectangular batch of exported candidates, the shader's read at
`world * stride + atom / 32` tests exactly the selected world's semantic atom.
The preceding worlds supply the offset. The in-range atom keeps the load in
the selected row even when a following world has opposite bits.

This is a mathematical address in admitted storage. Host buffer identity,
checked device address arithmetic and actual shader execution are separate
refinement obligations. -/
theorem candidate_word_read (size : Nat) (before after : List (List (BitVec 64)))
    (row : List (BitVec 64)) (stored : row.length = count64 size)
    (padding : ZeroPadding size row) {atom : Nat} (inside : atom < size) :
    let buffer := (before ++ row :: after).flatMap (export32 size)
    let index := before.length * count32 size + atom / 32
    (word buffer index &&& (1#32 <<< (atom % 32)) != 0#32) = contains size row atom := by
  have prefixLength (rows : List (List (BitVec 64))) :
      (rows.flatMap (export32 size)).length = rows.length * count32 size := by
    induction rows with
    | nil => simp
    | cons row rows previous => simp [previous, export32, Nat.add_mul, Nat.add_comm]
  dsimp only
  have within : atom / 32 < (export32 size row).length := by
    rw [(export32_exact size row stored padding).1]
    unfold count32
    omega
  have sameWord : word ((before ++ row :: after).flatMap (export32 size))
      (before.length * count32 size + atom / 32) = word (export32 size row) (atom / 32) := by
    rw [← prefixLength before]
    simp only [List.flatMap_append, List.flatMap_cons, word]
    rw [List.getElem?_append_right (by omega)]
    simp only [Nat.add_sub_cancel_left, List.getElem?_append_left within]
  rw [sameWord, mask_bit _ _ (Nat.mod_lt atom (by decide))]
  exact (export32_exact size row stored padding).2.1 atom

/-- A word has no left-only bit exactly when every left bit is present
    on the right. This law includes physical padding; it has no logical carrier
    or resource-accounting premise. -/
theorem and_not_zero_iff_bit_imp {width : Nat} (left right : BitVec width) :
    left &&& ~~~right = 0 ↔
      ∀ offset, offset < width → left.getLsbD offset = true → right.getLsbD offset = true := by
  rw [BitVec.eq_of_getLsbD_eq_iff]
  constructor
  · intro absent offset inside held
    have zero := absent offset inside
    simpa [inside, held] using zero
  · intro contained offset inside
    cases held : left.getLsbD offset with
    | false => simp [held]
    | true =>
      have retained := contained offset inside held
      simp [retained]

/-- Ascending equal-shape containment, stopping at the first left-only word.
    The mismatch cases only make this mathematical function total; Rust checks
    equal storage lengths before its callbacks and never enters those cases. -/
def subsetScan {width : Nat} : List (BitVec width) → List (BitVec width) → Bool
  | [], [] => true
  | left :: rest, right :: others =>
      if left &&& ~~~right = 0 then subsetScan rest others else false
  | _, _ => false

/-- Under equal storage lengths, the completed scan is true exactly when each
    paired lane preserves every left bit. A first failing pair decides false
    without using the tail; if no pair fails, induction covers the whole list.
    Converting physical coordinates to semantic membership additionally needs
    the owner's carrier/shape/padding correspondence. Refused callbacks are not
    modeled by this total completed-value law. -/
theorem subset_scan_exact {width : Nat} (left right : List (BitVec width))
    (shape : left.length = right.length) :
    subsetScan left right = true ↔
      ∀ pair ∈ left.zip right, ∀ offset, offset < width →
        pair.1.getLsbD offset = true → pair.2.getLsbD offset = true := by
  induction left generalizing right with
  | nil =>
    cases right with
    | nil => simp [subsetScan]
    | cons value rest => simp at shape
  | cons value rest tailExact =>
    cases right with
    | nil => simp at shape
    | cons other others =>
      have restShape : rest.length = others.length := Nat.succ.inj shape
      change (if value &&& ~~~other = 0#width then subsetScan rest others else false) = true ↔
        ∀ pair ∈ (value, other) :: rest.zip others, ∀ offset, offset < width →
          pair.1.getLsbD offset = true → pair.2.getLsbD offset = true
      by_cases head : value &&& ~~~other = 0#width
      · have preserved := (and_not_zero_iff_bit_imp value other).mp head
        rw [if_pos head]
        constructor
        · intro completed pair listed offset inside present
          rcases List.mem_cons.mp listed with same | later
          · subst pair
            exact preserved offset inside present
          · exact (tailExact others restShape).mp completed pair later offset inside present
        · intro each
          apply (tailExact others restShape).mpr
          intro pair listed offset inside present
          exact each pair (List.mem_cons_of_mem (value, other) listed) offset inside present
      · rw [if_neg head]
        constructor
        · intro impossible
          cases impossible
        · intro each
          have preserved : ∀ offset, offset < width → value.getLsbD offset = true →
              other.getLsbD offset = true :=
            each (value, other) List.mem_cons_self
          exact False.elim (head ((and_not_zero_iff_bit_imp value other).mpr preserved))

end Zetesis.Refinement.PackedInterpretations
