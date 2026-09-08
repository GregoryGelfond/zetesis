import Zetesis.Core

/-!
# Optional finite indices represented by positive successors

A finite zero-based index can be represented by its positive successor without
borrowing an index to mean absence. Optional links retain `none` and encode only
their present identities. The central laws establish exact round trips and that
replacing a link commutes with the representation map. Consequently, a physical
link update need not change the identity or absence of any logical link.

The bound is explicit and arbitrary. A machine-word implementation must also
establish that each successor fits its word and each link indexes its allocated
registry. These laws do not prove Rust's nonzero niche layout, vector allocation,
watch relocation, propagation, work charging, cancellation or solver correctness.
They provide the identity correspondence needed by those separate obligations.
-/

namespace Zetesis.OptionalIndex

/-- A positive representative has one additional possible numeric endpoint. -/
abbrev Positive (bound : Nat) := { value : Fin (bound + 1) // value.val ≠ 0 }

/-- Present indices are shifted by one; no valid zero-based index is discarded. -/
def encode {bound : Nat} (index : Fin bound) : Positive bound :=
  ⟨⟨index.val + 1, by omega⟩, by simp⟩

/-- Positivity makes the predecessor a valid index in the original domain. -/
def decode {bound : Nat} (identity : Positive bound) : Fin bound :=
  ⟨identity.val.val - 1, by have inside := identity.val.isLt; have positive := identity.property; omega⟩

/-- Bounding the complete registry also bounds every stored successor. -/
theorem successor_fits {bound maximum : Nat} (index : Fin bound)
    (representable : bound ≤ maximum) : (encode index).val.val ≤ maximum := by
  have inside : index.val < bound := index.isLt
  simp only [encode]
  omega

/-- Every logical index survives encoding followed by decoding. -/
theorem decode_encode {bound : Nat} (index : Fin bound) :
    decode (encode index) = index := by
  apply Fin.ext
  simp [decode, encode]

/-- Every admitted positive identity survives decoding followed by encoding. -/
theorem encode_decode {bound : Nat} (identity : Positive bound) :
    encode (decode identity) = identity := by
  apply Subtype.ext
  apply Fin.ext
  have positive : identity.val.val ≠ 0 := identity.property
  simp only [encode, decode]
  omega

/-- Distinct logical nodes cannot acquire the same represented identity. -/
theorem encode_injective {bound : Nat} : Function.Injective (@encode bound) := by
  intro left right represented
  have decoded : decode (encode left) = decode (encode right) := congrArg decode represented
  simpa only [decode_encode] using decoded

/-- Optional links preserve both absence and every present identity. -/
theorem optional_round_trip {bound : Nat} (link : Option (Fin bound)) :
    (link.map encode).map decode = link := by
  cases link with
  | none => rfl
  | some index => simp [decode_encode]

/-- Absence cannot be confused with a represented present node. -/
theorem absence_exact {bound : Nat} (link : Option (Fin bound)) :
    link.map encode = none ↔ link = none := by
  cases link <;> simp

/-- Replace a single link in a table whose keys remain unchanged. -/
def replace {Key Value : Type} [DecidableEq Key] (table : Key → Value)
    (key : Key) (value : Value) : Key → Value :=
  fun queried => if queried = key then value else table queried

/-- A link update has the same logical effect before or after representation. -/
theorem replacement_commutes {Key : Type} [DecidableEq Key] {bound : Nat}
    (table : Key → Option (Fin bound)) (key : Key) (link : Option (Fin bound)) :
    (fun queried => (replace table key link queried).map encode) =
      replace (fun queried => (table queried).map encode) key (link.map encode) := by
  funext queried
  by_cases selected : queried = key
  · simp [replace, selected]
  · simp [replace, selected]

end Zetesis.OptionalIndex
