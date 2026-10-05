import Evaluator.Types

/-!
# A relaxed read from a supplied observation token

`Evaluator.TypesExternal` must import `AtomicTypes` before the generated
`Evaluator.Types` is checked. This module then supplies the exact missing load
name used by the generated functions.

Only the production poll's Relaxed ordering has a modeled observation. Other
orderings report `undef` as an explicit boundary of this proof model. In
particular, that branch does not assert that a Rust Acquire or SeqCst load fails.
The operation consumes no logical state: reusing its token repeats the value.
`FixedLoop` proves the generated loop under that fixed-token interpretation.
Correspondence with varying concurrent Rust reads remains a separate obligation.
-/

/-- Return the supplied Boolean for a Relaxed read, repeating it on reuse.
    Unsupported orderings remain explicit model failure. -/
@[rust_fun
  "core::sync::atomic::{core::sync::atomic::Atomic<bool, core::sync::atomic::private::Align1<u8>>}::load"]
def core.sync.atomic.AtomicBoolAlign1U8.load
    (observation : core.sync.atomic.Atomic Bool
      (core.sync.atomic.private.Align1 Aeneas.Std.U8))
    (ordering : ZetesisExtract.core.sync.atomic.Ordering) : Aeneas.Std.Result Bool :=
  match ordering with
  | .Relaxed => .ok observation.nextRead
  | _ => .fail .undef

/-- Return the supplied U64 for a Relaxed read. The extracted membership body
    performs its own comparison with the captured active word. -/
@[rust_fun
  "core::sync::atomic::{core::sync::atomic::Atomic<u64, core::sync::atomic::private::Align8<u64>>}::load"]
def core.sync.atomic.AtomicU64Align8U64.load
    (observation : core.sync.atomic.Atomic Aeneas.Std.U64
      (core.sync.atomic.private.Align8 Aeneas.Std.U64))
    (ordering : ZetesisExtract.core.sync.atomic.Ordering) : Aeneas.Std.Result Aeneas.Std.U64 :=
  match ordering with
  | .Relaxed => .ok observation.nextRead
  | _ => .fail .undef
