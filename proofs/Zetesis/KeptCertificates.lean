import Zetesis.AxisFolds

/-!
# Certificates kept over append-only rows

Support completion grows its relations one round at a time, and a round only
appends rows: every earlier row keeps its position and its values. Two numeric
certificates over a relation's rows are kept from one round to the next instead
of being rebuilt. Each is the fold of a commutative monoid over the rows, so the
fold laws of `AxisFolds` decide how a kept certificate extends.

- A column's integer hull joins the rows' values. A row holding a number
  contributes its point; a row holding anything else declines the hull. Joining
  the kept hull with the appended rows' hull gives the hull of the whole column
  (`hull_extend`), and a declined hull stays declined whatever is appended
  (`hull_declined_stays`).
- A key certificate states that every row's key is defined. The certificate of
  the whole relation is the kept certificate and the appended rows' certificate
  (`defined_extend`), and a declined certificate stays declined
  (`defined_declined_stays`).

In both, the kept rows are the first rows of every later snapshot
(`kept_rows_prefix`), so a certificate computed over them is a certificate over
that prefix.

Not proved here, and obligations of the implementation: that support rows are
append-only with stable positions and values; that the Rust hull and key
certificate compute these folds, including the decline on a nonnumeric value or
an undefined key; and that merging the appended keys' sorted entries from the
end keeps the index ordered by key and row.
-/

namespace Zetesis.KeptCertificates

open AxisFolds

/-- An integer hull: no value read yet, a closed range, or a decline. -/
inductive Hull where
  | empty
  | bounded (low high : Int)
  | declined
  deriving DecidableEq

/-- Join two hulls. The empty hull is the identity; a decline absorbs. -/
def Hull.join : Hull → Hull → Hull
  | .empty, other => other
  | .declined, _ => .declined
  | .bounded low high, .empty => .bounded low high
  | .bounded _ _, .declined => .declined
  | .bounded low high, .bounded low' high' => .bounded (min low low') (max high high')

/-- Hulls under join: the commutative monoid a column's hull folds. -/
def hull : Reduction Hull where
  op := Hull.join
  unit := .empty
  assoc := fun a b c => by
    cases a <;> cases b <;> cases c <;>
      simp only [Hull.join, Int.min_assoc, Int.max_assoc]
  comm := fun a b => by
    cases a <;> cases b <;> simp only [Hull.join, Int.min_comm, Int.max_comm]
  unit_op := fun a => by cases a <;> rfl

/-- One row's contribution to a column's hull: the point of a number, or a
decline for any other value. -/
def point : Option Int → Hull
  | some value => .bounded value value
  | none => .declined

theorem hull_declined_absorbing : hull.Absorbing .declined := fun a => by
  cases a <;> rfl

/-- **Extension.** The hull of a column whose kept rows gained appended rows is
the kept hull joined with the appended rows' hull. -/
theorem hull_extend (kept appended : List (Option Int)) :
    hull.fold point (kept ++ appended)
      = hull.op (hull.fold point kept) (hull.fold point appended) :=
  hull.fold_append point kept appended

/-- **A decline stays.** Folding appended rows into a declined hull leaves it
declined. -/
theorem hull_declined_stays (appended : List (Option Int)) :
    hull.foldFrom point .declined appended = .declined :=
  hull.foldFrom_absorbing point hull_declined_absorbing appended

/-- **Extension.** Every key of the extended relation is defined exactly when
every kept key and every appended key is. -/
theorem defined_extend {Row : Type} (defined : Row → Bool) (kept appended : List Row) :
    all.fold defined (kept ++ appended)
      = (all.fold defined kept && all.fold defined appended) :=
  all.fold_append defined kept appended

/-- **A decline stays.** A key certificate declined over the kept rows remains
declined over any extension. -/
theorem defined_declined_stays {Row : Type} (defined : Row → Bool) (appended : List Row) :
    all.foldFrom defined false appended = false :=
  all.foldFrom_absorbing defined all_absorbing appended

/-- The kept rows are the first rows of every later snapshot. -/
theorem kept_rows_prefix {Row : Type} (kept appended : List Row) :
    (kept ++ appended).take kept.length = kept := by
  simp

end Zetesis.KeptCertificates
