import Zetesis.BindingScopes

/-!
# Canonical template substitution

A template argument is a variable position or a constant identity. Substitution
reads a partial binding; a missing variable prevents publication of the tuple.
Decoding identities after substitution gives exactly the same ordered tuple as
substituting decoded values. Repeated arguments keep their occurrence positions.

The identity type ranges over the available vocabulary. Its decoder is total
on that type. Rust must establish this domain by checking the vocabulary scope
and readable prefix; this module does not justify unchecked integer indexing.
No injectivity or identity-allocation order is needed for substitution. Rules
and objectives may share this mechanism without sharing their semantic policy.

These laws do not establish interning uniqueness, Rust lifetime or resource
accounting, source compilation, support completeness or answer-set membership.
-/

namespace Zetesis.CanonicalTemplates

universe u v
variable {Id : Type u} {Value : Type v}

/-- One ordered argument occurrence; a variable is not an interned constant. -/
inductive Argument (α : Type u) where
  | variable : Nat → Argument α
  | constant : α → Argument α

/-- Decode only constants. Variable positions retain their source meaning. -/
def decodeArgument (decode : Id → Value) : Argument Id → Argument Value
  | .variable slot => .variable slot
  | .constant identity => .constant (decode identity)

/-- Resolve one argument without replacing absence by a logical value. -/
def read (binding : StructuralBindings.Binding Id) : Argument Id → Option Id
  | .variable slot => binding slot
  | .constant identity => some identity

/-- Substitute an ordered tuple, publishing only when every argument resolves. -/
def substitute (binding : StructuralBindings.Binding Id) : List (Argument Id) → Option (List Id)
  | [] => some []
  | argument :: arguments => do
    let value ← read binding argument
    let values ← substitute binding arguments
    pure (value :: values)

/-- Decoding a resolved argument agrees with resolving its decoded form.
Constants use their decoder; variables use the same optional binding slot. -/
theorem read_decodes (decode : Id → Value) (binding : StructuralBindings.Binding Id)
    (argument : Argument Id) :
    read (fun slot => (binding slot).map decode) (decodeArgument decode argument) =
      (read binding argument).map decode := by
  cases argument <;> rfl

/-- Decoding commutes with substitution of the entire ordered tuple.
Induct over occurrences. The first argument agrees by `read_decodes`; the tail
agrees by induction. Either missing read prevents both forms from publishing. -/
theorem substitute_decodes (decode : Id → Value) (binding : StructuralBindings.Binding Id)
    (arguments : List (Argument Id)) :
    substitute (fun slot => (binding slot).map decode)
        (arguments.map (decodeArgument decode)) =
      (substitute binding arguments).map (List.map decode) := by
  induction arguments with
  | nil => rfl
  | cons argument arguments tail =>
    simp only [List.map_cons, substitute, read_decodes, tail]
    cases read binding argument <;> cases substitute binding arguments <;> rfl

/-- Bindings agreeing at every referenced argument produce the same tuple.
Constants are unchanged; variables outside the tuple's inputs impose no
agreement requirement. This is a binding law, not a decoder-extension law. -/
theorem substitute_agrees
    (before after : StructuralBindings.Binding Id)
    (arguments : List (Argument Id))
    (agreement : ∀ argument ∈ arguments, read before argument = read after argument) :
    substitute before arguments = substitute after arguments := by
  induction arguments with
  | nil => rfl
  | cons argument arguments tail =>
    have first : read before argument = read after argument :=
      agreement argument (by simp)
    have rest : substitute before arguments = substitute after arguments := by
      apply tail
      intro other present
      exact agreement other (by simp [present])
    simp only [substitute, first, rest]

/-- A tuple of variable occurrences is the existing checked scope read.
This connects canonical substitution to the partial-binding library rather than
introducing another meaning for an absent source variable. -/
theorem variables_are_scope_reads (binding : StructuralBindings.Binding Id) (slots : List Nat) :
    substitute binding (slots.map Argument.variable) = BindingScopes.readAll binding slots := by
  induction slots with
  | nil => rfl
  | cons slot slots tail =>
    simp only [List.map_cons, substitute, read, BindingScopes.readAll, tail]

end Zetesis.CanonicalTemplates
