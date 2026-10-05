import RuntimeRuns

open Aeneas Aeneas.Std Aeneas.Data.Coinductive
open ZetesisExtract RuntimeEffects

/-!
# Exact receipts for the control-read sequence

A poll first reads local cancellation, then its optional slot word, then its
optional deadline. A slot is identified by its U64 read owner and the immutable
active word captured by the token. The observed word is compared with that word
in the computation itself. Neither a successful comparison nor a poll outcome is
assumed.

These definitions are a normal form for eventful polling, not another Rust
control representation. The source-checked context must separately be proved
equal to `poll`. The finite-run laws below then apply to that exact context.
No claim about compare-and-swap, generation allocation or scheduling is made.
-/
namespace ControlReads

/-- Exactly one matching word is read when the token has a slot membership. -/
def matchingSlot (slot : Option (Nat × U64)) : List Event :=
  match slot with
  | none => []
  | some (object, active) => [⟨.readU64 object .Relaxed, active⟩]

/-- Exactly one clear expiry bit is read when a deadline exists. -/
def clearDeadline (deadline : Option Nat) : List Event :=
  match deadline with
  | none => []
  | some object => [⟨.read object .Relaxed, false⟩]

/-- The final read stage reached only after both cancellation checks are clear. -/
def finish (deadline : Option Nat) :
    Computation (core.result.Result Unit zetesis_cpu.cancellation.Stop) :=
  match deadline with
  | none => .ret (.Ok ())
  | some object => .vis (.read object .Relaxed) fun expired =>
      .ret (if expired then .Err .Deadline else .Ok ())

/-- The optional slot contributes its actual word. A mismatch stops before the
expiry stage; absent membership makes no request. -/
def afterLocal (slot : Option (Nat × U64)) (deadline : Option Nat) :
    Computation (core.result.Result Unit zetesis_cpu.cancellation.Stop) :=
  match slot with
  | none => finish deadline
  | some (object, active) => .vis (.readU64 object .Relaxed) fun observed =>
      if observed != active then .ret (.Err .Cancelled) else finish deadline

/-- The read sequence preserves local-cancellation, slot-cancellation and expiry
precedence. The first request occurs for every returning poll. -/
def poll (cancel : Nat) (slot : Option (Nat × U64)) (deadline : Option Nat) :
    Computation (core.result.Result Unit zetesis_cpu.cancellation.Stop) :=
  .vis (.read cancel .Relaxed) fun cancelled =>
    if cancelled then .ret (.Err .Cancelled) else afterLocal slot deadline

/-- A complete poll receipt records its source cause, response and read order.
A matching slot read is required for success and expiry. Slot cancellation keeps
the mismatching U64 response, rather than only recording a derived Boolean. -/
inductive Receipt (cancel : Nat) (slot : Option (Nat × U64)) (deadline : Option Nat) :
    List Event → core.result.Result Unit zetesis_cpu.cancellation.Stop → Prop where
  | clear : Receipt cancel slot deadline
      (⟨.read cancel .Relaxed, false⟩ :: (matchingSlot slot ++ clearDeadline deadline)) (.Ok ())
  | cancelled : Receipt cancel slot deadline
      [⟨.read cancel .Relaxed, true⟩] (.Err .Cancelled)
  | mismatched (object : Nat) (active observed : U64)
      (present : slot = some (object, active)) (different : observed ≠ active) :
      Receipt cancel slot deadline
        [⟨.read cancel .Relaxed, false⟩, ⟨.readU64 object .Relaxed, observed⟩]
        (.Err .Cancelled)
  | expired (object : Nat) (present : deadline = some object) :
      Receipt cancel slot deadline
        (⟨.read cancel .Relaxed, false⟩ :: (matchingSlot slot ++
          [⟨.read object .Relaxed, true⟩])) (.Err .Deadline)

/-- Every returned execution has exactly its indicated read receipt. Invert the
local read first. A reached slot supplies its actual word; only equality with
the captured active word allows the deadline stage to run. No fixed observation
value or control verdict is a premise. -/
theorem receipt_of_run (cancel : Nat) (slot : Option (Nat × U64)) (deadline : Option Nat)
    (events : List Event) (answer : core.result.Result Unit zetesis_cpu.cancellation.Stop)
    (run : Runs (poll cancel slot deadline) events answer) :
    Receipt cancel slot deadline events answer := by
  have finishReceipt (tail : List Event)
      (finished : Runs (finish deadline) tail answer) :
      Receipt cancel slot deadline
        (⟨.read cancel .Relaxed, false⟩ :: (matchingSlot slot ++ tail)) answer := by
    cases configured : deadline with
    | none =>
      simp only [finish, configured] at finished
      obtain ⟨same, ended⟩ := RuntimeRuns.returned_inv _ _ _ finished
      subst answer
      subst tail
      simpa only [clearDeadline, configured] using
        (Receipt.clear (cancel := cancel) (slot := slot) (deadline := deadline))
    | some object =>
      simp only [finish, configured] at finished
      obtain ⟨expired, rest, consumed, continued⟩ := RuntimeRuns.observed_inv _ _ _ _ finished
      cases expired with
      | false =>
        simp only [Bool.false_eq_true, ↓reduceIte] at continued
        obtain ⟨same, ended⟩ := RuntimeRuns.returned_inv _ _ _ continued
        subst answer
        subst rest
        rw [consumed]
        simpa only [clearDeadline, configured] using
          (Receipt.clear (cancel := cancel) (slot := slot) (deadline := deadline))
      | true =>
        simp only [↓reduceIte] at continued
        obtain ⟨same, ended⟩ := RuntimeRuns.returned_inv _ _ _ continued
        subst answer
        subst rest
        rw [consumed]
        exact Receipt.expired object rfl
  unfold poll at run
  obtain ⟨cancelled, tail, consumed, continued⟩ := RuntimeRuns.observed_inv _ _ _ _ run
  cases cancelled with
  | true =>
    obtain ⟨same, ended⟩ := RuntimeRuns.returned_inv _ _ _ continued
    subst answer
    subst tail
    rw [consumed]
    exact Receipt.cancelled
  | false =>
    cases configured : slot with
    | none =>
      simp only [afterLocal, configured, Bool.false_eq_true, ↓reduceIte] at continued
      rw [consumed]
      simpa only [matchingSlot, configured, List.nil_append] using finishReceipt tail continued
    | some membership =>
      rcases membership with ⟨object, active⟩
      simp only [afterLocal, configured, Bool.false_eq_true, ↓reduceIte] at continued
      obtain ⟨observed, rest, readWord, following⟩ := RuntimeRuns.observed_inv _ _ _ _ continued
      by_cases matching : observed = active
      · subst observed
        simp only [bne_self_eq_false, Bool.false_eq_true, ↓reduceIte] at following
        rw [consumed, readWord]
        simpa only [matchingSlot, configured, List.cons_append, List.nil_append] using
          finishReceipt rest following
      · have differs : (observed != active) = true := by simp [matching]
        rw [differs] at following
        obtain ⟨same, ended⟩ := RuntimeRuns.returned_inv _ _ _ following
        subst answer
        subst rest
        rw [consumed, readWord]
        exact Receipt.mismatched object active observed rfl matching

/-- Every receipt is realized by the read computation. Responses remain
unconstrained except for the explicit equality or mismatch recorded by that
receipt; a local cancellation needs neither a slot nor a deadline response. -/
theorem run_of_receipt (cancel : Nat) (slot : Option (Nat × U64)) (deadline : Option Nat)
    (events : List Event) (answer : core.result.Result Unit zetesis_cpu.cancellation.Stop)
    (receipt : Receipt cancel slot deadline events answer) :
    Runs (poll cancel slot deadline) events answer := by
  have finishClear : Runs (finish deadline) (clearDeadline deadline) (.Ok ()) := by
    cases deadline with
    | none => exact .returned _
    | some object =>
      exact .observed (.read object .Relaxed) false _ _ _ (.returned _)
  have matchingPrefix (tail : List Event)
      (result : core.result.Result Unit zetesis_cpu.cancellation.Stop)
      (finished : Runs (finish deadline) tail result) :
      Runs (afterLocal slot deadline) (matchingSlot slot ++ tail) result := by
    cases slot with
    | none => exact finished
    | some membership =>
      rcases membership with ⟨object, active⟩
      refine .observed (.readU64 object .Relaxed) active _ _ _ ?_
      change Runs (if active != active then .ret (.Err .Cancelled) else finish deadline) tail result
      simpa only [bne_self_eq_false, Bool.false_eq_true, ↓reduceIte] using finished
  cases receipt with
  | clear =>
    refine .observed (.read cancel .Relaxed) false _ _ _ ?_
    exact matchingPrefix _ _ finishClear
  | cancelled =>
    exact .observed (.read cancel .Relaxed) true _ _ _ (.returned _)
  | mismatched object active observed present different =>
    refine .observed (.read cancel .Relaxed) false _ _ _ ?_
    simp only [afterLocal, present, Bool.false_eq_true, ↓reduceIte]
    refine .observed (.readU64 object .Relaxed) observed _ _ _ ?_
    have differs : (observed != active) = true := by simp [different]
    simp only [differs, ↓reduceIte]
    exact .returned _
  | expired object present =>
    refine .observed (.read cancel .Relaxed) false _ _ _ ?_
    apply matchingPrefix
    simp only [finish, present]
    exact .observed (.read object .Relaxed) true _ _ _ (.returned _)

/-- Successful polling records a clear local bit, exactly the matching slot word
when present, and exactly a clear expiry bit when present. This is an equivalence
with an executed computation, not a success premise for subsequent work. -/
theorem completed_iff (cancel : Nat) (slot : Option (Nat × U64)) (deadline : Option Nat)
    (events : List Event) :
    Runs (poll cancel slot deadline) events (.Ok ()) ↔
      events = ⟨.read cancel .Relaxed, false⟩ :: (matchingSlot slot ++ clearDeadline deadline) := by
  constructor
  · intro run
    cases receipt_of_run cancel slot deadline events (.Ok ()) run
    rfl
  · intro same
    subst events
    exact run_of_receipt cancel slot deadline _ _ Receipt.clear

/-- Every finite returned poll consumes its initial local cancellation read,
independently of its result, slot configuration or deadline configuration. -/
theorem nonempty (cancel : Nat) (slot : Option (Nat × U64)) (deadline : Option Nat)
    (events : List Event) (answer : core.result.Result Unit zetesis_cpu.cancellation.Stop)
    (run : Runs (poll cancel slot deadline) events answer) : events ≠ [] := by
  obtain ⟨response, tail, consumed, _⟩ := RuntimeRuns.observed_inv _ _ _ _ run
  rw [consumed]
  exact List.cons_ne_nil _ _

end ControlReads
