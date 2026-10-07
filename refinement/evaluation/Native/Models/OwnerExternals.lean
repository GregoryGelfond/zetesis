import Native.Types

/-!
# Native-generation shared-owner clone binding

The native signature uses its generated AllocatorClone record and the existing
shared Arc model. Cloning preserves the same owner token and exposed value.
This is the existing explicit library contract, not a proof of reference
counts, allocation, destruction, custom allocator callbacks or live Rust heaps.

The binding's native namespace prevents collision with the legacy generated
trait type and its historical clone model. No equality between old and native
generated types is assumed.
-/
namespace ZetesisNativeExtract

/-- Preserve the owner and value under the native allocator-trait signature.
Only the generated Theory clone's global-allocator use is covered. -/
def alloc.sync.Arc.Insts.CoreCloneClone.clone
    {T : Type} {A : Type} (_allocator : core.alloc.AllocatorClone A)
    (view : _root_.alloc.sync.Arc T) : Aeneas.Std.Result (_root_.alloc.sync.Arc T) :=
  .ok view

end ZetesisNativeExtract
