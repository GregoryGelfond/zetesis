import Zetesis.ColumnRelations

/-!
# Extending an equality layout

An appendable owner preserves each existing row and its equality identifiers.
The new dictionary may contain additional values, but each old identifier still
decodes to the same complete value. These obligations imply preservation of
whole-row reconstruction and equality selection on the old row domain.

The laws make representation preservation explicit. They do not prove Rust
reservation rollback, dictionary insertion, capacity accounting, source owner
lifetimes or a grounding fixed point. The embeddings below are logical maps;
the implementation additionally preserves their numeric row and equality IDs.
-/

namespace Zetesis.RelationExtension

open ColumnRelations

universe u
variable {Value : Type u}

/-- Existing rows have unchanged decoded tuples when both the old dictionary
    meanings and their column references survive extension. For each cell,
    substitute the preserved reference, then its preserved decoded value. -/
theorem reconstruction_preserved
    (before after : Dictionary Value) {oldRows newRows arity : Nat}
    (oldColumns : Columns before oldRows arity)
    (newColumns : Columns after newRows arity)
    (rowMap : Fin oldRows → Fin newRows)
    (idMap : Fin before.size → Fin after.size)
    (meanings : ∀ id, after.decode (idMap id) = before.decode id)
    (references : ∀ column row,
      newColumns column (rowMap row) = idMap (oldColumns column row)) :
    ∀ row, reconstruct after newColumns (rowMap row) =
      reconstruct before oldColumns row := by
  intro row
  funext column
  exact (congrArg after.decode (references column row)).trans
    (meanings (oldColumns column row))

/-- An unchanged complete tuple satisfies exactly the same equality query after
    extension. Apply the column representation's exactness law on both sides;
    the preserved reconstruction is their common logical row. -/
theorem acceptance_preserved
    (before after : Dictionary Value) {oldRows newRows arity : Nat}
    (oldColumns : Columns before oldRows arity)
    (newColumns : Columns after newRows arity)
    (rowMap : Fin oldRows → Fin newRows)
    (preserved : ∀ row, reconstruct after newColumns (rowMap row) =
      reconstruct before oldColumns row)
    (equalities : Equalities Value arity) (row : Fin oldRows) :
    accepts after newColumns equalities (rowMap row) =
      accepts before oldColumns equalities row := by
  apply Bool.eq_iff_iff.mpr
  rw [accepts_exact, accepts_exact, preserved row]

end Zetesis.RelationExtension
