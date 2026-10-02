import Evaluator.Types

/-!
# A relaxed read from a supplied observation token

`Evaluator.TypesExternal` must import `AtomicTypes` before the unchanged generated
`Evaluator.Types` is checked. This module then supplies the exact missing load
name used by the unchanged generated functions.

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
