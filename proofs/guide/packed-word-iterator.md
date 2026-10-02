# Checked iteration over packed words

[`PackedWordIterator.lean`](../Zetesis/PackedWordIterator.lean) proves an
executable cursor algorithm for exporting numeric 32-bit halves of 64-bit packed
words. It belongs to `Zetesis.Refinement.PackedWordIterator` and imports only the
existing [packed interpretation capability](packed-interpretations.md). The
underlying word list is immutable. The cursor records the next half and its fixed
exclusive endpoint.

This closes a representation-algorithm obligation beyond proving a whole-list
conversion: the iterator's individual reads, updates, stopping condition and
repeated output are related to `PackedInterpretations.export32` by proofs.
It is authored Lean, not an extraction of Rust.

## One checked step

`start size` sets the next index to zero and the endpoint to `count32 size`.
`Invariant` states that the next index has not passed that endpoint and that the
endpoint still matches the declared size. `remaining` is their difference.

`step` first checks endpoint equality, as the Rust iterator does. At the endpoint
it reports `exhausted` and leaves the cursor unchanged. Otherwise it checks the
source list at `next / 2`, numerically shifts by `(next % 2) * 32`, and narrows to
32 bits. A successful return advances the cursor once. A missing word has its own
`missingWord` outcome and does not advance; it is not exhaustion.

`step_active` proves that exact 64-bit storage and the cursor invariant rule out
that missing-word branch before the endpoint. It derives the returned half from
the checked lookup and numeric operations. No padding assumption is needed for
this access result. `advance_invariant` preserves the endpoint and proves that
one return decreases `remaining` by one. `exhausted_iff` identifies the stopping
condition exactly.

`suffix_cons` identifies the next value and tail of the whole-list export.
`step_refines` composes it with the checked access law: one iterator step is one
list step on `export32 size words |>.drop cursor.next`. No premise equates the
iterator's output with that export.

## Repeated steps and completion

`run steps` attempts at most that many returns. It stops normally at exhaustion
and reports `none` for malformed short storage. Zero attempts return the unchanged
partial cursor; that result alone does not certify completion.

For any number of steps no larger than `remaining`, `run_prefix` proves the
exact returned prefix of the unconsumed export and the resulting cursor index.
The induction uses the preceding step and suffix laws. `remaining_exact` proves
that the count is the length of the export suffix, including zero words.

`run_complete` then takes exactly that count, returns the entire remaining suffix,
and reaches the endpoint. One further call reports exhaustion. `run_exhausted`
proves that arbitrarily many further attempts return no values and leave the
cursor unchanged. Empty universes and already exhausted cursors are included.

`initialized_exact` combines these laws with `export32_exact`: from valid storage
and zero raw padding, the actual returned list has the declared length, preserves
membership at every coordinate, and has zero unused tail bits. `packed_exact`
also composes `pack_exact`; bounded input coordinates alone then suffice to prove
word count and exact returned set membership, allowing duplicates and any input
order. The existing packed-interpretation construction and export proofs are
reused rather than restated as assumptions about the iterator.

## Implementation boundary

The corresponding Rust operations are `Interpretation::words32`,
`InterpretationWords::next`, `len` and `size_hint` in
`crates/zetesis-ferraris/src/theory.rs`. The Lean invariant explains why equality
is a sufficient exhaustion test and why the Rust indexed read is in bounds.
`remaining_exact` supplies the logical count used by both size APIs. The fused
behavior is proved for this authored algorithm.

A concrete Rust correspondence still must establish the representation and
cursor invariant at the API boundary, exact interpretation/theory ownership,
borrow preservation, machine-sized index arithmetic, and that the explicit
little-endian byte round trip implements the numeric narrowing. The Lean model
uses a checked lookup where Rust relies on an admitted invariant. Its immutable
list is not a proof of Rust lifetimes, allocation behavior or constant-time array
access. The result concerns representation only; it does not certify device
execution, answer-set membership or complete enumeration.
