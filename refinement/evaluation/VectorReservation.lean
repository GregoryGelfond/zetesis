import PureExternals

/-!
# A supplied fallible reservation operation

This interface supplies the external `Vec::try_reserve_exact` operation used by
actual extracted reservation calls. An audited binding parameter must connect
the provider to its generated callers. This module does not change a generated
body, install a default instance, or replace the existing result monad.

The operation returns both the source success/refusal and the vector after the
call, inside the backend result. All of those outcomes are retained. There is
no allocation-success or sequence-preservation law in the class. Consumers state
any required library contract explicitly, such as preservation of logical
contents after a successful reservation. That law would make a successful
reservation on an empty input logically empty; it does not follow from this
interface alone.

The backend vector contains a logical sequence and a length bound, not capacity
or allocation identity. Sequence preservation does not establish spare capacity,
absence of later allocation, or a memory guarantee. Such runtime conclusions
need their stated library contracts. A freeze proof can instead use the actual
evaluator's initial clear when it needs an empty logical output, without adding
an unnecessary premise about the returned reservation contents.

A fixed provider is a pure function. It can describe the outcome of one supplied
invocation, including a typed refusal, backend failure, or divergence. Repeated
calls on equal represented inputs are not a model of changing allocator state,
capacity, or runtime histories. The separate `RuntimeEffects` event specification
is not installed here. No allocator, destructor, or runtime correspondence is
proved by defining this interface.
-/

open Aeneas Aeneas.Std

/-- A supplied interpretation of reservation. The allocator type is retained
from the generated signature; no custom allocator behavior is assumed. Any
successful-sequence or capacity contract belongs to the consuming theorem. -/
class VectorReservation where
  reserve : {T : Type} → (allocator : Type) →
    Aeneas.Std.alloc.vec.Vec T → Usize →
    Result ((core.result.Result Unit alloc.collections.TryReserveError) ×
      Aeneas.Std.alloc.vec.Vec T)

/-- Dispatch the exact extracted reservation arguments to the supplied operation.
The implicit provider is the explicit binding adaptation. The returned source
verdict and vector, backend failure, and divergence are passed through unchanged. -/
@[rust_fun "alloc::vec::{alloc::vec::Vec<@T>}::try_reserve_exact"]
def alloc.vec.Vec.try_reserve_exact {T : Type} [reservation : VectorReservation]
    (allocator : Type) (vector : Aeneas.Std.alloc.vec.Vec T) (additional : Usize) :
    Result ((core.result.Result Unit alloc.collections.TryReserveError) ×
      Aeneas.Std.alloc.vec.Vec T) :=
  reservation.reserve allocator vector additional
