import Evaluator.Types

/-!
# A relaxed read in the one-invocation observation model

`Evaluator.TypesExternal` must import `AtomicTypes` before the unchanged generated
`Evaluator.Types` is checked. This module then supplies the exact missing load
name used by the unchanged generated functions.

Only the production poll's Relaxed ordering has a modeled observation. Other
orderings report `undef` as an explicit boundary of this proof model. In
particular, that branch does not assert that a Rust Acquire or SeqCst load fails.
The operation consumes no logical state: it reads a supplied one-use observation
token. The proof scope, rather than the Lean type system, enforces one invocation
and at most one read at each observed flag site. No whole-loop or concurrent
atomic-memory semantics follows from this definition.
-/

/-- Replay the supplied Boolean for this one Relaxed read site. Unsupported
    orderings remain explicit model failure; no successful observation is
    manufactured for them. -/
@[rust_fun
  "core::sync::atomic::{core::sync::atomic::Atomic<bool, core::sync::atomic::private::Align1<u8>>}::load"]
def core.sync.atomic.AtomicBoolAlign1U8.load
    (observation : core.sync.atomic.Atomic Bool
      (core.sync.atomic.private.Align1 Aeneas.Std.U8))
    (ordering : ZetesisExtract.core.sync.atomic.Ordering) : Aeneas.Std.Result Bool :=
  match ordering with
  | .Relaxed => .ok observation.nextRead
  | _ => .fail .undef
