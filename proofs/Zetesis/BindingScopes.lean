import Zetesis.StructuralBindings

/-!
# Scoped partial assignments

An unavailable variable is absent, not an arbitrary logical constant. Restricting
an assignment to a body or component scope preserves each available value in
that scope and makes every other variable unavailable. A consumer publishes a
list of values only after every requested variable has an assigned value.

These laws use the partial-binding vocabulary of `StructuralBindings`. They do
not verify source-scope compilation, Rust allocation, generator scheduling,
arithmetic failure order or correspondence between Rust frames and these maps.
-/

namespace Zetesis.BindingScopes

open StructuralBindings (Binding)

universe u
variable {α : Type u}

/-- A scope selects source variable identities without renumbering them. -/
def restrict (binding : Binding α) (scope : List Nat) : Binding α :=
  fun slot => if slot ∈ scope then binding slot else none

/-- Read precisely the requested variables; one absent input prevents publication. -/
def readAll (binding : Binding α) : List Nat → Option (List α)
  | [] => some []
  | slot :: slots => do
    let value ← binding slot
    let values ← readAll binding slots
    pure (value :: values)

/-- Restriction preserves the original value of every variable in the scope. -/
theorem restrict_inside (binding : Binding α) (scope : List Nat) (slot : Nat)
    (inside : slot ∈ scope) : restrict binding scope slot = binding slot := by
  simp [restrict, inside]

/-- A variable outside the scope is unavailable, even if its parent has a value. -/
theorem restrict_outside (binding : Binding α) (scope : List Nat) (slot : Nat)
    (outside : slot ∉ scope) : restrict binding scope slot = none := by
  simp [restrict, outside]

/-- A missing input cannot be interpreted as any chosen value. -/
theorem absent_is_not_value (binding : Binding α) (slot : Nat) (value : α)
    (absent : binding slot = none) : binding slot ≠ some value := by
  simp [absent]

/-- If the first requested variable is absent, the consumer publishes no row. -/
theorem absent_blocks_read (binding : Binding α) (slot : Nat) (slots : List Nat)
    (absent : binding slot = none) : readAll binding (slot :: slots) = none := by
  simp [readAll, absent]

/-- Equal values on the requested scope suffice for equal consumer results.

The argument follows the requested variable list. Equality at its first variable
preserves the first checked read; the induction hypothesis preserves the rest.
No assumption is made about variables outside that list.
-/
theorem readAll_agrees (before after : Binding α) (slots : List Nat)
    (agree : ∀ slot ∈ slots, before slot = after slot) :
    readAll before slots = readAll after slots := by
  induction slots with
  | nil => rfl
  | cons slot slots tail =>
    have first : before slot = after slot := agree slot (by simp)
    have rest : readAll before slots = readAll after slots := by
      apply tail
      intro other present
      exact agree other (by simp [present])
    simp only [readAll, first, rest]

/-- A consumer whose inputs are in scope obtains the same result after restriction. -/
theorem readAll_restrict (binding : Binding α) (scope slots : List Nat)
    (covered : ∀ slot ∈ slots, slot ∈ scope) :
    readAll (restrict binding scope) slots = readAll binding slots := by
  apply readAll_agrees
  intro slot requested
  exact restrict_inside binding scope slot (covered slot requested)

end Zetesis.BindingScopes
