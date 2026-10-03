import Aeneas
import Evaluator.Types

open Aeneas Aeneas.Std Aeneas.Data.Coinductive

/-!
# Returning runtime observations

This separately checked specification uses the pinned backend's generic interaction tree. A
read request names an immutable object handle; successive responses may differ.
A reservation request returns either a capacity certificate or a source-level
error. Its failure is not the backend's nonreturning failure effect.

The generated `Aeneas.Std.Result` fixes a smaller effect signature. Audited
source contexts instantiate this effectful interface without changing generated
bodies; separate projection and refusal laws connect their finite executions to
the reference checker. The relation below records exactly the responses consumed.
It makes no scheduling, allocator implementation or Rust memory-model claim.
-/
namespace RuntimeEffects

/-- An observed reservation error is opaque to the source wrapper, which maps
all such errors to `Stop::Allocation`. No allocator internals are represented. -/
inductive ReservationError where
  | refused
  deriving DecidableEq

/-- A successful observation supplies enough element capacity. The machine
length bound is explicit; byte layout and allocator behavior remain external
contract obligations. Rejection supplies no capacity certificate. -/
inductive ReservationResponse (required : Nat) where
  | granted (capacity : Nat) (enough : required ≤ capacity)
      (bounded : capacity ≤ Usize.max)
  | rejected (reason : ReservationError)

/-- Requests preserve the observed object and ordering for reads, or the current
logical length and requested additional capacity for reservations. -/
inductive Request where
  | read (object : Nat) (ordering : ZetesisExtract.core.sync.atomic.Ordering)
  | reserve (length additional : Nat)
  | fail (error : Aeneas.Std.Error)

abbrev Response : Request → Type
  | .read _ _ => Bool
  | .reserve length additional => ReservationResponse (length + additional)
  | .fail _ => PEmpty

abbrev effects : Effect := { I := Request, O := Response }
abbrev Computation (T : Type) := ITree effects T
abbrev Event := (request : Request) × Response request

/-- A finite execution records every returning external observation. Divergence,
an unanswered request and nonreturning backend failure have no completed run. -/
inductive Runs {T : Type} : Computation T → List Event → T → Prop where
  | returned (value : T) : Runs (.ret value) [] value
  | observed (request : Request) (response : Response request)
      (next : Response request → Computation T) (events : List Event) (value : T) :
      Runs (next response) events value →
      Runs (.vis request next) (⟨request, response⟩ :: events) value

/-- Sequential composition concatenates exactly the consumed observations.
The proof follows the first run: its return starts the second run, and each
external response stays in front of the recursively composed remainder. -/
theorem runs_bind {T U : Type} {first : Computation T}
    {next : T → Computation U} {before after : List Event} {value : T} {result : U}
    (firstRun : Runs first before value) (nextRun : Runs (next value) after result) :
    Runs (ITree.bind first next) (before ++ after) result := by
  induction firstRun with
  | returned value => simpa using nextRun
  | observed request response continuation events value run inductionHypothesis =>
      rw [itree_vis_bind]
      exact Runs.observed request response (fun answer => ITree.bind (continuation answer) next)
          (events ++ after) result (inductionHypothesis nextRun)

/-- The loop combinator specialized to returning runtime effects. Its control
shape matches the backend loop; `RuntimeLoop` separately proves finite completed
projection under explicit local correspondence and observation-progress laws. -/
def loop {T U : Type} (body : T → Computation (ControlFlow T U)) (state : T) :
    Computation U := do
  let transition ← body state
  match transition with
  | .cont next => loop body next
  | .done value => pure value
partial_fixpoint

/-- An actual finite body run ending in continuation composes with the following
loop run, consuming precisely their concatenated responses. -/
theorem loop_continues {T U : Type} (body : T → Computation (ControlFlow T U))
    (state next : T) (before after : List Event) (value : U)
    (bodyRun : Runs (body state) before (.cont next))
    (following : Runs (loop body next) after value) :
    Runs (loop body state) (before ++ after) value := by
  rw [loop]
  exact runs_bind bodyRun following

/-- A body run returning done completes the effectful loop without consuming
another response. The returned value can itself be a source-level typed stop. -/
theorem loop_finishes {T U : Type} (body : T → Computation (ControlFlow T U))
    (state : T) (events : List Event) (value : U)
    (bodyRun : Runs (body state) events (.done value)) :
    Runs (loop body state) events value := by
  rw [loop, ← List.append_nil events]
  exact runs_bind bodyRun (Runs.returned value)

/-- Embed an existing backend computation without dropping its non-success
cases. A backend failure stays nonreturning failure; divergence stays divergence.
The old effect has no returning observations, so no event history is invented. -/
def embed {T : Type} (computation : Aeneas.Std.Result T) : Computation T :=
  match computation.match with
  | .ok value => .ret value
  | .div => .div
  | .vis (.fail error) _ => .vis (.fail error) PEmpty.elim

/-- A pure backend return embeds without external observations. -/
theorem embed_ok {T : Type} (value : T) :
    embed (Aeneas.Std.Result.ok value) = ITree.ret value := by
  simp only [embed, Aeneas.Std.Result.match.ok]

/-- Backend failure is preserved, including its original error. It is not
confused with a source-level `Result.Err` returned by fallible reservation. -/
theorem embed_failure {T : Type} (error : Aeneas.Std.Error) :
    embed (Aeneas.Std.Result.fail error : Aeneas.Std.Result T) =
      ITree.vis (E := effects) (.fail error) PEmpty.elim := by
  simp only [embed, Aeneas.Std.Result.match.fail]

/-- Divergence is retained rather than reported as failure or successful return. -/
theorem embed_divergence {T : Type} :
    embed (Aeneas.Std.Result.div : Aeneas.Std.Result T) = ITree.div := by
  simp only [embed, Aeneas.Std.Result.match.div]

/-- The embedding commutes with sequential composition. Existing checked scalar
and vector operations can therefore be lifted without changing their pure,
failure or divergence behavior. The proof considers the backend's three cases;
the only visible backend event has no possible response. -/
theorem embed_bind {T U : Type} (computation : Aeneas.Std.Result T)
    (next : T → Aeneas.Std.Result U) :
    embed (Aeneas.Std.bind computation next) =
      ITree.bind (embed computation) (fun value => embed (next value)) := by
  cases computation with
  | ret value => simp only [Aeneas.Std.bind_ok, embed_ok, itree_ret_bind]
  | div => simp only [Aeneas.Std.bind_div, embed_divergence, itree_div_bind]
  | vis request continuation =>
      cases request with
      | fail error =>
          simp only [Aeneas.Std.bind_vis, embed, Aeneas.Std.Result.match.vis, itree_vis_bind]
          congr 1
          funext impossible
          exact PEmpty.elim impossible

/-- A finite sequence of actual immediate-result calls under the current
backend monad. This records body equations, not an assumed whole-loop result. -/
inductive PureCalls {T U : Type} (body : T → Aeneas.Std.Result (ControlFlow T U)) :
    T → U → Prop where
  | done {state : T} {value : U}
      (step : body state = Aeneas.Std.Result.ok (.done value)) :
      PureCalls body state value
  | next {state following : T} {value : U}
      (step : body state = Aeneas.Std.Result.ok (.cont following))
      (rest : PureCalls body following value) : PureCalls body state value

/-- The current Aeneas loop and its eventful specialization agree on finite
sequences of immediate-result body calls. The actual backend loop equation is
derived by unfolding it; the lifted run uses bind-compatible embedding and
consumes no observations. This does not add changing effects to the old loop. -/
theorem pure_loop_compatible {T U : Type}
    (body : T → Aeneas.Std.Result (ControlFlow T U)) {state : T} {value : U}
    (calls : PureCalls body state value) :
    Aeneas.Std.loop body state = Aeneas.Std.Result.ok value ∧
    Runs (loop (fun current => embed (body current)) state) [] value := by
  induction calls with
  | done step =>
      constructor
      · rw [Aeneas.Std.loop, step]
        simp only [Aeneas.Std.bind_ok]
      · apply loop_finishes
        rw [step, embed_ok]
        exact Runs.returned _
  | @next state following value step rest inductionHypothesis =>
      obtain ⟨executed, lifted⟩ := inductionHypothesis
      constructor
      · rw [Aeneas.Std.loop, step]
        simpa only [Aeneas.Std.bind_ok] using executed
      · have bodyRun : Runs (embed (body state)) [] (.cont following) := by
          rw [step, embed_ok]
          exact Runs.returned _
        exact loop_continues (fun current => embed (body current)) state following [] [] value bodyRun lifted

/-- An immutable atomic handle identifies the observed object. Its next value
comes from a request, not from mutable state hidden in a pure definition. -/
structure Atomic where
  object : Nat

/-- This has the generated load's ordinary arguments and Boolean result shape.
Its effectful result type is deliberately distinct from the generated Result.
Only the used Relaxed ordering is modeled; other orderings are an explicit
model boundary, not a claim that those orderings necessarily fail in Rust. -/
def load (cell : Atomic) (ordering : ZetesisExtract.core.sync.atomic.Ordering) :
    Computation Bool :=
  match ordering with
  | .Relaxed => .vis (.read cell.object .Relaxed) .ret
  | _ => .vis (.fail .undef) PEmpty.elim

/-- The existing vector is preserved logically on either reservation outcome.
A successful event carries capacity evidence in the execution record; it does
not add a capacity field to the pinned backend's sequence representation. -/
def tryReserveExact {T : Type} (_allocator : Type) (vector : alloc.vec.Vec T)
    (additional : Usize) :
    Computation ((core.result.Result Unit ReservationError) × alloc.vec.Vec T) :=
  .vis (.reserve vector.val.length additional.val) fun response =>
    match response with
    | .granted _ _ _ => .ret (.Ok (), vector)
    | .rejected reason => .ret (.Err reason, vector)

/-- Model the source wrapper's error mapping, followed by a read only after
reservation succeeds. This is an authored feasibility example, not an extracted
function or a proof of capacity preservation across later vector operations. -/
def reserveThenRead (count : Usize) (cell : Atomic) :
    Computation (core.result.Result Bool ZetesisExtract.zetesis_cpu.cancellation.Stop) := do
  let (answer, _) ← tryReserveExact Global (alloc.vec.Vec.new Bool) count
  match answer with
  | .Err _ => pure (.Err .Allocation)
  | .Ok _ =>
      let value ← load cell .Relaxed
      pure (.Ok value)

/-- Two uses of the same handle consume two responses. Object identity does not
force the observed values to be equal. -/
def readTwice (cell : Atomic) : Computation (Bool × Bool) := do
  let first ← load cell .Relaxed
  let second ← load cell .Relaxed
  pure (first, second)

/-- The same object can be observed clear and then set, with no handle mutation
or replacement between the reads. -/
theorem distinct_reads (cell : Atomic) :
    Runs (readTwice cell)
      [⟨.read cell.object .Relaxed, false⟩, ⟨.read cell.object .Relaxed, true⟩]
      (false, true) := by
  simp only [readTwice, load, Bind.bind, itree_vis_bind, Pure.pure]
  refine Runs.observed (.read cell.object .Relaxed) false _ _ _ ?_
  simp only [itree_ret_bind]
  refine Runs.observed (.read cell.object .Relaxed) true _ _ _ ?_
  exact Runs.returned _

/-- Allocation refusal becomes the source's typed stop and consumes no following
read. In particular it is neither an outer backend failure nor a success axiom. -/
theorem reservation_refusal (count : Usize) (cell : Atomic) :
    Runs (reserveThenRead count cell)
      [⟨.reserve 0 count.val, .rejected .refused⟩] (.Err .Allocation) := by
  simp only [reserveThenRead, tryReserveExact, Bind.bind, itree_vis_bind]
  change Runs (.vis (.reserve 0 count.val) _) _ _
  refine Runs.observed (.reserve 0 count.val) (.rejected .refused) _ _ _ ?_
  simp only [itree_ret_bind, Pure.pure]
  exact Runs.returned _

/-- The successful reservation branch is nonvacuous and permits the next read.
The response certifies the requested capacity while the logical vector remains
empty; this does not claim an implementation of Rust's allocator. -/
theorem reservation_success (count : Usize) (cell : Atomic) (value : Bool) :
    Runs (reserveThenRead count cell)
      [⟨.reserve 0 count.val, .granted count.val (by omega) (by scalar_tac)⟩,
       ⟨.read cell.object .Relaxed, value⟩] (.Ok value) := by
  simp only [reserveThenRead, tryReserveExact, Bind.bind, itree_vis_bind]
  change Runs (.vis (.reserve 0 count.val) _) _ _
  refine Runs.observed (.reserve 0 count.val) (.granted count.val (by omega) (by scalar_tac)) _ _ _ ?_
  simp only [itree_ret_bind, load, itree_vis_bind]
  refine Runs.observed (.read cell.object .Relaxed) value _ _ _ ?_
  simp only [Pure.pure]
  exact Runs.returned _

/-- A small loop counts clear observations and returns cancellation at the first
set observation. It uses one unchanged object handle throughout. This is an
operational feasibility example, not the extracted work-budget loop. -/
def observeUntilCancelled (cell : Atomic) (visited : Nat) :
    Computation (ControlFlow Nat
      (core.result.Result Unit ZetesisExtract.zetesis_cpu.cancellation.Stop × Nat)) := do
  let cancelled ← load cell .Relaxed
  pure (if cancelled then .done (.Err .Cancelled, visited) else .cont (visited + 1))

/-- The same handle is first clear and later cancelled in a genuine effectful
loop execution. Exactly one clear visit is counted before typed refusal. -/
theorem loop_observes_change (cell : Atomic) :
    Runs (loop (observeUntilCancelled cell) 0)
      [⟨.read cell.object .Relaxed, false⟩, ⟨.read cell.object .Relaxed, true⟩]
      (.Err .Cancelled, 1) := by
  have first : Runs (observeUntilCancelled cell 0)
      [⟨.read cell.object .Relaxed, false⟩] (.cont 1) := by
    simp only [observeUntilCancelled, load, Bind.bind, itree_vis_bind]
    refine Runs.observed (.read cell.object .Relaxed) false _ _ _ ?_
    simp only [itree_ret_bind, Bool.false_eq_true, ↓reduceIte, Nat.zero_add, Pure.pure]
    exact Runs.returned _
  have second : Runs (observeUntilCancelled cell 1)
      [⟨.read cell.object .Relaxed, true⟩] (.done (.Err .Cancelled, 1)) := by
    simp only [observeUntilCancelled, load, Bind.bind, itree_vis_bind]
    refine Runs.observed (.read cell.object .Relaxed) true _ _ _ ?_
    simp only [itree_ret_bind, ↓reduceIte, Pure.pure]
    exact Runs.returned _
  exact loop_continues (observeUntilCancelled cell) 0 1 _ _ _ first
    (loop_finishes (observeUntilCancelled cell) 1 _ _ second)

end RuntimeEffects
