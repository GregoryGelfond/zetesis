# Packed interpretations and exact word export

[`PackedInterpretations.lean`](../Zetesis/PackedInterpretations.lean) proves a
concrete representation algorithm in the namespace
`Zetesis.Refinement.PackedInterpretations`. Its input is a finite atom universe
and a list of natural-number atom coordinates. Its storage is a list of 64-bit
bit vectors, with the least significant bit representing the first atom in each
word. The module imports the general semantic library only for predicate sets
and their extensional equality.

The algorithm and its proofs are authored Lean. They are not a new extraction
of the Rust solver. The historical Aeneas membership package remains unchanged.

## Construction establishes membership

`pack` allocates the required number of zero words and folds `insert` over the
input atoms. Each insertion addresses a word by division by 64 and a bit by
remainder modulo 64, then updates the word with a shifted OR mask.

`insert_exact` proves that an insertion into an existing word preserves storage
length and changes exactly the selected bit. For bits in the same word, equality
of quotient and remainder identifies the atom; bits in other words retain their
original value. `insertAll_exact` lifts this argument through the actual list
fold. Repeated atoms and input order need no special assumptions.

`pack_exact` combines that fold with zero initialization. Assuming each input
atom is below the declared universe size, it establishes all three parts of the
representation contract:

- The stored list has exactly the number of 64-bit words needed.
- The guarded mask test `contains` returns true exactly for input atoms.
- All bits beyond the declared universe are zero, including the final word's
  padding.

The bounds assumption describes successful construction. The Lean function does
not model a rejecting constructor. `mask_bit` proves the mask-test identity for
any word width, and
`contains_eq_bit` exposes the relation between the guarded mask test and raw bit
reading without assuming valid storage or zero padding.

## Export preserves the representation

`export32` builds the exact number of 32-bit words needed for the same universe.
`half32` reads successive low and high numeric halves by shifting and narrowing
the bit vector. `export32_exact` proves that every such read addresses a stored
64-bit word and that every exported bit agrees with guarded membership. The
premises are exact source storage and zero padding, both established by
`pack_exact`. Outside coordinates are either zero padding or beyond the exported
list. The empty universe exports no words.

`denotes` presents a packed interpretation as a predicate on `Fin size`.
`pack_denotes` identifies that predicate with input-list membership, while
`export32_denotes` shows that changing word width preserves it. These equalities
allow substitution into the library's satisfaction and reduct definitions. They
do not independently establish stability, search completeness or solver output
correctness.

## Remaining implementation correspondence

The corresponding Rust operations are `Interpretation::new`, `contains` and
`words32` in `crates/zetesis-ferraris/src/theory.rs`. Their correspondence still
requires separate proofs or validation for:

- Rejection of invalid inputs, allocation failure, and machine-sized arithmetic.
  The model uses unbounded natural numbers; its word-count formulas are not a
  proof about overflow or Rust's `div_ceil` implementation.
- Rust vector reads and writes, ownership and theory identity, and preservation
  of storage and padding by every operation that can produce an interpretation.
- The actual export's little-endian byte conversion. The Lean export uses
  numeric shifts and narrowing; it does not model `to_le_bytes` followed by
  `u32::from_le_bytes`.
- Iterator cursor advancement, exhaustion, exact size hints and fused behavior;
  the Lean export constructs a list directly.
- Device upload, shader layout and device execution.

The historical Aeneas package proves extracted membership under its storage
invariant using its retained toolchain. It does not prove this construction or
export, and the two packages have not been joined into a fresh checked Rust
extraction. This module removes assumed correctness from the authored packed
construction and export model; it leaves that concrete implementation bridge
explicit.
