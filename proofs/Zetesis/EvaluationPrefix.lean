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

Whole-expression reuse has a separate invariant: every cached result came from
successful evaluation of the same pure expression at an equivalent input key.
Inserting a successful result preserves that invariant. Lookup then preserves
both values and errors, and the same complete-row filter receives the same
ordered results. This does not justify skipping a source row. A separately
covered finite input carrier permits exact inverse selection by a wanted result;
coverage of actual source bindings by that carrier remains a caller obligation.

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

section SuccessfulReuse

universe w x y
variable {Input : Type w} {Key : Type x} {Result : Type y}

/-- Every cache hit names a successful result of the same pure expression.
    The key may omit input details only when those details cannot affect that
    expression's result. Failure results have no cache representation. -/
def SuccessCacheSound (operation : Input → Except Fault Value) (key : Input → Key)
    (cache : Key → Option Value) : Prop :=
  ∀ input value, cache (key input) = some value → operation input = .ok value

/-- Publish one successful result under its exact input key. -/
def rememberSuccess [DecidableEq Key] (cache : Key → Option Value)
    (inputKey : Key) (value : Value) : Key → Option Value :=
  fun queried => if queried = inputKey then some value else cache queried

/-- Consult successful results, evaluating the original expression on a miss. -/
def reuseSuccess (operation : Input → Except Fault Value) (key : Input → Key)
    (cache : Key → Option Value) (input : Input) : Except Fault Value :=
  match cache (key input) with
  | none => operation input
  | some value => .ok value

/-- Inserting a successful evaluation preserves cache soundness. Equal keys
    must identify equal results of the fixed pure expression; equal raw numbers
    from unrelated input owners do not establish this premise.

    A query for the new key agrees with the successful evaluation by the key
    premise. Every other query retains its previous sound entry. -/
theorem remember_success_sound [DecidableEq Key]
    (operation : Input → Except Fault Value) (key : Input → Key)
    (cache : Key → Option Value)
    (identifies : ∀ left right, key left = key right → operation left = operation right)
    (sound : SuccessCacheSound operation key cache)
    (input : Input) (value : Value) (computed : operation input = .ok value) :
    SuccessCacheSound operation key (rememberSuccess cache (key input) value) := by
  intro queried output stored
  by_cases same : key queried = key input
  · have same_value : value = output := by
      simpa [rememberSuccess, same] using stored
    rw [← same_value]
    exact (identifies queried input same).trans computed
  · apply sound queried output
    simpa [rememberSuccess, same] using stored

/-- A sound hit returns exactly the original successful result. A miss calls
    the original evaluator, so errors are preserved as well as values. -/
theorem reuse_success_exact (operation : Input → Except Fault Value)
    (key : Input → Key) (cache : Key → Option Value)
    (sound : SuccessCacheSound operation key cache) (input : Input) :
    reuseSuccess operation key cache input = operation input := by
  unfold reuseSuccess
  cases stored : cache (key input) with
  | none => rfl
  | some value => exact (sound input value stored).symm

/-- Reuse preserves any fixed filter or evidence reduction over the complete
    ordered list of source-row results, including false results and errors.
    The source rows, their order and the consuming function remain unchanged;
    this theorem does not justify pruning rows or changing error precedence.

    Pointwise evaluation equality gives equality of the complete result list,
    and the same consumer therefore returns the same result. -/
theorem reuse_complete_filter (operation : Input → Except Fault Value)
    (key : Input → Key) (cache : Key → Option Value)
    (sound : SuccessCacheSound operation key cache) (rows : List Input)
    (finish : List (Except Fault Value) → Result) :
    finish (rows.map (reuseSuccess operation key cache)) =
      finish (rows.map operation) := by
  have same_evaluation : reuseSuccess operation key cache = operation := by
    funext input
    exact reuse_success_exact operation key cache sound input
  rw [same_evaluation]


/-- The inputs in a supplied finite carrier whose retained successful result
    equals the wanted value. Input order and duplicate occurrences are unchanged. -/
def successPreimage [DecidableEq Value] (key : Input → Key)
    (cache : Key → Option Value) (inputs : List Input) (wanted : Value) : List Input :=
  inputs.filter fun input => decide (cache (key input) = some wanted)

/-- Inverting a sound cache that covers a finite input carrier selects exactly
    the inputs in that carrier evaluating successfully to the wanted value.

    Soundness turns a retained hit into a successful evaluation. Conversely,
    cache coverage supplies a stored result; determinism of the fixed evaluator
    makes it equal to the wanted result. Thus the input survives the filter.

    Cache coverage of the listed inputs is explicit and establishes totality
    only there. Applying this law to source bindings separately requires that
    their inputs belong to the carrier. Arbitrary successful memo entries do not
    establish that source coverage, nor does this law justify skipping diagnostics. -/
theorem success_preimage_exact [DecidableEq Value]
    (operation : Input → Except Fault Value) (key : Input → Key)
    (cache : Key → Option Value) (inputs : List Input)
    (sound : SuccessCacheSound operation key cache)
    (covered : ∀ input ∈ inputs, ∃ value, cache (key input) = some value)
    (wanted : Value) (input : Input) :
    input ∈ successPreimage key cache inputs wanted ↔
      input ∈ inputs ∧ operation input = .ok wanted := by
  simp only [successPreimage, List.mem_filter, decide_eq_true_eq]
  constructor
  · intro retained
    exact ⟨retained.1, sound input wanted retained.2⟩
  · intro computed
    obtain ⟨value, stored⟩ := covered input computed.1
    have same : value = wanted := by
      have successful : operation input = .ok value := sound input value stored
      exact Except.ok.inj (successful.symm.trans computed.2)
    exact ⟨computed.1, same ▸ stored⟩

end SuccessfulReuse

end Zetesis.EvaluationPrefix
