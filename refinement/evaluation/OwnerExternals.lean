import Evaluator.Types

/-!
# Shared-owner clone interface

The active Arc view records a live allocation identity and its exposed value.
Cloning retains both. This external definition has the exact generated signature;
the selected Theory clone uses the global allocator. It models no reference
counts, allocation, allocator callbacks, destruction or concurrent memory.
Correspondence with Rust's live allocation identity remains a library contract.
-/

open Aeneas Aeneas.Std Result

/-- Retain the identity and value of the existing shared allocation. The allocator
trait parameter matches extraction; only the global-allocator use is covered.
No claim about arbitrary custom allocator effects follows from this definition. -/
@[rust_fun "alloc::sync::{core::clone::Clone<alloc::sync::Arc<@T>>}::clone"]
def alloc.sync.Arc.Insts.CoreCloneClone.clone
    {T : Type} {A : Type} (_allocator : ZetesisExtract.core.alloc.AllocatorClone A)
    (view : alloc.sync.Arc T) : Result (alloc.sync.Arc T) :=
  ok view
