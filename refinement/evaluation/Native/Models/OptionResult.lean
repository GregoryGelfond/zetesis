import Aeneas

/-!
# Pure Option and Result bindings reached by native extraction

These definitions supply the three additional pure standard-library signatures
in the native external template. They preserve the source constructors and the
backend Result wrapper. No successful callback, allocation or semantic result
is assumed. The impossible residual is eliminated by its empty type.

They live in the native generated namespace, leaving the legacy model and its
recorded scope unchanged. Correspondence with Rust's standard library remains
an explicit external-model boundary, not an extracted standard-library proof.
-/
namespace ZetesisNativeExtract

/-- Option's Try branch continues exactly on Some and breaks with None on None. -/
def core.option.Option.Insts.CoreOpsTry_traitTry.branch {T : Type}
    (value : Option T) : Aeneas.Std.Result
      (Aeneas.Std.core.ops.control_flow.ControlFlow
        (Option Aeneas.Std.core.convert.Infallible) T) :=
  match value with
  | some item => .ok (.Continue item)
  | none => .ok (.Break none)

/-- Only None is a constructible Option residual; propagate it unchanged. -/
def core.option.Option.Insts.CoreOpsTry_traitFromResidualOptionInfallible.from_residual
    (T : Type) (residual : Option Aeneas.Std.core.convert.Infallible) :
    Aeneas.Std.Result (Option T) :=
  match residual with
  | none => .ok none
  | some impossible => nomatch impossible

/-- Source Ok retains its payload; source Err yields absence, with no invented
backend failure and no assertion about the error's semantic meaning. -/
def core.result.Result.ok {T E : Type}
    (value : Aeneas.Std.core.result.Result T E) : Aeneas.Std.Result (Option T) :=
  match value with
  | .Ok item => .ok (some item)
  | .Err _ => .ok none

end ZetesisNativeExtract
