import Zetesis.FiniteValues

/-!
# Checked scalar operations in finite evaluation plans

A numeric result is admitted only in the signed 32-bit range. Undefined
operations and machine overflow are distinct failures. Division and remainder
share a zero-divisor check and the exceptional minimum divided by negative one
pair; the latter
refuses even when a supplied mathematical remainder would be zero. Negative
exponents are undefined before checking the result's range.

The central substitution law says that replacing every operation by one with
the same value or failure preserves the complete finite plan, including its
first failure. The plan's remaining list is its decreasing measure. Combined
with `FiniteValues.original_literal_identity` and `frozen_literal_identity`,
equal resolved source atoms preserve enclosing original and frozen formulas.

These laws model contracts over mathematical integers and abstract partial
operations. They do not prove Rust i32/checked-operator refinement, bitwise
semantics, the supplied quotient or remainder, source compilation, resource
accounting, allocation behavior, or equality with themelios's implementation.
Those correspondences require separate source review and differential tests.
-/

namespace Zetesis.ScalarArithmetic

/-- Numeric faults remain distinct from a successful value and from each other. -/
inductive Fault where
  | undefined
  | overflow
  deriving DecidableEq

/-- The source evaluator's admitted machine integer range, including endpoints. -/
def InRange (value : Int) : Prop := -2147483648 ≤ value ∧ value ≤ 2147483647

instance (value : Int) : Decidable (InRange value) := by
  unfold InRange
  infer_instance

/-- A mathematical result is returned only when representable in the carrier. -/
def checked (value : Int) : Except Fault Int :=
  if InRange value then .ok value else .error .overflow

/-- Representability is an explicit premise; checked arithmetic does not wrap. -/
theorem checked_success (value : Int) (representable : InRange value) :
    checked value = .ok value := by
  simp [checked, representable]

/-- An out-of-range mathematical result produces overflow, never a wrapped value. -/
theorem checked_overflow (value : Int) (outside : ¬ InRange value) :
    checked value = .error .overflow := by
  simp [checked, outside]

/-- Policy shared by division and remainder. The caller supplies their correct
mathematical result; this definition only specifies fault and range precedence. -/
def divisionResult (left right result : Int) : Except Fault Int :=
  if right = 0 then .error .undefined
  else if left = -2147483648 ∧ right = -1 then .error .overflow
  else checked result

/-- A zero divisor is undefined independently of any proposed result. -/
theorem zero_divisor (left result : Int) :
    divisionResult left 0 result = .error .undefined := by
  simp [divisionResult]

/-- The minimum and negative-one pair overflows even for a representable remainder. -/
theorem exceptional_quotient (result : Int) :
    divisionResult (-2147483648) (-1) result = .error .overflow := by
  simp [divisionResult]

/-- Away from the two fault conditions, a representable supplied result survives. -/
theorem division_success (left right result : Int)
    (nonzero : right ≠ 0)
    (ordinary : ¬ (left = -2147483648 ∧ right = -1))
    (representable : InRange result) :
    divisionResult left right result = .ok result := by
  have accepted_result : checked result = .ok result :=
    checked_success result representable
  simp [divisionResult, nonzero, ordinary, accepted_result]

/-- A negative exponent has no scalar value; nonnegative powers are range checked. -/
def power (base exponent : Int) : Except Fault Int :=
  if exponent < 0 then .error .undefined
  else checked (base ^ exponent.toNat)

/-- Negative exponents are undefined for every base, including zero and one. -/
theorem negative_exponent (base exponent : Int) (negative : exponent < 0) :
    power base exponent = .error .undefined := by
  simp [power, negative]

/-- A nonnegative power succeeds under the explicit mathematical range premise. -/
theorem power_success (base exponent : Int) (nonnegative : ¬ exponent < 0)
    (representable : InRange (base ^ exponent.toNat)) :
    power base exponent = .ok (base ^ exponent.toNat) := by
  have accepted_result : checked (base ^ exponent.toNat) = .ok (base ^ exponent.toNat) :=
    checked_success _ representable
  simp [power, nonnegative, accepted_result]

universe u
variable {Operation : Type u}

/-- A finite plan retains previous results in reverse order. No result from a
failed step is published, and a failure stops all remaining evaluation. -/
def evaluate (step : Operation → List Int → Except Fault Int) :
    List Operation → List Int → Except Fault (List Int)
  | [], previous => .ok previous
  | operation :: rest, previous =>
    match step operation previous with
    | .error fault => .error fault
    | .ok value => evaluate step rest (value :: previous)

/-- Agreement on each operation's value or fault preserves the entire plan.
This is the obligation for replacing temporary term evaluation by direct leaves. -/
theorem operation_agreement
    (left right : Operation → List Int → Except Fault Int)
    (agreement : ∀ operation previous, left operation previous = right operation previous)
    (plan : List Operation) (previous : List Int) :
    evaluate left plan previous = evaluate right plan previous := by
  induction plan generalizing previous with
  | nil => rfl
  | cons operation rest induction =>
    have same_step : left operation previous = right operation previous :=
      agreement operation previous
    simp only [evaluate, same_step]
    cases right operation previous with
    | error fault => rfl
    | ok value => exact induction (value :: previous)

/-- The first failed operation is authoritative regardless of the remaining plan. -/
theorem first_failure (step : Operation → List Int → Except Fault Int)
    (operation : Operation) (rest : List Operation) (previous : List Int) (fault : Fault)
    (failed : step operation previous = .error fault) :
    evaluate step (operation :: rest) previous = .error fault := by
  simp [evaluate, failed]

/-- A successful step extends the previous values before evaluating the tail. -/
theorem successful_step (step : Operation → List Int → Except Fault Int)
    (operation : Operation) (rest : List Operation) (previous : List Int) (value : Int)
    (succeeded : step operation previous = .ok value) :
    evaluate step (operation :: rest) previous = evaluate step rest (value :: previous) := by
  simp [evaluate, succeeded]

end Zetesis.ScalarArithmetic
