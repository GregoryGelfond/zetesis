import Aeneas

/-!
# Pure library models for the extracted evaluator

These definitions fill only the pure external signatures used by the current
`Evaluator` extraction. Options preserve the extracted callback's result and do
not invoke it when absent. Arc dereference exposes an already live cell's stored
value; cloning preserves its explicit owner identity, and pointer comparison
tests that identity. Vector clear erases logical elements in the backend's
sequence model.

This file supplies no atomic type or load behavior. The extraction carries an
Instant, Mutex and Condvar inside Deadline but never operates on those fields;
concrete identity tokens retain their distinct types without adding axioms or
clock, lock or wake-up behavior. The relation from those tokens to runtime
objects is outside this model.

Rust library correspondence remains a trust boundary. In particular, these
models establish no reference-count, allocation, capacity, destruction, borrowing
or concurrent-memory property. Owner tokens correspond to live Rust allocations
by contract; consistency with one immutable heap is required to transport data
across an owner comparison. The vector operation is
used at `Bool` in this extraction, where clearing elements runs no user-defined
destructor. Its generic signature is not a model of arbitrary Rust drop effects.
-/

open Aeneas Aeneas.Std

/-- A live Arc view retains an owner token and the value exposed by dereference.
The token is not a machine address. Equal tokens justify equal immutable data
only under a common-heap consistency contract. Interior mutation belongs to the
model of the value; this wrapper supplies no atomic or reference-count behavior. -/
@[rust_type "alloc::sync::Arc"]
structure alloc.sync.Arc (T : Type) where
  value : T
  owner : Nat

/-- An opaque reservation error. Extracted wrappers discard its fields when
mapping a refusal to their allocation error. The token supplies the error type,
not allocator internals; `VectorReservation` supplies the operation separately.
Interpretation construction remains unproved. -/
@[rust_type "alloc::collections::TryReserveError"]
structure alloc.collections.TryReserveError where
  token : Nat

/-- An unobserved Instant is represented by a token, without time or ordering.
The generated evaluator does not inspect the deadline's `at` field. -/
@[rust_type "std::time::Instant"]
structure std.time.Instant where
  token : Nat

/-- An unobserved mutex has a token and a phantom contents type.
The generated evaluator neither locks nor reads the retirement field. -/
@[rust_type "std::sync::poison::mutex::Mutex"]
structure std.sync.poison.mutex.Mutex (_T : Type) where
  token : Nat

/-- An unobserved condition variable has a token, without waiting or notification.
The generated evaluator never operates on the deadline's wake-up field. -/
@[rust_type "std::sync::poison::condvar::Condvar"]
structure std.sync.poison.condvar.Condvar where
  token : Nat

/-- The erased shared-reference view preserves the option and its stored value.
No allocation or mutation is represented by this pure borrowing interface. -/
@[rust_fun "core::option::{core::option::Option<@T>}::as_ref"]
def core.option.Option.as_ref {T : Type} (value : Option T) : Result (Option T) :=
  .ok value

/-- A present option invokes the supplied extracted callback exactly once;
absence returns false without calling it. Callback failure is preserved. -/
@[rust_fun "core::option::{core::option::Option<@T>}::is_some_and"]
def core.option.Option.is_some_and {T Closure : Type}
    (callback : core.ops.function.FnOnce Closure T Bool)
    (value : Option T) (closure : Closure) : Result Bool :=
  match value with
  | none => .ok false
  | some item => callback.call_once closure item

/-- A present option invokes the supplied extracted callback exactly once;
absence returns true without calling it. Callback failure is preserved. -/
@[rust_fun "core::option::{core::option::Option<@T>}::is_none_or"]
def core.option.Option.is_none_or {T Closure : Type}
    (callback : core.ops.function.FnOnce Closure T Bool)
    (value : Option T) (closure : Closure) : Result Bool :=
  match value with
  | none => .ok true
  | some item => callback.call_once closure item

/-- Dereferencing an already live wrapper exposes its stored value.
The unused allocator parameter matches the generated signature; no allocator
operation follows. -/
@[rust_fun "alloc::sync::{core::ops::deref::Deref<alloc::sync::Arc<@T>, @T>}::deref"]
def alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref {T : Type}
    (_allocator : Type) (cell : alloc.sync.Arc T) : Result T :=
  .ok cell.value

/-- Compare allocation identities, not stored values. This operation alone
does not rule out forged views; the heap-consistency relation supplies that
invariant for immutable theory data. -/
@[rust_fun "alloc::sync::{alloc::sync::Arc<@T>}::ptr_eq"]
def alloc.sync.Arc.ptr_eq {T : Type} (_allocator : Type)
    (left right : alloc.sync.Arc T) : Result Bool :=
  .ok (decide (left.owner = right.owner))

/-- Clear the backend vector's logical contents. This model has no capacity
field, so returning its empty sequence says nothing about Rust's retained
allocation. This extraction uses Bool elements, with no user drop effects. -/
@[rust_fun "alloc::vec::{alloc::vec::Vec<@T>}::clear"]
def alloc.vec.Vec.clear {T : Type} (_allocator : Type)
    (_vector : Aeneas.Std.alloc.vec.Vec T) : Result (Aeneas.Std.alloc.vec.Vec T) :=
  .ok (Aeneas.Std.alloc.vec.Vec.new T)
