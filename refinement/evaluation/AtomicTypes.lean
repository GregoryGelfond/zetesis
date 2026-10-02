import Aeneas

/-!
# Observation tokens for one extracted evaluator invocation

These are external-definition inputs for a conditional, single-invocation proof
of the unchanged generated evaluator body. They are not shared-memory objects.
An `Atomic` token records one supplied read observation; it has no address,
mutable contents, modification order, or promise about a later read.

The production body polls at most once, loading the cancellation flag at most once and
the deadline flag at most once. Tokens are supplied afresh for those read sites
on each independently considered invocation. Reusing a token in the generated
whole loop would repeat that observation and is outside this model's scope.

`Align1` is an unobserved type marker, not a claim about physical alignment.
The load operation is defined separately because its Ordering type is generated
in `Evaluator.Types`. No external-type axiom is introduced here.
-/

/-- A phantom marker needed by the extracted generic atomic type's signature.
    No layout, allocation or alignment property is represented. -/
@[rust_type "core::sync::atomic::private::Align1"]
structure core.sync.atomic.private.Align1 (_T : Type) where

/-- One explicitly supplied next-read observation. This type's Rust-shaped name
    permits the generated source to remain unchanged; its meaning is an input
    observation token, not an immutable model of a concurrently mutable cell.
    `_Storage` carries only the external signature's unobserved marker. -/
@[rust_type "core::sync::atomic::Atomic"]
structure core.sync.atomic.Atomic (T : Type) (_Storage : Type) where
  nextRead : T
