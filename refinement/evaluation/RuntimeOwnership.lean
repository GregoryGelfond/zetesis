import OwnerExternals
import OwnerValues

/-!
# Immutable shared-owner contracts

The active Arc representation records an allocation identity and its exposed
value. A common immutable heap assigns at most one value to each identity.
Consistency with that heap permits equal owners to imply equal data without
identifying independent allocations merely because their data are equal.

These laws use the actual external clone and pointer-comparison definitions.
The identities' correspondence with live Rust allocations remains a library
contract. No fresh allocation, reference counting, destruction, interior mutation
or concurrent history is modeled. The clone interface is scoped to its selected
global-allocator use, as documented in OwnerExternals.
-/

namespace RuntimeOwnership

open Aeneas Aeneas.Std Result

/-- The external clone retains the complete input view, introducing no owner. -/
theorem clone_exact {α A : Type}
    (allocator : ZetesisExtract.core.alloc.AllocatorClone A) (view : ArcView α) :
    alloc.sync.Arc.Insts.CoreCloneClone.clone allocator view = ok view := by
  rfl

/-- A returned clone preserves both fields and consistency with the same heap.
The external result supplies the copy rather than assuming its fields agree. -/
theorem clone_consistent {α A : Type}
    (allocator : ZetesisExtract.core.alloc.AllocatorClone A)
    (heap : Heap α) (view copied : ArcView α) (valid : Consistent heap view)
    (returned : alloc.sync.Arc.Insts.CoreCloneClone.clone allocator view = ok copied) :
    copied.owner = view.owner ∧ copied.value = view.value ∧ Consistent heap copied := by
  have same : view = copied := by
    simpa only [alloc.sync.Arc.Insts.CoreCloneClone.clone, Result.ok.injEq] using returned
  subst copied
  exact ⟨rfl, rfl, valid⟩

/-- A positive external pointer comparison is exactly equality of identities.
No data comparison or heap-consistency premise is needed for that test. -/
theorem ptr_eq_exact {α : Type} (allocator : Type) (left right : ArcView α) :
    alloc.sync.Arc.ptr_eq allocator left right = ok true ↔ left.owner = right.owner := by
  simp [alloc.sync.Arc.ptr_eq]

/-- A successful external owner comparison permits transporting immutable data
only for views consistent with the same heap. -/
theorem accepted_owner_value {α : Type} (allocator : Type)
    (heap : Heap α) (left right : ArcView α)
    (leftValid : Consistent heap left) (rightValid : Consistent heap right)
    (accepted : alloc.sync.Arc.ptr_eq allocator left right = ok true) :
    left.value = right.value := by
  have same : left.owner = right.owner := by
    exact (ptr_eq_exact allocator left right).mp accepted
  exact same_owner_value heap left right leftValid rightValid same

/-- Identical data in distinct allocations still fail the external owner test.
The constant heap mapping identities to this value consistently admits both. -/
theorem equal_values_distinct_owners {α : Type} (allocator : Type)
    (value : α) (left right : Nat) (distinct : left ≠ right) :
    alloc.sync.Arc.ptr_eq allocator
      ({ value, owner := left } : ArcView α) { value, owner := right } = ok false := by
  simp [alloc.sync.Arc.ptr_eq, distinct]

end RuntimeOwnership
