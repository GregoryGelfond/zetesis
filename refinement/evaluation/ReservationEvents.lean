import RuntimeRuns
import ReservedStorage
import CheckerContexts
import ContextEvents

open Aeneas Aeneas.Std Aeneas.Data.Coinductive
open ZetesisExtract

/-!
# Returning reservation observations

Each reached reservation consumes its own response. A granted response supplies
capacity and preserves the logical sequence; rejection is a source error. These
are trusted standard-library contracts, not allocator implementation proofs.
Equal logical requests may receive different responses on separate invocations.
The fixed provider below is only a projection of successful calls, not a claim
that physical allocation always succeeds.
-/
namespace ReservationEvents

local instance : MonadLift Result RuntimeEffects.Computation where
  monadLift := RuntimeEffects.embed

/-- The runtime reservation has the extracted operation's argument and return
shape. A refusal's opaque token is unused by the actual source wrapper. -/
def tryReserveExact {T : Type} (_allocator : Type) (vector : alloc.vec.Vec T)
    (additional : Usize) : RuntimeEffects.Computation
      ((core.result.Result Unit alloc.collections.TryReserveError) × alloc.vec.Vec T) :=
  ITree.vis (.reserve vector.val.length additional.val) fun response =>
    match response with
    | .granted _ _ _ => ITree.ret (.Ok (), vector)
    | .rejected _ => ITree.ret (.Err ⟨0⟩, vector)

/-- A successful runtime reservation retains the exact logical vector and
consumes one grant carrying sufficient, machine-bounded capacity. -/
theorem completed_reservation {T : Type} (allocator : Type) (vector after : alloc.vec.Vec T)
    (additional : Usize) (events : List RuntimeEffects.Event)
    (run : RuntimeEffects.Runs (tryReserveExact allocator vector additional) events (.Ok (), after)) :
    after = vector ∧ ∃ capacity enough bounded,
      events = [⟨.reserve vector.val.length additional.val, .granted capacity enough bounded⟩] := by
  obtain ⟨response, tail, partition, following⟩ := RuntimeRuns.observed_inv _ _ _ _ run
  cases response with
  | granted capacity enough bounded =>
      obtain ⟨same, vacant⟩ := RuntimeRuns.returned_inv _ _ _ following
      have retained : vector = after := congrArg Prod.snd same
      exact ⟨retained.symm, capacity, enough, bounded, by simpa [vacant] using partition⟩
  | rejected reason =>
      obtain ⟨impossible, _⟩ := RuntimeRuns.returned_inv _ _ _ following
      cases impossible

/-- A typed reservation error retains the logical input and records a rejection.
A grant cannot be misreported as this error, nor can a nonreturning backend fail. -/
theorem refused_reservation {T : Type} (allocator : Type) (vector after : alloc.vec.Vec T)
    (additional : Usize) (events : List RuntimeEffects.Event)
    (error : alloc.collections.TryReserveError)
    (run : RuntimeEffects.Runs (tryReserveExact allocator vector additional) events (.Err error, after)) :
    after = vector ∧ ∃ reason,
      events = [⟨.reserve vector.val.length additional.val, .rejected reason⟩] := by
  obtain ⟨response, tail, partition, following⟩ := RuntimeRuns.observed_inv _ _ _ _ run
  cases response with
  | granted capacity enough bounded =>
      obtain ⟨impossible, _⟩ := RuntimeRuns.returned_inv _ _ _ following
      cases impossible
  | rejected reason =>
      obtain ⟨same, vacant⟩ := RuntimeRuns.returned_inv _ _ _ following
      have retained : vector = after := congrArg Prod.snd same
      exact ⟨retained.symm, reason, by simpa [vacant] using partition⟩

/-- A logical fixed provider for normalization of successful runtime calls. It
preserves the sequence and rejects impossible element counts. A runtime grant
justifies the success branch for each call being projected; no runtime success
is inferred by choosing this provider. -/
@[reducible] def fixed : VectorReservation where
  reserve := fun _ vector additional =>
    if vector.val.length + additional.val ≤ Usize.max then
      Result.ok (.Ok (), vector)
    else Result.ok (.Err ⟨0⟩, vector)

/-- A successful observed reservation has the same result under the fixed
logical provider. Its capacity certificate discharges that provider's bound. -/
theorem completed_projection {T : Type} (allocator : Type) (vector after : alloc.vec.Vec T)
    (additional : Usize) (events : List RuntimeEffects.Event)
    (run : RuntimeEffects.Runs (tryReserveExact allocator vector additional) events (.Ok (), after)) :
    @alloc.vec.Vec.try_reserve_exact T fixed allocator vector additional =
      Result.ok (.Ok (), after) := by
  obtain ⟨retained, capacity, enough, bounded, _⟩ :=
    completed_reservation allocator vector after additional events run
  have fits : vector.val.length + additional.val ≤ Usize.max := Nat.le_trans enough bounded
  subst after
  change (if vector.val.length + additional.val ≤ Usize.max then
    Result.ok (core.result.Result.Ok (), vector) else
    Result.ok (core.result.Result.Err (⟨0⟩ : alloc.collections.TryReserveError), vector)) = _
  simp only [fits, ↓reduceIte]

/-- The actual reservation wrapper with a returning reservation observation.
Every source branch and the opaque-error mapping are retained by the checked
context; no second reservation algorithm is introduced. -/
def reserve (T : Type) (count : Usize) : RuntimeEffects.Computation
    (core.result.Result (alloc.vec.Vec T) zetesis_cpu.cancellation.Stop) :=
  CheckerContexts.reserve tryReserveExact T count

/-- The source wrapper maps a reached rejection to Allocation and retains a
reached grant's vector. This exact equation preserves the external call order. -/
theorem reserve_exact (T : Type) (count : Usize) :
    reserve T count =
      ITree.bind (tryReserveExact Global (alloc.vec.Vec.new T) count) (fun returned =>
        ITree.ret (match returned.1 with
          | .Ok _ => .Ok returned.2
          | .Err _ => .Err .Allocation)) := by
  unfold reserve CheckerContexts.reserve
  change ITree.bind _ _ = ITree.bind _ _
  congr 1
  funext returned
  rcases returned with ⟨answer, vector⟩
  cases answer with
  | Ok value =>
    cases value
    simp [core.result.Result.map_err, core.result.Result.Insts.CoreOpsTry.branch,
      ContextEvents.lift_result, RuntimeEffects.embed_ok, Bind.bind, itree_ret_bind]
  | Err error =>
    simp [core.result.Result.map_err,
      oracle.reserve.closure.Insts.CoreOpsFunctionFnOnceTupleTryReserveErrorStop.call_once,
      core.result.Result.Insts.CoreOpsTry.branch,
      core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual,
      core.convert.FromSame.from, ContextEvents.lift_result, RuntimeEffects.embed_ok,
      Bind.bind, itree_ret_bind]

/-- Successful runtime reservation reaches the actual fixed-provider source
wrapper with the same vector. Completion, rather than an allocation-success
assumption, supplies the necessary grant. -/
theorem completed_wrapper (T : Type) (count : Usize) (vector : alloc.vec.Vec T)
    (events : List RuntimeEffects.Event)
    (run : RuntimeEffects.Runs (reserve T count) events (.Ok vector)) :
    @oracle.reserve fixed T count = Result.ok (.Ok vector) := by
  rw [reserve_exact] at run
  obtain ⟨before, after, returned, _, reserved, following⟩ :=
    RuntimeRuns.bind_inv _ _ _ _ run
  obtain ⟨same, _⟩ := RuntimeRuns.returned_inv _ _ _ following
  rcases returned with ⟨answer, retained⟩
  cases answer with
  | Err error => cases same
  | Ok value =>
    cases value
    have identical : retained = vector := core.result.Result.Ok.inj same
    subst retained
    have generated := completed_projection Global (alloc.vec.Vec.new T) vector count before reserved
    rw [@ReservedStorage.reserve_exact fixed T count, generated]
    simp only [bind_tc_ok, uncurry]

/-- A typed wrapper refusal is precisely Allocation and has a matching reached
rejection. No successful source result, backend failure or divergence is silently
converted to that stop. -/
theorem refused_wrapper (T : Type) (count : Usize) (events : List RuntimeEffects.Event)
    (reason : zetesis_cpu.cancellation.Stop)
    (run : RuntimeEffects.Runs (reserve T count) events (.Err reason)) :
    reason = .Allocation ∧ ∃ error,
      events = [⟨.reserve 0 count.val, .rejected error⟩] := by
  rw [reserve_exact] at run
  obtain ⟨before, after, returned, partition, reserved, following⟩ :=
    RuntimeRuns.bind_inv _ _ _ _ run
  obtain ⟨same, silent⟩ := RuntimeRuns.returned_inv _ _ _ following
  rcases returned with ⟨answer, retained⟩
  cases answer with
  | Ok value => cases same
  | Err error =>
    have stopped : reason = .Allocation := (core.result.Result.Err.inj same).symm
    obtain ⟨_, rejected, receipt⟩ :=
      refused_reservation Global (alloc.vec.Vec.new T) retained count before error reserved
    refine ⟨stopped, rejected, ?_⟩
    rw [silent, List.append_nil, receipt] at partition
    exact partition

/-- Equal logical reservation requests can first succeed and then be refused.
Both calls use the actual wrapper context; the second rejection becomes its
Allocation result. This history cannot be represented by one deterministic
provider applied twice to equal inputs. -/
theorem repeated_request_may_refuse (T : Type) (count : Usize) :
    RuntimeEffects.Runs (do
      let first ← reserve T count
      let second ← reserve T count
      pure (first, second))
      [⟨.reserve 0 count.val, .granted count.val (by omega) (by scalar_tac)⟩,
        ⟨.reserve 0 count.val, .rejected .refused⟩]
      (.Ok (alloc.vec.Vec.new T), .Err .Allocation) := by
  rw [reserve_exact]
  simp only [tryReserveExact, Bind.bind, itree_vis_bind]
  change RuntimeEffects.Runs (ITree.vis (.reserve 0 count.val) _) _ _
  refine RuntimeEffects.Runs.observed (.reserve 0 count.val)
    (.granted count.val (by omega) (by scalar_tac)) _ _ _ ?_
  simp only [itree_ret_bind]
  change RuntimeEffects.Runs (ITree.vis (.reserve 0 count.val) _) _ _
  refine RuntimeEffects.Runs.observed (.reserve 0 count.val)
    (.rejected .refused) _ _ _ ?_
  simp only [itree_ret_bind, Pure.pure]
  exact .returned _

/-- The fixed logical projection preserves the input sequence whenever it
returns success. This is a law of that projection, not an allocation guarantee. -/
theorem fixed_preserves {T : Type} (allocator : Type) (before after : alloc.vec.Vec T)
    (count : Usize)
    (completed : @alloc.vec.Vec.try_reserve_exact T fixed allocator before count =
      Result.ok (.Ok (), after)) : after.val = before.val := by
  change (if before.val.length + count.val ≤ Usize.max then
    Result.ok (core.result.Result.Ok (), before) else
    Result.ok (core.result.Result.Err (⟨0⟩ : alloc.collections.TryReserveError), before)) = _ at completed
  split at completed
  · have pair := Result.ok_injective completed
    exact (congrArg (fun result => result.2.val) pair).symm
  · have pair := Result.ok_injective completed
    cases pair

end ReservationEvents
