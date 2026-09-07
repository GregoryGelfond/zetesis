import Zetesis.StructuralBindings

/-!
# Constructor shape and whole-subtree extraction

A finite value tree retains each function's name, sign and ordered children.
Paths select whole subtrees. A finite pattern plan checks constructor shapes and
extracts named occurrences at supplied paths; anonymous occurrences contribute
no named capture. Shape tests are explicit, rather than an assumed shape Boolean.
The extracted constraints feed the existing transactional binding algorithm.

Path traversal decreases the path length; capture traversal decreases the finite
capture list. Shape inspection compares names and arities, and field lookup costs
the traversed list prefix. These are mathematical trees and unbounded naturals.
The laws do not prove Rust's flat preorder encoding, plan compilation, constant
leaf checks, path/offset correspondence, source safety, support completeness,
allocation accounting or machine bounds. In particular, callers must establish
that the supplied tests and capture paths describe the complete source pattern.
-/

namespace Zetesis.ConstructorPatterns

universe u

/-- Complete semantic data. Tuple identity is separate from every named function. -/
inductive Value (α : Type u) where
  | scalar : α → Value α
  | tuple : List (Value α) → Value α
  | function : String → Bool → List (Value α) → Value α

/-- A constructor's identity includes arity; scalar leaves have no such shape. -/
inductive Shape where
  | tuple : Nat → Shape
  | function : String → Bool → Nat → Shape
  deriving DecidableEq

variable {α : Type u}

def shape : Value α → Option Shape
  | .scalar _ => none
  | .tuple fields => some (.tuple fields.length)
  | .function name negative fields => some (.function name negative fields.length)

def fields : Value α → List (Value α)
  | .scalar _ => []
  | .tuple children | .function _ _ children => children

/-- A path denotes a complete subtree, never its spelling or first component. -/
def atPath (value : Value α) : List Nat → Option (Value α)
  | [] => some value
  | index :: rest => ((fields value)[index]?).bind (fun child => atPath child rest)

/-- Constructor tests and named captures are separately inspectable obligations. -/
structure Plan where
  tests : List (List Nat × Shape)
  captures : List (Nat × List Nat)

def test (value : Value α) (requirement : List Nat × Shape) : Bool :=
  decide ((atPath value requirement.1).bind shape = some requirement.2)

/-- Each capture must resolve; failure yields no shortened successful list. -/
def extract (value : Value α) : List (Nat × List Nat) → Option (List (Nat × Value α))
  | [] => some []
  | (slot, path) :: rest => do
      let selected ← atPath value path
      let remaining ← extract value rest
      pure ((slot, selected) :: remaining)

def constraints (value : Value α) (plan : Plan) : Option (List (Nat × Value α)) :=
  if plan.tests.all (test value) then extract value plan.captures else none

/-- Function name, sign and arity are jointly necessary and sufficient at a root. -/
theorem function_shape_exact (name expected : String) (negative expectedSign : Bool)
    (children : List (Value α)) (arity : Nat) :
    shape (.function name negative children) = some (.function expected expectedSign arity) ↔
      name = expected ∧ negative = expectedSign ∧ children.length = arity := by
  simp only [shape, Option.some.injEq, Shape.function.injEq]

/-- A tuple cannot satisfy a function shape, even when the arities agree. -/
theorem tuple_refuses_function (children : List (Value α))
    (name : String) (negative : Bool) (arity : Nat) :
    shape (.tuple children) ≠ some (.function name negative arity) := by
  intro equality
  cases equality

/-- An empty path retains the complete value, including nested constructors. -/
theorem root_capture (value : Value α) : atPath value [] = some value := by
  rfl

/-- Descending through a function preserves the selected child's complete identity. -/
theorem function_child (name : String) (negative : Bool) (children : List (Value α))
    (index : Nat) (child : Value α) (path : List Nat)
    (selected : children[index]? = some child) :
    atPath (.function name negative children) (index :: path) = atPath child path := by
  simp only [atPath, fields, selected, Option.bind_some]

/-- Every returned occurrence comes from the specified source path. -/
theorem extraction_sound (value : Value α) (captures : List (Nat × List Nat))
    (result : List (Nat × Value α)) (success : extract value captures = some result)
    (slot : Nat) (selected : Value α) (member : (slot, selected) ∈ result) :
    ∃ path, (slot, path) ∈ captures ∧ atPath value path = some selected := by
  induction captures generalizing result with
  | nil => simp [extract] at success; subst result; simp at member
  | cons capture rest induction =>
    obtain ⟨firstSlot, firstPath⟩ := capture
    cases first : atPath value firstPath with
    | none => simp [extract, first] at success
    | some firstValue =>
      cases remaining : extract value rest with
      | none => simp [extract, first, remaining] at success
      | some tail =>
        have output : (firstSlot, firstValue) :: tail = result := by
          simpa [extract, first, remaining] using success
        subst result
        rcases List.mem_cons.mp member with same | later
        · have identity : slot = firstSlot ∧ selected = firstValue := Prod.mk.inj same
          obtain ⟨rfl, rfl⟩ := identity
          exact ⟨firstPath, List.mem_cons_self, first⟩
        · obtain ⟨path, inTail, captured⟩ := induction tail remaining later
          exact ⟨path, List.mem_cons_of_mem _ inTail, captured⟩

/-- Resolvable capture paths suffice to obtain the entire finite constraint list. -/
theorem extraction_complete (value : Value α) (captures : List (Nat × List Nat))
    (coverage : ∀ capture ∈ captures, ∃ selected, atPath value capture.2 = some selected) :
    ∃ result, extract value captures = some result := by
  induction captures with
  | nil => exact ⟨[], rfl⟩
  | cons capture rest induction =>
    obtain ⟨slot, path⟩ := capture
    obtain ⟨selected, captured⟩ := coverage (slot, path) List.mem_cons_self
    have remaining : ∀ capture ∈ rest, ∃ selected, atPath value capture.2 = some selected := by
      intro capture member
      exact coverage capture (List.mem_cons_of_mem _ member)
    obtain ⟨tail, extracted⟩ := induction remaining
    exact ⟨(slot, selected) :: tail, by simp [extract, captured, extracted]⟩

/-- Every supplied shape obligation holds when the full plan returns constraints. -/
theorem successful_shapes (value : Value α) (plan : Plan)
    (result : List (Nat × Value α)) (success : constraints value plan = some result)
    (path : List Nat) (expected : Shape) (member : (path, expected) ∈ plan.tests) :
    (atPath value path).bind shape = some expected := by
  have allTests : plan.tests.all (test value) = true := by
    by_cases accepted : plan.tests.all (test value) = true
    · exact accepted
    · simp [constraints, accepted] at success
  have selectedTest : test value (path, expected) = true :=
    List.all_eq_true.mp allTests (path, expected) member
  exact of_decide_eq_true selectedTest

-- Complete-value equality is the same explicit premise used by the transaction
-- library. No executable equality or flat-value representation is refined here.
variable [DecidableEq (Value α)]

/-- The binding stage consumes only constraints from a complete shape/extraction pass. -/
def matchPattern (value : Value α) (plan : Plan)
    (before : StructuralBindings.Binding (Value α)) :=
  (constraints value plan).bind (StructuralBindings.matchConstraints before)

/-- Pattern success preserves old bindings and satisfies every extracted occurrence. -/
theorem binding_sound (value : Value α) (plan : Plan)
    (before after : StructuralBindings.Binding (Value α))
    (success : matchPattern value plan before = some after) :
    ∃ result, constraints value plan = some result ∧
      StructuralBindings.Extends before after ∧ StructuralBindings.Agrees after result := by
  cases extracted : constraints value plan with
  | none => simp [matchPattern, extracted] at success
  | some result =>
    have bound : StructuralBindings.matchConstraints before result = some after := by
      simpa [matchPattern, extracted] using success
    exact ⟨result, rfl, StructuralBindings.matching_sound before after result bound⟩

/-- Acceptance means complete extraction plus a consistent binding extension. -/
theorem acceptance_iff_consistent (value : Value α) (plan : Plan)
    (before : StructuralBindings.Binding (Value α)) :
    (matchPattern value plan before).isSome = true ↔
      ∃ result, constraints value plan = some result ∧
        ∃ after, StructuralBindings.Extends before after ∧ StructuralBindings.Agrees after result := by
  cases extracted : constraints value plan with
  | none => simp [matchPattern, extracted]
  | some result => simp [matchPattern, extracted, StructuralBindings.matching_iff_consistent]

/-- A missing subtree or incompatible shape cannot publish staged bindings. -/
theorem failure_preserves_binding (value : Value α) (plan : Plan)
    (before : StructuralBindings.Binding (Value α))
    (failed : matchPattern value plan before = none) :
    StructuralBindings.commit before (matchPattern value plan before) = before := by
  rw [failed]
  rfl

end Zetesis.ConstructorPatterns
