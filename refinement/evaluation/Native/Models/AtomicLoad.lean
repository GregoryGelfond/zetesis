import Native.Types

/-!
# Native-generation fixed observation reads

These exact external signatures take the native generation's Ordering, while
sharing the existing Atomic observation tokens. They preserve AtomicLoad's
explicit model boundary: Relaxed reads return the supplied observation; other
orderings are outside the model and return backend `undef`. That latter branch
is not a claim that Rust Acquire or SeqCst fails.

Names are scoped under the new generated namespace so no legacy external
declaration or generated Ordering is redefined or imported. Generated calls
resolve these native bindings within that namespace. Reusing a token repeats its
observation; no concurrent-memory, freshness or runtime-effect law is supplied.
-/
namespace ZetesisNativeExtract

/-- Read a supplied Boolean through the actual native Ordering signature. -/
def core.sync.atomic.AtomicBoolAlign1U8.load
    (observation : _root_.core.sync.atomic.Atomic Bool
      (_root_.core.sync.atomic.private.Align1 Aeneas.Std.U8))
    (ordering : core.sync.atomic.Ordering) : Aeneas.Std.Result Bool :=
  match ordering with
  | .Relaxed => .ok observation.nextRead
  | _ => .fail .undef

/-- Read a supplied U64; native membership performs its own active-word test. -/
def core.sync.atomic.AtomicU64Align8U64.load
    (observation : _root_.core.sync.atomic.Atomic Aeneas.Std.U64
      (_root_.core.sync.atomic.private.Align8 Aeneas.Std.U64))
    (ordering : core.sync.atomic.Ordering) : Aeneas.Std.Result Aeneas.Std.U64 :=
  match ordering with
  | .Relaxed => .ok observation.nextRead
  | _ => .fail .undef

end ZetesisNativeExtract
