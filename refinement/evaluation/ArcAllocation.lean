import PureExternals

/-!
# A supplied allocation operation

This interface supplies the external operation called by the extracted
`Theory::new` wrapper. The generated function is parameterized by an explicit,
audited section binder; this is an adaptation of the generated binding, not an
option supplied by the pinned Aeneas translator. The generated body is retained.

The class contains only an operation. Value preservation on return belongs in
the hypotheses of the theorem consuming that return. There is no global instance,
allocation-success assumption, or freshness law. The external operation's result
is passed through unchanged, including backend failure or divergence. Rust's
`Arc::new` does not return a typed admission error on allocation failure.

One supplied operation is a pure function. Reusing it on equal inputs does not
model independent fresh allocations. A claim about one runtime invocation must
relate its supplied operation and returned owner to that invocation. Freshness,
when needed, is relative to that invocation's prior heap; composing multiple
invocations requires a separate account of their owners and heaps. This module
establishes no allocator, reference-count, destruction, or runtime correspondence.
-/

open Aeneas Aeneas.Std

/-- A supplied interpretation of the external allocation call. The operation
may return a shared-owner view, fail, or diverge. Its contract is explicit in the
consumer's theorem rather than hidden in a type-class law. -/
class ArcAllocation where
  allocate : {T : Type} → T → Result (alloc.sync.Arc T)

/-- Dispatch the exact extracted `Arc::new` call to the supplied operation.
The implicit provider is the audited addition to the generated external
signature; the value argument and result are unchanged. No outcome is converted. -/
@[rust_fun "alloc::sync::{alloc::sync::Arc<@T>}::new"]
def alloc.sync.Arc.new {T : Type} [allocation : ArcAllocation]
    (value : T) : Result (alloc.sync.Arc T) :=
  allocation.allocate value
