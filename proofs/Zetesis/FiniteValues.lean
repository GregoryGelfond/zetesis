import Zetesis.Ferraris

/-!
# Declared inputs and deterministic data slots

A finite sequence reads source slots and applies already fixed partial operations
(including constructor operations) to previous results. The input set describes
all source reads. Operations may fail; failure returns no partial environment.
Evaluation is independent of the candidate/frozen interpretation. Extending a
fresh data slot preserves existing bindings and, when the resolved source atom
is unchanged, preserves every enclosing original and frozen formula.

The list tail decreases at each evaluation step. This model does not prove Rust
refinement, constructor byte/depth preflight, machine arithmetic, allocator
behavior, input safety, support completeness or choice eligibility coalescence.
The supplied partial operation models their result/failure contract abstractly.
-/

namespace Zetesis.FiniteValues
open Ferraris
universe u v
variable {V : Type u} {A : Type v}

/-- Source slots and previous operation results are distinct namespaces. -/
inductive Step (V : Type u) where
  | input : Nat → Step V
  | constant : V → Step V
  | apply : (List V → Option V) → Step V

/-- Every source slot read by the complete plan, including repeated occurrences. -/
def inputs : List (Step V) → List Nat
  | [] => []
  | .input slot :: rest => slot :: inputs rest
  | _ :: rest => inputs rest

def step (environment : Nat → V) (previous : List V) : Step V → Option V
  | .input slot => some (environment slot)
  | .constant value => some value
  | .apply operation => operation previous

/-- New results precede earlier results; a failed step publishes no partial row. -/
def evaluate (environment : Nat → V) : List (Step V) → List V → Option (List V)
  | [], previous => some previous
  | operation :: rest, previous => do
      let value ← step environment previous operation
      evaluate environment rest (value :: previous)

/-- Agreement on declared inputs suffices for equal values and equal failures. -/
theorem input_agreement (plan : List (Step V)) (left right : Nat → V)
    (agreement : ∀ slot ∈ inputs plan, left slot = right slot) (previous : List V) :
    evaluate left plan previous = evaluate right plan previous := by
  induction plan generalizing previous with
  | nil => rfl
  | cons operation rest induction =>
    have tail_agreement : ∀ slot ∈ inputs rest, left slot = right slot := by
      intro slot member
      apply agreement slot
      cases operation <;> simp_all [inputs]
    have same_step : step left previous operation = step right previous operation := by
      cases operation with
      | input slot =>
        have same_input := agreement slot (by simp [inputs])
        simp [step, same_input]
      | constant value => rfl
      | apply operation => rfl
    simp only [evaluate, same_step]
    cases step right previous operation with
    | none => rfl
    | some value => exact induction tail_agreement (value :: previous)

/-- One deterministic successful result extends only the supplied data slot. -/
def extend (environment : Nat → V) (target : Nat) (value : V) : Nat → V :=
  fun slot => if slot = target then value else environment slot

theorem extension_target (environment : Nat → V) (target : Nat) (value : V) :
    extend environment target value target = value := by
  simp [extend]

theorem extension_preserves (environment : Nat → V) (target slot : Nat) (value : V)
    (distinct : slot ≠ target) :
    extend environment target value slot = environment slot := by
  simp [extend, distinct]

/-- Constructor identity retains its tag and ordered complete children. -/
inductive Constructed (V : Type u) where
  | named : String → Bool → List V → Constructed V
  | tuple : List V → Constructed V

/-- Neither the spelling nor a set of children can replace constructor identity. -/
theorem constructor_identity (name other : String) (negative otherSign : Bool)
    (children otherChildren : List V) :
    Constructed.named name negative children = .named other otherSign otherChildren ↔
      name = other ∧ negative = otherSign ∧ children = otherChildren := by
  simp only [Constructed.named.injEq]

/-- Tuple identity cannot alias a named constructor, even with equal children. -/
theorem tuple_identity (name : String) (negative : Bool) (children : List V) :
    Constructed.tuple children ≠ .named name negative children := by
  intro equality
  cases equality

/-- Polarity belongs to the source literal after its data argument is resolved. -/
def literal (depth : Nat) (atom : A) : Formula A :=
  Nat.rec (.atom atom) (fun _ formula => Neg formula) depth

/-- Exact ground-atom identity transports the complete enclosing source formula. -/
theorem original_literal_identity (M : Atoms A) (source lowered : A)
    (identity : source = lowered) (depth : Nat) (context : Formula A → Formula A) :
    Satisfies M (context (literal depth source)) ↔
      Satisfies M (context (literal depth lowered)) := by
  subst lowered
  rfl

/-- The same identity holds for arbitrary M/J, without a subset premise. -/
theorem frozen_literal_identity (M J : Atoms A) (source lowered : A)
    (identity : source = lowered) (depth : Nat) (context : Formula A → Formula A) :
    Satisfies J (Reduct M (context (literal depth source))) ↔
      Satisfies J (Reduct M (context (literal depth lowered))) := by
  subst lowered
  rfl

/-- A failed operation cannot return a shortened successful computation. -/
theorem failed_step (environment : Nat → V) (operation : Step V)
    (rest : List (Step V)) (previous : List V)
    (failed : step environment previous operation = none) :
    evaluate environment (operation :: rest) previous = none := by
  simp [evaluate, failed]

end Zetesis.FiniteValues
