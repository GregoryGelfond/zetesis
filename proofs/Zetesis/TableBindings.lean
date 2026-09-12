import Zetesis.FiniteTables
import Zetesis.StructuralBindings

/-!
# Table selection preserves positive binding families

A flat source argument is a constant or a source slot. Table variable labels
have a separate namespace: their origin identifies the source argument. Repeated
labels therefore impose the same argument, without equating table labels with
source slots. A successful structural binding match satisfies the resulting
aliases and constant/already-bound-value domains.

An exact table preselection followed by the unchanged positive matcher returns
the same ordered list of row occurrences and bindings as matching the original
rows. Finite join composition retains source-occurrence tags and whole witness
traces, including repeated predicates and equal-valued rows at distinct positions.

Here `none` means a positive relational mismatch, not cancellation, allocation
failure or an undefined authored expression. The completed-support certificate,
canonical scope construction, typed relation decoding, packed masks, Rust
lifetimes, accounting and authored-body validation remain implementation obligations.
Possible support is not logical truth; source atoms remain in emitted formulas.
-/

namespace Zetesis.TableBindings

open StructuralBindings

variable {Row Variable Value : Type}

/-- Canonical table labels refer to source arguments, not to table-local values. -/
inductive Argument (Value : Type) where
  | constant : Value → Argument Value
  | slot : Nat → Argument Value

/-- Only constants and existing bindings restrict the positive-row prefilter. -/
def Allowed (before : Binding Value) : Argument Value → Value → Prop
  | .constant expected, value => value = expected
  | .slot slot, value => before slot = none ∨ before slot = some value

/-- The existing binding matcher receives every source-slot occurrence in order.
Constant checks are separate; they do not manufacture binding slots. -/
def constraints {arity : Nat} (arguments : Fin arity → Argument Value)
    (values : Fin arity → Value) : List (Nat × Value) :=
  (List.finRange arity).flatMap fun column =>
    match arguments column with
    | .constant _ => []
    | .slot slot => [(slot, values column)]

/-- A successful flat match survives table aliases and necessary domains.

`matching_sound` supplies agreement on all source-slot columns and preservation of
incoming bindings. Constant checks supply the remaining columns. Two aliased
columns read the same source argument, so their whole values agree; each column
also satisfies its constant or incoming-slot restriction. Empty arity is covered
by the same statement, retaining the explicit original-row premise. -/
theorem flat_match_survives [DecidableEq Value] {arity : Nat}
    (table : Row → Prop) (values : Row → Fin arity → Value)
    (scope : Fin arity → Variable) (origin : Variable → Argument Value)
    (row : Row) (before after : Binding Value) (present : table row)
    (constants : ∀ column value, origin (scope column) = .constant value →
      values row column = value)
    (success : matchConstraints before
      (constraints (fun column => origin (scope column)) (values row)) = some after) :
    FiniteTables.Survives table values scope
      (fun variableId => Allowed before (origin variableId)) row := by
  have sound := matching_sound before after _ success
  have slots : ∀ column slot, origin (scope column) = .slot slot →
      after slot = some (values row column) := by
    intro column slot named
    apply sound.2 slot (values row column)
    simp only [constraints, List.mem_flatMap]
    exact ⟨column, List.mem_finRange column, by simp [named]⟩
  have coherent : FiniteTables.Coherent values scope row := by
    intro left right same
    cases argument : origin (scope left) with
    | constant value =>
      have atLeft := constants left value argument
      have atRight := constants right value (same ▸ argument)
      exact atLeft.trans atRight.symm
    | slot slot =>
      have atLeft := slots left slot argument
      have atRight := slots right slot (same ▸ argument)
      exact Option.some.inj (atLeft.symm.trans atRight)
  have domains : ∀ column, Allowed before (origin (scope column)) (values row column) := by
    intro column
    cases argument : origin (scope column) with
    | constant value => exact constants column value argument
    | slot slot =>
      cases incoming : before slot with
      | none => exact Or.inl incoming
      | some value =>
        have retained := sound.1 slot value incoming
        have matched := slots column slot argument
        have same : value = values row column := Option.some.inj (retained.symm.trans matched)
        exact Or.inr (same ▸ incoming)
  exact ⟨present, coherent, domains⟩

/-- Preserve each successful row occurrence beside its resulting binding.
Equal values at different positions remain different witnesses in this list. -/
def matchedRows {Binding : Type} (rows : List Row) (matcher : Row → Option Binding) :
    List (Row × Binding) :=
  rows.filterMap fun row => (matcher row).map fun binding => (row, binding)

/-- Exact indexed preselection preserves the ordered positive-match family.

For a successful match, the necessity premise and the existing finite-table law
force its selection bit to be true. A failed positive match contributes nothing
with either bit. List induction retains order, multiplicity and the same resulting
binding, rather than merely equating sets of values. The matcher excludes errors
and later authored-body validation; those are not relational mismatches. -/
theorem indexed_matches_preserved {Column Binding : Type}
    (rows : List Row) (table : Row → Prop) (values : Row → Column → Value)
    (scope : Column → Variable) (domains : Variable → Value → Prop)
    (selected : Row → Bool) (matcher : Row → Option Binding)
    (exactSelection : ∀ row ∈ rows, selected row = true ↔
      FiniteTables.IndexedSurvival table values scope domains row)
    (necessary : ∀ row ∈ rows, ∀ binding, matcher row = some binding →
      FiniteTables.Survives table values scope domains row) :
    matchedRows (rows.filter selected) matcher = matchedRows rows matcher := by
  induction rows with
  | nil => rfl
  | cons row rest induction =>
    have tailExact : ∀ row ∈ rest, selected row = true ↔
        FiniteTables.IndexedSurvival table values scope domains row := by
      intro row member
      exact exactSelection row (List.mem_cons_of_mem _ member)
    have tailNecessary : ∀ row ∈ rest, ∀ binding, matcher row = some binding →
        FiniteTables.Survives table values scope domains row := by
      intro row member
      exact necessary row (List.mem_cons_of_mem _ member)
    have tailSame := induction tailExact tailNecessary
    cases result : matcher row with
    | none =>
      cases bit : selected row <;> simp [matchedRows, result, bit] at tailSame ⊢
      all_goals exact tailSame
    | some binding =>
      have survives := necessary row (by simp) binding result
      have indexed := (FiniteTables.indexed_survival_exact table values scope domains row).2 survives
      have bit : selected row = true := (exactSelection row (by simp)).2 indexed
      simpa [matchedRows, result, bit] using congrArg (List.cons (row, binding)) tailSame

/-- A finite positive join records authored source-occurrence identities beside
the selected row positions, in the supplied execution schedule. -/
def joinFamily {Occurrence Binding : Type}
    (next : Occurrence → Binding → List (Row × Binding)) :
    List Occurrence → Binding → List (List (Occurrence × Row) × Binding)
  | [], before => [([], before)]
  | occurrence :: rest, before =>
    (next occurrence before).flatMap fun (row, after) =>
      (joinFamily next rest after).map fun (trace, result) =>
        ((occurrence, row) :: trace, result)

/-- Replacing each positive row source by an equal ordered match family preserves
the complete join family, including its source/row witness traces.

The empty schedule retains the incoming binding once. For a nonempty schedule,
the step premise supplies the same row and intermediate binding occurrences;
the induction hypothesis supplies the same continuation for each occurrence.
This is about successful finite enumeration, not prefixes stopped by limits. -/
theorem join_family_preserved {Occurrence Binding : Type}
    (left right : Occurrence → Binding → List (Row × Binding))
    (schedule : List Occurrence)
    (steps : ∀ occurrence ∈ schedule, ∀ before, left occurrence before = right occurrence before)
    (before : Binding) :
    joinFamily left schedule before = joinFamily right schedule before := by
  induction schedule generalizing before with
  | nil => rfl
  | cons occurrence rest induction =>
    have headSame := steps occurrence (by simp) before
    have tailSteps : ∀ occurrence ∈ rest, ∀ before,
        left occurrence before = right occurrence before := by
      intro occurrence member
      exact steps occurrence (List.mem_cons_of_mem _ member)
    have tails : ∀ pair : Row × Binding,
        (joinFamily left rest pair.2).map (fun (trace, result) =>
          ((occurrence, pair.1) :: trace, result)) =
        (joinFamily right rest pair.2).map (fun (trace, result) =>
          ((occurrence, pair.1) :: trace, result)) := by
      intro pair
      exact congrArg (List.map _) (induction tailSteps pair.2)
    simp only [joinFamily, headSame]
    exact congrArg (List.flatMap · (right occurrence before)) (funext tails)

end Zetesis.TableBindings
