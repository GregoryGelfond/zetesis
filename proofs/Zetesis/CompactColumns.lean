import Zetesis.ColumnRelations
import Init.Data.BitVec.Lemmas

/-!
# Compact equality columns

A finite equality identifier can use any word width that represents it exactly.
Packing checks that condition; widening retains the identifier's numerical value.
Aligned columns still use one row domain, so compact storage preserves complete
tuples and their equality selections. Identifier order has no logical meaning.

These authored representation laws apply to widths including 8, 16 and 32 bits.
They do not establish Rust capacity accounting, reservation failure, owner
authentication, GPU transport offsets or instruction-level vectorization.
The concrete storage and device correspondence remain separate obligations.
-/

namespace Zetesis.CompactColumns

/-- Every supplied identifier has an exact representation at the chosen width. -/
def Fits (width : Nat) (ids : List Nat) : Prop :=
  ∀ id ∈ ids, id < 2 ^ width

instance (width : Nat) (ids : List Nat) : Decidable (Fits width ids) := by
  unfold Fits
  infer_instance

/-- Refuse an unrepresentable identifier rather than truncate it. -/
def pack (width : Nat) (ids : List Nat) : Option (List (BitVec width)) :=
  if Fits width ids then some (ids.map (BitVec.ofNat width)) else none

/-- Decode cells in their original row order. -/
def decode {width : Nat} (cells : List (BitVec width)) : List Nat :=
  cells.map BitVec.toNat

/-- A fitting identifier survives word construction exactly. -/
theorem cell_roundtrip (width id : Nat) (fits : id < 2 ^ width) :
    (BitVec.ofNat width id).toNat = id := by
  simp [Nat.mod_eq_of_lt fits]

/-- Encoding all fitting cells preserves the complete ordered column. Apply
the cell round trip to the head and the induction hypothesis to the tail. -/
theorem column_roundtrip (width : Nat) (ids : List Nat) (fits : Fits width ids) :
    decode (ids.map (BitVec.ofNat width)) = ids := by
  induction ids with
  | nil => rfl
  | cons id rest inductionHypothesis =>
    have headFits : id < 2 ^ width := by
      exact fits id (by simp)
    have tailFits : Fits width rest := by
      intro value member
      exact fits value (by simp [member])
    simp only [List.map_cons, decode, BitVec.toNat_ofNat]
    rw [Nat.mod_eq_of_lt headFits]
    exact congrArg (id :: ·) (inductionHypothesis tailFits)

/-- Successful checked packing decodes to the original column. Success supplies
the fit premise; no truncation can be hidden behind a successful result. -/
theorem packed_exact (width : Nat) (ids : List Nat) (cells : List (BitVec width))
    (completed : pack width ids = some cells) : decode cells = ids := by
  by_cases fits : Fits width ids
  · have encoded : ids.map (BitVec.ofNat width) = cells := by
      simpa [pack, fits] using completed
    rw [← encoded]
    exact column_roundtrip width ids fits
  · simp [pack, fits] at completed

/-- Every failed packing contains an identifier outside the chosen width. -/
theorem refusal_iff (width : Nat) (ids : List Nat) :
    pack width ids = none ↔ ¬ Fits width ids := by
  by_cases fits : Fits width ids <;> simp [pack, fits]

/-- Re-encode existing cells at a destination width. Callers establish that the
destination represents their values before publishing the replacement. -/
def resize {source : Nat} (target : Nat) (cells : List (BitVec source)) :
    List (BitVec target) :=
  (decode cells).map (BitVec.ofNat target)

/-- A fitting replacement preserves all values and row positions, including
when the replacement widens an existing column. -/
theorem resize_exact {source : Nat} (target : Nat) (cells : List (BitVec source))
    (fits : Fits target (decode cells)) :
    decode (resize target cells) = decode cells :=
  column_roundtrip target (decode cells) fits

/-- Packing an append preserves the old column as an unchanged decoded prefix. -/
theorem append_prefix (width : Nat) (before added : List Nat)
    (cells : List (BitVec width))
    (completed : pack width (before ++ added) = some cells) :
    (decode cells).take before.length = before := by
  rw [packed_exact width (before ++ added) cells completed]
  simp

open ColumnRelations

universe u
variable {Value : Type u}

/-- Physical columns may differ in width while sharing the same row positions. -/
abbrev Storage (rows arity : Nat) (widths : Fin arity → Nat) :=
  (column : Fin arity) → Fin rows → BitVec (widths column)

/-- Store each dictionary identifier at its corresponding column and row. -/
def encode (dictionary : Dictionary Value) {rows arity : Nat}
    (columns : Columns dictionary rows arity) (widths : Fin arity → Nat) :
    Storage rows arity widths :=
  fun column row => BitVec.ofNat (widths column) (columns column row).val

/-- The declared widths suffice for the actual IDs, not merely for the number
of different IDs occurring in each column. -/
def Representable (dictionary : Dictionary Value) {rows arity : Nat}
    (columns : Columns dictionary rows arity) (widths : Fin arity → Nat) : Prop :=
  ∀ column row, (columns column row).val < 2 ^ widths column

/-- Each encoded cell retains its original dictionary identifier. -/
theorem encoded_cell_exact (dictionary : Dictionary Value) {rows arity : Nat}
    (columns : Columns dictionary rows arity) (widths : Fin arity → Nat)
    (fits : Representable dictionary columns widths)
    (column : Fin arity) (row : Fin rows) :
    (encode dictionary columns widths column row).toNat = (columns column row).val :=
  cell_roundtrip (widths column) (columns column row).val (fits column row)

/-- Compare dictionary equality IDs directly with decoded physical cells.
An absent queried value matches no cell; zero is an ordinary identifier. -/
def accepts (dictionary : Dictionary Value) {rows arity : Nat}
    {widths : Fin arity → Nat} (storage : Storage rows arity widths)
    (equalities : Equalities Value arity) (row : Fin rows) : Bool :=
  equalities.all fun equality =>
    decide ((dictionary.encode equality.2).map Fin.val =
      some (storage equality.1 row).toNat)

/-- Compact equality checking agrees with the existing logical column check.
For each equality, decode the stored ID exactly; equality of finite identifiers
is precisely equality of their numerical coordinates. -/
theorem accepts_exact (dictionary : Dictionary Value) {rows arity : Nat}
    (columns : Columns dictionary rows arity) (widths : Fin arity → Nat)
    (fits : Representable dictionary columns widths)
    (equalities : Equalities Value arity) (row : Fin rows) :
    accepts dictionary (encode dictionary columns widths) equalities row =
      ColumnRelations.accepts dictionary columns equalities row := by
  unfold accepts ColumnRelations.accepts
  apply congrArg (fun predicate => equalities.all predicate)
  funext equality
  rw [encoded_cell_exact dictionary columns widths fits]
  cases found : dictionary.encode equality.2 with
  | none => simp
  | some id => simp [Fin.ext_iff]

/-- Filtering with compact columns retains the exact original row sequence.
This preserves duplicate tuple occurrences at distinct row positions. -/
theorem selection_exact (dictionary : Dictionary Value) {rows arity : Nat}
    (columns : Columns dictionary rows arity) (widths : Fin arity → Nat)
    (fits : Representable dictionary columns widths)
    (equalities : Equalities Value arity) (input : List (Fin rows)) :
    input.filter (accepts dictionary (encode dictionary columns widths) equalities) =
      ColumnRelations.select dictionary columns equalities input := by
  apply List.filter_congr
  intro row _member
  exact accepts_exact dictionary columns widths fits equalities row

end Zetesis.CompactColumns
