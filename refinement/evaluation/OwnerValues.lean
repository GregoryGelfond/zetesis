import PureExternals

/-!
# Values exposed by one immutable shared owner

These generic laws use the existing Arc representation directly. A common heap
assigns one value to each owner; two views consistent with that heap and owner
therefore expose the same value. They assume no generated Theory type, allocator
success, clone operation or reference-count behavior. Historical and native
owner checks import this one relation and keep their distinct generated calls.
-/
namespace RuntimeOwnership

open Aeneas Aeneas.Std Result

/-- The active representation is shared with extraction, not copied into a
second ownership model. -/
abbrev ArcView (α : Type) := alloc.sync.Arc α

/-- A heap describes the immutable value at each live identity. Absence is not
an allocation failure result; this module performs no allocation. -/
abbrev Heap (α : Type) := Nat → Option α

/-- A view reads the value stored at its owner in the common immutable heap.
Two inconsistent payloads cannot both satisfy this relation at the same owner. -/
def Consistent {α : Type} (heap : Heap α) (view : ArcView α) : Prop :=
  heap view.owner = some view.value

/-- Consistent views of the same immutable allocation have the same value.
The common heap is essential: each view reads the same table entry. -/
theorem same_owner_value {α : Type} (heap : Heap α) (left right : ArcView α)
    (leftValid : Consistent heap left) (rightValid : Consistent heap right)
    (same : left.owner = right.owner) : left.value = right.value := by
  have sameEntry : some left.value = some right.value := by
    calc
      some left.value = heap left.owner := leftValid.symm
      _ = heap right.owner := congrArg heap same
      _ = some right.value := rightValid
  exact Option.some.inj sameEntry

end RuntimeOwnership
