import Harness.Types

open Aeneas Aeneas.Std

/-- Dereferencing a live immutable cell exposes its stored value in this model.
This definition supplies no membership, indexing or bitwise conclusion. -/
@[rust_fun "alloc::sync::{core::ops::deref::Deref<alloc::sync::Arc<@T>, @T>}::deref"]
def alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref
    {T : Type} (_allocator : Type) (cell : alloc.sync.Arc T) : Result T :=
  .ok cell.value
