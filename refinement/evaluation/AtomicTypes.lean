import Aeneas

/-!
# Observation tokens for extracted evaluation

An `Atomic` token supplies a fixed read value. It has no address, mutable contents
or modification order and does not represent a Rust shared-memory object.

The production body polls at most once, loading cancellation at most once and
deadline at most once. Independent body invocations may receive fresh tokens.
`FixedLoop` instead follows the generated loop's returned-control threading;
repeated reads of a token return the same value. Its mathematical loop theorem
does not establish correspondence with changing runtime observations.

`Align1` is an unobserved type marker, not a claim about physical alignment.
The load operation is defined separately because its Ordering type is generated
in `Evaluator.Types`. No external-type axiom is introduced here.
-/

/-- A phantom marker needed by the extracted generic atomic type's signature.
    No layout, allocation or alignment property is represented. -/
@[rust_type "core::sync::atomic::private::Align1"]
structure core.sync.atomic.private.Align1 (_T : Type) where

/-- An explicitly supplied observation, repeated if the token is reused.
    The Rust-shaped name preserves the generated source; this is not a model of
    a concurrently mutable cell. `_Storage` is an unobserved signature marker. -/
@[rust_type "core::sync::atomic::Atomic"]
structure core.sync.atomic.Atomic (T : Type) (_Storage : Type) where
  nextRead : T
