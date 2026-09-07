import Zetesis.ChoiceIntervals

/-!
# Transactional positive structural binding

A structural matcher first checks constructor shape and extracts a finite list
of named-slot/value constraints. Complete values have decidable equality. The
transaction below either extends an incoming binding consistently or refuses;
repeated names and prebound names must agree. Anonymous occurrences contribute
no constraint. A failed transaction publishes the unchanged incoming binding.

The source atom attached to a selected support row remains a logical atom.
Selection is a finite grounding operation, not evidence that the atom is true in
an interpretation. Retaining that atom gives the same original and frozen
formula in every surrounding context. Finite support coverage is an explicit
premise of the row selection law.

These laws do not prove the Rust preorder tuple traversal, subtree extraction,
scope/safety compiler, support fixed point, allocation accounting, work counters,
or their correspondence to this abstract transaction. They do not extend the
source profile to nonground named constructors or negative structural binders.
-/

namespace Zetesis.StructuralBindings

universe u v
variable {α : Type u} {β : Type v} [DecidableEq α]

/-- A partial binding; a missing value is not a logical constant. -/
abbrev Binding (α : Type u) := Nat → Option α

/-- Every prebound name keeps its complete value. -/
def Extends (before after : Binding α) : Prop :=
  ∀ slot value, before slot = some value → after slot = some value

/-- All extracted occurrences agree, including repeated uses of one name. -/
def Agrees (binding : Binding α) (constraints : List (Nat × α)) : Prop :=
  ∀ slot value, (slot, value) ∈ constraints → binding slot = some value

/-- One consistent extension, written privately before the row is committed. -/
def bind (before : Binding α) (constraint : Nat × α) : Option (Binding α) :=
  match before constraint.1 with
  | none => some (fun slot => if slot = constraint.1 then some constraint.2 else before slot)
  | some value => if value = constraint.2 then some before else none

/-- Sequential agreement over the complete finite constraint list. -/
def matchConstraints (before : Binding α) : List (Nat × α) → Option (Binding α)
  | [] => some before
  | constraint :: rest => (bind before constraint).bind (fun next => matchConstraints next rest)

/-- A row whose constructor shape failed cannot supply a binding. -/
def matchRow (shape : Bool) (before : Binding α) (constraints : List (Nat × α)) :
    Option (Binding α) :=
  if shape then matchConstraints before constraints else none

/-- Only a complete successful transaction can replace the incoming binding. -/
def commit (before : Binding α) (result : Option (Binding α)) : Binding α :=
  result.getD before

omit [DecidableEq α] in
private theorem extends_refl (binding : Binding α) : Extends binding binding := by
  intro slot value bound
  exact bound

omit [DecidableEq α] in
private theorem extends_trans {first middle last : Binding α}
    (left : Extends first middle) (right : Extends middle last) : Extends first last := by
  intro slot value bound
  exact right slot value (left slot value bound)

private theorem bind_sound (before after : Binding α) (slot : Nat) (value : α)
    (success : bind before (slot, value) = some after) :
    Extends before after ∧ after slot = some value := by
  cases found : before slot with
  | none =>
    have result : (fun other => if other = slot then some value else before other) = after := by
      simpa [bind, found] using success
    subst after
    constructor
    · intro other old bound
      by_cases same : other = slot
      · subst other
        rw [found] at bound
        contradiction
      · simpa [same] using bound
    · simp
  | some old =>
    by_cases same : old = value
    · subst old
      have result : before = after := by simpa [bind, found] using success
      subst after
      exact ⟨extends_refl before, found⟩
    · simp [bind, found, same] at success

/-- Successful matching preserves incoming values and satisfies every occurrence. -/
theorem matching_sound (before after : Binding α) (constraints : List (Nat × α))
    (success : matchConstraints before constraints = some after) :
    Extends before after ∧ Agrees after constraints := by
  induction constraints generalizing before with
  | nil =>
    have result : before = after := by simpa [matchConstraints] using success
    subst after
    exact ⟨extends_refl before, by simp [Agrees]⟩
  | cons constraint rest induction =>
    cases step : bind before constraint with
    | none => simp [matchConstraints, step] at success
    | some middle =>
      have remaining : matchConstraints middle rest = some after := by
        simpa [matchConstraints, step] using success
      have tail : Extends middle after ∧ Agrees after rest := induction middle remaining
      have head : Extends before middle ∧ middle constraint.1 = some constraint.2 :=
        bind_sound before middle constraint.1 constraint.2 step
      constructor
      · exact extends_trans head.1 tail.1
      · intro slot value member
        rcases List.mem_cons.mp member with first | later
        · cases first
          exact tail.1 slot value head.2
        · exact tail.2 slot value later

private theorem bind_fits (before target : Binding α) (slot : Nat) (value : α)
    (extension : Extends before target) (agreement : target slot = some value) :
    ∃ after, bind before (slot, value) = some after ∧ Extends after target := by
  cases found : before slot with
  | none =>
    let after : Binding α := fun other => if other = slot then some value else before other
    refine ⟨after, by simp [bind, found, after], ?_⟩
    intro other old bound
    by_cases same : other = slot
    · subst other
      have equal : value = old := by simpa [after] using bound
      simpa [equal] using agreement
    · apply extension other old
      simpa [after, same] using bound
  | some old =>
    have prior : target slot = some old := extension slot old found
    have equal : old = value := Option.some.inj (prior.symm.trans agreement)
    refine ⟨before, ?_, extension⟩
    simp [bind, found, equal]

/-- Any consistent extension witnesses success. The matcher needs no enumeration
    of complete values; only the finite extracted constraint list is traversed. -/
theorem matching_complete (before target : Binding α) (constraints : List (Nat × α))
    (extension : Extends before target) (agreement : Agrees target constraints) :
    ∃ after, matchConstraints before constraints = some after ∧ Extends after target := by
  induction constraints generalizing before with
  | nil => exact ⟨before, rfl, extension⟩
  | cons constraint rest induction =>
    have head : target constraint.1 = some constraint.2 :=
      agreement constraint.1 constraint.2 (List.mem_cons_self)
    obtain ⟨middle, step, compatible⟩ :=
      bind_fits before target constraint.1 constraint.2 extension head
    have tail : Agrees target rest := by
      intro slot value member
      exact agreement slot value (List.mem_cons_of_mem constraint member)
    obtain ⟨after, remaining, final⟩ := induction middle compatible tail
    refine ⟨after, ?_, final⟩
    simp [matchConstraints, step, remaining]

/-- Repeated named positions cannot silently take different complete values. -/
theorem repeated_name_agreement (before after : Binding α)
    (constraints : List (Nat × α)) (slot : Nat) (left right : α)
    (success : matchConstraints before constraints = some after)
    (first : (slot, left) ∈ constraints) (second : (slot, right) ∈ constraints) :
    left = right := by
  have agreement : Agrees after constraints := (matching_sound before after constraints success).2
  exact Option.some.inj ((agreement slot left first).symm.trans (agreement slot right second))

/-- Failure cannot publish a partial binding, even after earlier occurrences matched. -/
theorem failure_rolls_back (before : Binding α) (shape : Bool)
    (constraints : List (Nat × α)) (failure : matchRow shape before constraints = none) :
    commit before (matchRow shape before constraints) = before := by
  rw [failure]
  rfl

/-- Anonymous occurrences impose no equality or binding requirement. -/
theorem wildcard_only_preserves_binding (before : Binding α) :
    matchConstraints before [] = some before := by
  rfl

/-- A shape mismatch is a refused row, independently of named bindings. -/
theorem incompatible_shape_refuses (before : Binding α) (constraints : List (Nat × α)) :
    matchRow false before constraints = none := by
  rfl

/-- Success is equivalent to the existence of a consistent extension. This
    characterizes refusal without selecting a preferred complete value carrier. -/
theorem matching_iff_consistent (before : Binding α) (constraints : List (Nat × α)) :
    (matchConstraints before constraints).isSome = true ↔
      ∃ after, Extends before after ∧ Agrees after constraints := by
  constructor
  · intro success
    cases result : matchConstraints before constraints with
    | none => simp [result] at success
    | some after => exact ⟨after, matching_sound before after constraints result⟩
  · rintro ⟨target, extension, agreement⟩
    obtain ⟨after, success, _⟩ := matching_complete before target constraints extension agreement
    simp [success]

/-- Select complete source rows; neither the row atom nor its truth value is
    replaced by the projected binding. -/
def selectRows (rows : List β) (shape : β → Bool) (constraints : β → List (Nat × α))
    (before : Binding α) : List β :=
  rows.filter (fun row => (matchRow (shape row) before (constraints row)).isSome)

/-- A complete finite support carrier supplies exactly the source rows with
    compatible constructor shape and a consistent binding extension. The support
    coverage premise must be established separately by the source grounder. -/
theorem selected_row_coverage (rows : List β) (sourceRow : β → Prop)
    (shape : β → Bool) (constraints : β → List (Nat × α)) (before : Binding α)
    (coverage : ∀ row, row ∈ rows ↔ sourceRow row) (row : β) :
    row ∈ selectRows rows shape constraints before ↔
      sourceRow row ∧ shape row = true ∧
        ∃ after, Extends before after ∧ Agrees after (constraints row) := by
  rw [selectRows, List.mem_filter, coverage row]
  cases found : shape row with
  | false => simp [matchRow]
  | true => simp [matchRow, matching_iff_consistent]

open Ferraris
open ChoiceIntervals (Equivalent equivalent_refl substitute substitute_equivalent)

omit [DecidableEq α] in
/-- Captured source atoms are retained as logical leaves. A capture map equal to
    the original atom map preserves every context before and after freezing. -/
theorem retained_atom_context (context : Formula β) (original captured : β → α)
    (identity : ∀ row, captured row = original row) :
    Equivalent (substitute context (fun row => .atom (captured row)))
      (substitute context (fun row => .atom (original row))) := by
  apply substitute_equivalent
  intro row
  rw [identity row]
  exact equivalent_refl _

omit [DecidableEq α] in
/-- Original atom retention preserves stability with the unchanged remaining
    theory. No positive support row has been replaced by propositional truth. -/
theorem retained_atom_stability (M : Atoms α) (context : Formula β)
    (original captured : β → α) (identity : ∀ row, captured row = original row)
    (theory : Theory α) :
    Stable M (substitute context (fun row => .atom (captured row)) :: theory) ↔
      Stable M (substitute context (fun row => .atom (original row)) :: theory) := by
  have same := retained_atom_context context original captured identity
  simp only [Stable, models_cons, ReductTheory, List.map_cons, same.1 M, same.2 M]

end Zetesis.StructuralBindings
