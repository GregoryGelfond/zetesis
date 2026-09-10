/-!
# Finite expression evaluation over a live prefix

A flat evaluation plan reads only its initialized prefix. Unused storage may
contain arbitrary old values. Appending one successful result extends that
prefix by exactly one; an error stops the plan before any later operation.
The central preservation law compares an explicit initialized-length workspace
with evaluation over a list containing only live values. Resetting the length
to zero therefore removes every previous evaluation from the logical input.
The final operation may return its value directly: no later operation needs it
in the live prefix. The root law preserves both that value and the first error.

The finite list of operations is the decreasing measure. Operations are pure
partial functions supplied by the caller: source-plan validity, operand indices,
checked arithmetic, error precedence and logical charging remain separate
obligations. These laws do not verify Rust Vec layout, allocation, Drop/unwind
behavior, source compilation or the complete solver. No physical storage or
old payload is required to remain allocated by the model.
-/

namespace Zetesis.EvaluationPrefix

universe u v
variable {Value : Type u} {Fault : Type v}

/-- Initialize the next cell without exposing the still-unused suffix. -/
def write (storage : List Value) (initialized : Nat) (value : Value) : List Value :=
  storage.take initialized ++ [value] ++ storage.drop (initialized + 1)

/-- A valid prefix grows by exactly the newly computed value. -/
theorem write_prefix (storage : List Value) (initialized : Nat) (value : Value)
    (covered : initialized ≤ storage.length) :
    (write storage initialized value).take (initialized + 1) =
      storage.take initialized ++ [value] := by
  simp [write, List.take_append, List.length_take, Nat.min_eq_left covered, List.take_take]

/-- Reference evaluation retains only initialized results, in plan order. -/
def evaluate : List (List Value → Except Fault Value) → List Value →
    Except Fault (List Value)
  | [], live => .ok live
  | operation :: rest, live =>
    match operation live with
    | .error fault => .error fault
    | .ok value => evaluate rest (live ++ [value])

/-- Workspace evaluation supplies each operation only its initialized prefix. -/
def evaluateStorage : List (List Value → Except Fault Value) → List Value → Nat →
    Except Fault (List Value)
  | [], storage, initialized => .ok (storage.take initialized)
  | operation :: rest, storage, initialized =>
    match operation (storage.take initialized) with
    | .error fault => .error fault
    | .ok value => evaluateStorage rest (write storage initialized value) (initialized + 1)

/-- Reusing arbitrary unused cells preserves the complete value or first error.
The initialized prefix must exist before the first operation. -/
theorem storage_preservation (plan : List (List Value → Except Fault Value))
    (storage : List Value) (initialized : Nat) (covered : initialized ≤ storage.length) :
    evaluateStorage plan storage initialized = evaluate plan (storage.take initialized) := by
  induction plan generalizing storage initialized with
  | nil => rfl
  | cons operation rest induction =>
    simp only [evaluateStorage, evaluate]
    cases result : operation (storage.take initialized) with
    | error fault => rfl
    | ok value =>
      have next_covered : initialized + 1 ≤ (write storage initialized value).length := by
        simp [write, List.length_take, Nat.min_eq_left covered]
      have next_prefix :
          (write storage initialized value).take (initialized + 1) =
            storage.take initialized ++ [value] :=
        write_prefix storage initialized value covered
      have remaining_agrees :=
        induction (write storage initialized value) (initialized + 1) next_covered
      rw [next_prefix] at remaining_agrees
      exact remaining_agrees

/-- An empty live prefix makes every old workspace value irrelevant. -/
theorem reset_preservation (plan : List (List Value → Except Fault Value))
    (storage : List Value) :
    evaluateStorage plan storage 0 = evaluate plan [] := by
  have empty_covered : 0 ≤ storage.length := Nat.zero_le _
  have empty_agrees := storage_preservation plan storage 0 empty_covered
  simpa using empty_agrees

/-- Consecutive plan segments compose through the completed live prefix.
An error in the first segment prevents every operation in the second. -/
theorem evaluate_append (first second : List (List Value → Except Fault Value))
    (live : List Value) :
    evaluate (first ++ second) live =
      (evaluate first live).bind (evaluate second) := by
  induction first generalizing live with
  | nil => rfl
  | cons operation rest induction =>
    simp only [List.cons_append, evaluate]
    cases result : operation live with
    | error fault => rfl
    | ok value => exact induction (live ++ [value])

/-- Evaluate the proper prefix, then return the final operation's own result.
Only intermediate values are retained for later operand lookup. -/
def evaluateRoot (properPrefix : List (List Value → Except Fault Value))
    (root : List Value → Except Fault Value) (live : List Value) : Except Fault Value :=
  (evaluate properPrefix live).bind root

/-- Returning the root directly preserves the reference plan's final value or
first error. The reference appends that value and observes its last entry.

First compose the proper prefix with the singleton root plan. Prefix failure
ends both evaluations. Otherwise both call the same root on the same prefix;
root failure agrees, and success makes the appended root the last value.
This law does not establish Rust allocation or cleanup behavior. -/
theorem root_preservation (properPrefix : List (List Value → Except Fault Value))
    (root : List Value → Except Fault Value) (live : List Value) :
    (evaluate (properPrefix ++ [root]) live).map List.getLast? =
      (evaluateRoot properPrefix root live).map some := by
  rw [evaluate_append]
  unfold evaluateRoot
  cases prefixResult : evaluate properPrefix live with
  | error fault => rfl
  | ok values =>
    simp only [Except.bind, evaluate]
    cases rootResult : root values with
    | error fault => rfl
    | ok value => simp [Except.map]

end Zetesis.EvaluationPrefix
