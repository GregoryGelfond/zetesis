import Std

/-!
# Exact domain projection over complete rows

Row occurrences retain identity even when their values coincide. Column scopes
can repeat variables; coherence requires equal values at aliased columns.
Surviving rows belong to the supplied table and satisfy every domain. Projected
values have surviving whole-row witnesses.

These laws explain a value-to-row index and solution-preserving narrowing.
They do not establish ASP source completeness, producer support, candidate
truth or answer-set membership. Packing, Rust/WGSL execution, resource accounting
and source correspondence remain separate obligations. The mathematical laws do
not require finiteness; the executable table and domains are explicitly finite.
-/

namespace Zetesis.FiniteTables

variable {Row Column Variable Value : Type}

/-- Aliased columns of one row carry equal whole values. -/
def Coherent (values : Row → Column → Value) (scope : Column → Variable)
    (row : Row) : Prop :=
  ∀ left right, scope left = scope right → values row left = values row right

/-- Original rows respecting aliases and every supplied domain. -/
def Survives (table : Row → Prop) (values : Row → Column → Value)
    (scope : Column → Variable) (domains : Variable → Value → Prop)
    (row : Row) : Prop :=
  table row ∧ Coherent values scope row ∧
    ∀ column, domains (scope column) (values row column)

/-- A support bit names a coherent original row carrying the stated value. -/
def Support (table : Row → Prop) (values : Row → Column → Value)
    (scope : Column → Variable) (column : Column) (value : Value)
    (row : Row) : Prop :=
  table row ∧ Coherent values scope row ∧ values row column = value

/-- Per-column unions of value supports, intersected with coherent base rows.
The explicit base handles empty column scopes without manufacturing rows. -/
def IndexedSurvival (table : Row → Prop) (values : Row → Column → Value)
    (scope : Column → Variable) (domains : Variable → Value → Prop)
    (row : Row) : Prop :=
  table row ∧ Coherent values scope row ∧
    ∀ column, ∃ value, domains (scope column) value ∧
      Support table values scope column value row

/-- A projected value has a surviving complete-row occurrence as its witness. -/
def Narrowed (table : Row → Prop) (values : Row → Column → Value)
    (scope : Column → Variable) (domains : Variable → Value → Prop)
    (variableId : Variable) (value : Value) : Prop :=
  ∃ row column, Survives table values scope domains row ∧
    scope column = variableId ∧ values row column = value

/-- Support intersection selects exactly the complete admissible rows.

Each selected bit supplies the row value and its domain membership. Conversely,
a surviving row supplies its own value as the selected support in each column.
Both directions retain the same row occurrence. -/
theorem indexed_survival_exact
    (table : Row → Prop) (values : Row → Column → Value)
    (scope : Column → Variable) (domains : Variable → Value → Prop) (row : Row) :
    IndexedSurvival table values scope domains row ↔
      Survives table values scope domains row := by
  constructor
  · intro selected
    have admitted : ∀ column, domains (scope column) (values row column) := by
      intro column
      obtain ⟨value, inDomain, support⟩ := selected.2.2 column
      exact support.2.2.symm ▸ inDomain
    exact ⟨selected.1, selected.2.1, admitted⟩
  · intro surviving
    have supports : ∀ column, ∃ value, domains (scope column) value ∧
        Support table values scope column value row := by
      intro column
      exact ⟨values row column, surviving.2.2 column,
        surviving.1, surviving.2.1, rfl⟩
    exact ⟨surviving.1, surviving.2.1, supports⟩

/-- Every projected value was allowed by its original domain. -/
theorem narrowing_contracts
    {table : Row → Prop} {values : Row → Column → Value}
    {scope : Column → Variable} {domains : Variable → Value → Prop}
    {variableId : Variable} {value : Value}
    (witnessed : Narrowed table values scope domains variableId value) :
    domains variableId value := by
  obtain ⟨row, column, surviving, sameVariable, sameValue⟩ := witnessed
  exact sameValue ▸ (sameVariable ▸ surviving.2.2 column)

/-- Narrowing preserves exactly the surviving rows, including aliased columns.

A surviving row witnesses every one of its values, so none can be removed.
Conversely, projected values belong to the original domains; no new row appears. -/
theorem narrowing_preserves_rows
    (table : Row → Prop) (values : Row → Column → Value)
    (scope : Column → Variable) (domains : Variable → Value → Prop) (row : Row) :
    Survives table values scope (Narrowed table values scope domains) row ↔
      Survives table values scope domains row := by
  constructor
  · intro narrowed
    have admitted : ∀ column, domains (scope column) (values row column) := by
      intro column
      exact narrowing_contracts (narrowed.2.2 column)
    exact ⟨narrowed.1, narrowed.2.1, admitted⟩
  · intro original
    have witnessed : ∀ column,
        Narrowed table values scope domains (scope column) (values row column) := by
      intro column
      exact ⟨row, column, original, rfl, rfl⟩
    exact ⟨original.1, original.2.1, witnessed⟩

/-- A second projection removes no further values from this single table.
The previous law identifies the whole-row witnesses in both projections. -/
theorem narrowing_idempotent
    (table : Row → Prop) (values : Row → Column → Value)
    (scope : Column → Variable) (domains : Variable → Value → Prop)
    (variableId : Variable) (value : Value) :
    Narrowed table values scope (Narrowed table values scope domains) variableId value ↔
      Narrowed table values scope domains variableId value := by
  constructor
  · rintro ⟨row, column, surviving, sameVariable, sameValue⟩
    exact ⟨row, column,
      (narrowing_preserves_rows table values scope domains row).mp surviving,
      sameVariable, sameValue⟩
  · rintro ⟨row, column, surviving, sameVariable, sameValue⟩
    exact ⟨row, column,
      (narrowing_preserves_rows table values scope domains row).mpr surviving,
      sameVariable, sameValue⟩

/-- Widening domains cannot remove a surviving original row.
This concerns recomputation from the original table. Reusing an old reduced mask
as the new base would require a separate completeness argument. -/
theorem widening_preserves_rows
    {table : Row → Prop} {values : Row → Column → Value}
    {scope : Column → Variable} {small large : Variable → Value → Prop}
    (wider : ∀ variableId value, small variableId value → large variableId value)
    {row : Row} (surviving : Survives table values scope small row) :
    Survives table values scope large row := by
  have admitted : ∀ column, large (scope column) (values row column) := by
    intro column
    exact wider (scope column) (values row column) (surviving.2.2 column)
  exact ⟨surviving.1, surviving.2.1, admitted⟩

end Zetesis.FiniteTables
