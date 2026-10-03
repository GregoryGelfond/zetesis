import Evaluator.Funs
import VectorReservation

/-!
# The extracted reservation wrapper

These laws follow the actual `oracle.reserve` body. The external reservation's
source refusal becomes `Stop.Allocation`; backend failure and divergence remain
in the outer result. A completed wrapper success recovers the actual successful
external call without assuming allocation success or unchanged contents.

Only `returned_empty` uses a sequence-preservation contract. It says nothing
about capacity or allocation identity, which the backend vector does not store.
All statements use one supplied reservation provider, with the per-invocation
scope documented in `VectorReservation`.
-/

open Aeneas Aeneas.Std Result
open ZetesisExtract

namespace ReservedStorage

variable [reservation : VectorReservation]

/-- The wrapper performs one supplied reservation on an empty logical vector.
It returns that call's vector on source success and `Stop.Allocation` on source
refusal. The exact bind equation also retains backend failure and divergence. -/
theorem reserve_exact (T : Type) (count : Usize) :
    oracle.reserve T count =
      (do
        let (verdict, vector) ←
          alloc.vec.Vec.try_reserve_exact Global (alloc.vec.Vec.new T) count
        match verdict with
        | .Ok _ => ok (.Ok vector)
        | .Err _ => ok (.Err zetesis_cpu.cancellation.Stop.Allocation)) := by
  unfold oracle.reserve
  congr 1
  funext returned
  rcases returned with ⟨verdict, vector⟩
  cases verdict with
  | Ok value =>
      cases value
      simp [core.result.Result.map_err, core.result.Result.Insts.CoreOpsTry.branch]
  | Err error =>
      simp [core.result.Result.map_err,
        oracle.reserve.closure.Insts.CoreOpsFunctionFnOnceTupleTryReserveErrorStop.call_once,
        core.result.Result.Insts.CoreOpsTry.branch,
        core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual,
        core.convert.FromSame.from]

/-- A returned vector came from the actual successful reservation call. No
reservation outcome, empty-vector property, or capacity guarantee is assumed.
Proof: a source refusal, backend failure, or divergence cannot yield success. -/
theorem completed_reservation (T : Type) (count : Usize) (vector : alloc.vec.Vec T)
    (completed : oracle.reserve T count = ok (.Ok vector)) :
    alloc.vec.Vec.try_reserve_exact Global (alloc.vec.Vec.new T) count =
      ok (.Ok (), vector) := by
  rw [reserve_exact] at completed
  cases outcome : alloc.vec.Vec.try_reserve_exact Global (alloc.vec.Vec.new T) count with
  | ret pair =>
      rcases pair with ⟨verdict, returned⟩
      cases verdict with
      | Ok value =>
          cases value
          have same : returned = vector := by
            simpa [outcome] using completed
          simp only [same]
      | Err error =>
          simp [outcome] at completed
  | vis effect continuation =>
      simp only [outcome, bind_tc_vis, vis_not_ok] at completed
  | div =>
      simp only [outcome, bind_tc_div, div_not_ok] at completed

/-- Any typed wrapper refusal is exactly `Stop.Allocation` and witnesses an
actual source reservation refusal. Backend failure and divergence cannot be
misreported as this typed stop. The refused call's vector remains in the witness. -/
theorem returned_refusal (T : Type) (count : Usize) (reason : zetesis_cpu.cancellation.Stop)
    (completed : oracle.reserve T count = ok (.Err reason)) :
    reason = .Allocation ∧
      ∃ error vector,
        alloc.vec.Vec.try_reserve_exact Global (alloc.vec.Vec.new T) count =
          ok (.Err error, vector) := by
  rw [reserve_exact] at completed
  cases outcome : alloc.vec.Vec.try_reserve_exact Global (alloc.vec.Vec.new T) count with
  | ret pair =>
      rcases pair with ⟨verdict, vector⟩
      cases verdict with
      | Ok value =>
          simp [outcome] at completed
      | Err error =>
          have same : zetesis_cpu.cancellation.Stop.Allocation = reason := by
            simpa [outcome] using completed
          exact ⟨same.symm, error, vector, rfl⟩
  | vis effect continuation =>
      simp only [outcome, bind_tc_vis, vis_not_ok] at completed
  | div =>
      simp only [outcome, bind_tc_div, div_not_ok] at completed

/-- A successful wrapper returns an empty logical sequence when its external
call preserves input contents on success. This explicit library contract gives
no information about the Rust vector's capacity or allocation identity. -/
theorem returned_empty (T : Type) (count : Usize) (vector : alloc.vec.Vec T)
    (preservesSequence : ∀ before after : alloc.vec.Vec T,
      alloc.vec.Vec.try_reserve_exact Global before count = ok (.Ok (), after) →
        after.val = before.val)
    (completed : oracle.reserve T count = ok (.Ok vector)) :
    vector.val = [] := by
  have reserved : alloc.vec.Vec.try_reserve_exact Global (alloc.vec.Vec.new T) count =
      ok (.Ok (), vector) := completed_reservation T count vector completed
  have unchanged : vector.val = (alloc.vec.Vec.new T).val :=
    preservesSequence (alloc.vec.Vec.new T) vector reserved
  exact unchanged

end ReservedStorage
