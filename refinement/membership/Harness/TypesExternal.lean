import Aeneas

/-!
Read-only Arc model for the membership extraction.

The query only dereferences an already live immutable Arc. This model retains
its value and does not represent reference counting, allocation or destruction.
Agreement with Rust's Arc implementation is outside the checked theorem.
-/

@[rust_type "alloc::sync::Arc"]
structure alloc.sync.Arc (T : Type) where
  value : T
