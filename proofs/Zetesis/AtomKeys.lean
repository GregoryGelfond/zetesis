import Zetesis.BindingScopes

/-!
# Checked atom identities

A key resolves a finite argument list in a partial binding. Constants supply
values directly; variables require present slots. A successful key retains the
signed predicate and exact argument order. Lookup compares that identity with
stored atoms, without needing a second owner.

These laws concern substitution and extensional membership. They do not verify
Rust lifetimes, allocation, hash encoding, comparison cost or the implementation
of ordered and hashed indexes. Key construction is not a support certificate.
-/

namespace Zetesis.AtomKeys

universe u v
variable {α : Type u} {σ : Type v}

/-- One argument is a complete logical value or a named variable. -/
inductive Argument (α : Type u) where
  | constant : α → Argument α
  | variable : Nat → Argument α

/-- Resolve one argument, preserving absence for a missing variable. -/
def resolve (binding : StructuralBindings.Binding α) : Argument α → Option α
  | .constant value => some value
  | .variable slot => binding slot

/-- Preserve argument order; no tuple is published when an argument is absent. -/
def tuple (binding : StructuralBindings.Binding α) : List (Argument α) → Option (List α)
  | [] => some []
  | argument :: arguments => do
    let value ← resolve binding argument
    let values ← tuple binding arguments
    pure (value :: values)

/-- A successful substitution has the same predicate and complete ordered tuple. -/
def key (predicate : σ) (binding : StructuralBindings.Binding α)
    (arguments : List (Argument α)) : Option (σ × List α) :=
  (tuple binding arguments).map (fun values => (predicate, values))

/-- An absent referenced variable prevents publication at that argument. -/
theorem missing_blocks_tuple (binding : StructuralBindings.Binding α) (slot : Nat)
    (arguments : List (Argument α)) (absent : binding slot = none) :
    tuple binding (.variable slot :: arguments) = none := by
  simp [tuple, resolve, absent]

/-- Binding views agreeing on each requested argument resolve the same tuple.

Induct on the argument list. The first read is equal by the hypothesis;
the induction hypothesis preserves the remaining ordered reads.
-/
theorem tuple_agrees (before after : StructuralBindings.Binding α)
    (arguments : List (Argument α))
    (agree : ∀ argument ∈ arguments, resolve before argument = resolve after argument) :
    tuple before arguments = tuple after arguments := by
  induction arguments with
  | nil => rfl
  | cons argument arguments tail =>
    have first := agree argument (by simp)
    have rest : tuple before arguments = tuple after arguments := by
      apply tail
      intro other present
      exact agree other (by simp [present])
    simp only [tuple, first, rest]

/-- Materializing a checked tuple preserves its full predicate identity. -/
theorem key_identity (predicate : σ) (binding : StructuralBindings.Binding α)
    (arguments : List (Argument α)) (values : List α)
    (complete : tuple binding arguments = some values) :
    key predicate binding arguments = some (predicate, values) := by
  simp [key, complete]

/-- Membership of a materialized key is exactly membership of its checked identity.

Replace the successful substitution by the established identity. No property
of a hash value, source spelling or candidate interpretation is used.
-/
theorem membership_identity (predicate : σ) (binding : StructuralBindings.Binding α)
    (arguments : List (Argument α)) (values : List α) (stored : σ × List α → Prop)
    (complete : tuple binding arguments = some values) :
    (∃ atom, key predicate binding arguments = some atom ∧ stored atom) ↔
      stored (predicate, values) := by
  rw [key_identity predicate binding arguments values complete]
  simp

end Zetesis.AtomKeys
