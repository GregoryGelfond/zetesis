import Zetesis.Core

/-!
# Ordered adjacency rows in contiguous storage

An adjacency index is a finite family of ordered rows. Concatenating their
entries and retaining row boundaries changes storage without changing a row.
This matters for propagation: operand order and repeated occurrences must not
be replaced by set membership alone.

The laws below read a row by the length of its preceding rows and its own
length. A Rust offset table must compute exactly those boundaries. They do not
verify the counting/filling builder, integer bounds, allocation, work charging
or the propagation algorithm consuming the rows.
-/

namespace Zetesis.AdjacencyRows

universe u v
variable {Entry : Type u} {State : Type v}

/-- Read the consecutive entries beginning at `start`, with the supplied width.
The representation correspondence must justify both values. -/
def slice (entries : List Entry) (start width : Nat) : List Entry :=
  (entries.drop start).take width

/-- A row read from the concatenation of all rows is exactly the original row,
including its order and repeated entries. Empty rows have width zero.
Flattening separates the preceding entries from this row and its successors;
dropping that prefix and taking the row's length recovers the row. -/
theorem slice_exact (before : List (List Entry)) (row : List Entry)
    (after : List (List Entry)) :
    slice (before ++ row :: after).flatten before.flatten.length row.length = row := by
  simp only [slice, List.flatten_append, List.flatten_cons,
    List.drop_append_length, List.take_append_length]

/-- Any ordered state reduction over a recovered row has the same result as
over the original row. The reducer may depend on order and multiplicity; it
need not commute. This states equivalence of the row inputs, not correctness of
the reducer itself. -/
theorem fold_exact (step : State → Entry → State) (initial : State)
    (before : List (List Entry)) (row : List Entry) (after : List (List Entry)) :
    (slice (before ++ row :: after).flatten before.flatten.length row.length).foldl
      step initial = row.foldl step initial := by
  rw [slice_exact]

end Zetesis.AdjacencyRows
