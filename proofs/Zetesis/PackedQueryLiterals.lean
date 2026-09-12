import Zetesis.QueryCompaction

/-!
# Compact references to classical query variables

An admitted signed query literal is represented by twice its variable index
plus its polarity bit. Decoding recovers the same variable and polarity, so
packing preserves literal truth and cannot identify two distinct literals.

The natural-number bound explains the Rust admission premise: twice the CNF
variable count must be representable before any literal is packed. This module
does not prove Rust word arithmetic, slice offsets, allocation or CNF search.
It concerns classical query references, not a rewrite of Ferraris negation.
-/

namespace Zetesis.PackedQueryLiterals

open QueryCompaction

/-- One polarity bit follows the variable index. -/
def encode (literal : Literal Nat) : Nat :=
  2 * literal.atom + if literal.positive then 1 else 0

/-- Recover the variable and its positive/negative occurrence. -/
def decode (word : Nat) : Literal Nat :=
  ⟨word / 2, word % 2 == 1⟩

/-- Packing followed by decoding preserves both parts of the signed reference.
The two polarity cases reduce to quotient and remainder by two. -/
theorem decode_encode (literal : Literal Nat) : decode (encode literal) = literal := by
  cases literal with
  | mk atom positive =>
    cases positive <;> simp [encode, decode] <;> omega

/-- Distinct signed literals cannot be merged by their packed representation.
Decode equal words and use the round-trip law. -/
theorem encode_injective : Function.Injective encode := by
  intro left right equal
  have decoded : decode (encode left) = decode (encode right) := congrArg decode equal
  simpa only [decode_encode] using decoded

/-- Every admitted literal fits below twice its declared variable count. This
is an arithmetic bound, conditional on the supplied index being in range. -/
theorem encode_lt (literal : Literal Nat) (variables : Nat)
    (admitted : literal.atom < variables) : encode literal < 2 * variables := by
  cases literal with
  | mk atom positive =>
    cases positive <;> simp [encode] <;> simp at admitted <;> omega

/-- Inspecting the packed reference gives exactly the original literal's truth
under any classical valuation; the valuation itself is unchanged. -/
theorem packed_truth (valuation : Nat → Bool) (literal : Literal Nat) :
    (decode (encode literal)).eval valuation = literal.eval valuation := by
  rw [decode_encode]

end Zetesis.PackedQueryLiterals
