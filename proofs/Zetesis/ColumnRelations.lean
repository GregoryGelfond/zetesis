import Zetesis.Core

/-!
# Typed relation columns and ordered row selection

A dictionary names complete logical values by finite equality identifiers.
Aligned columns retain a finite row domain shared by every argument position.
Reconstruction preserves the whole tuple; independent column memberships are
not a substitute for that row correlation. The row count remains explicit even
when the arity is zero.

Equality selection retains exactly the satisfying input subsequence. A complete
matcher may impose additional conditions; filtering preserves its complete row
sequence only when every full match satisfies the selected equalities. A scoped
selection can be applied only to its own immutable snapshot.

These are representation contracts over finite functions and lists. They do not
establish Rust dictionary construction, ownership/lifetimes, checked ID packing,
GPU mask reconstruction, allocation bounds or grounding coverage. The matcher
is a total Boolean predicate here; source errors and resource-limited prefixes
require separate preservation. Snapshot
identifiers denote fixed relations in this model, not reusable memory addresses.
Possible-support relations and current-world relations remain different subjects.
-/

namespace Zetesis.ColumnRelations

universe u v
variable {Value : Type u} {Owner : Type v}

/-- IDs name equality classes of complete values. Their numerical order has no
logical ordering or arithmetic meaning. Values absent from the finite dictionary
have no encoding. Both round trips are obligations of dictionary construction. -/
structure Dictionary (Value : Type u) where
  size : Nat
  decode : Fin size → Value
  encode : Value → Option (Fin size)
  encode_decode : ∀ id, encode (decode id) = some id
  decode_encode : ∀ value id, encode value = some id → decode id = value

/-- A successful lookup names exactly the decoded value. Decode then encode
supplies completeness; the other round trip supplies soundness. -/
theorem encoding_exact (dictionary : Dictionary Value) (value : Value)
    (id : Fin dictionary.size) :
    dictionary.encode value = some id ↔ dictionary.decode id = value := by
  constructor
  · exact dictionary.decode_encode value id
  · intro same
    rw [← same]
    exact dictionary.encode_decode id

/-- Two IDs denote equal complete values precisely when they are the same ID.
Apply encoding to equal decoded values, then use its round trip. -/
theorem identifier_equality (dictionary : Dictionary Value)
    (left right : Fin dictionary.size) :
    dictionary.decode left = dictionary.decode right ↔ left = right := by
  constructor
  · intro same
    have encoded : some left = some right := by
      calc
        some left = dictionary.encode (dictionary.decode left) :=
          (dictionary.encode_decode left).symm
        _ = dictionary.encode (dictionary.decode right) := congrArg dictionary.encode same
        _ = some right := dictionary.encode_decode right
    exact Option.some.inj encoded
  · intro same
    exact congrArg dictionary.decode same

/-- All columns use one finite row domain. Rows and arity are independent: a
zero-arity relation can contain zero rows or an existing empty tuple. -/
abbrev Columns (dictionary : Dictionary Value) (rows arity : Nat) :=
  Fin arity → Fin rows → Fin dictionary.size

/-- Decode one complete tuple without changing its shared row identity. -/
def reconstruct (dictionary : Dictionary Value) {rows arity : Nat}
    (columns : Columns dictionary rows arity) (row : Fin rows) : Fin arity → Value :=
  fun column => dictionary.decode (columns column row)

/-- Encoding every original cell into its corresponding column reconstructs the
original finite table. Each argument uses the same row; the dictionary round trip
then gives that original value. No Cartesian product of columns occurs. -/
theorem reconstruction_exact (dictionary : Dictionary Value) {rows arity : Nat}
    (original : Fin rows → Fin arity → Value)
    (columns : Columns dictionary rows arity)
    (encoded : ∀ row column,
      dictionary.encode (original row column) = some (columns column row)) :
    reconstruct dictionary columns = original := by
  funext row column
  exact dictionary.decode_encode (original row column) (columns column row) (encoded row column)

/-- A finite list of complete-value equalities. Duplicate equalities retain their
ordinary conjunctive meaning; none of them interprets structural patterns. -/
abbrev Equalities (Value : Type u) (arity : Nat) := List (Fin arity × Value)

/-- A row satisfies every requested equality at the corresponding argument. -/
def Holds {arity : Nat} (row : Fin arity → Value) (equalities : Equalities Value arity) : Prop :=
  ∀ equality ∈ equalities, row equality.1 = equality.2

/-- Equality-only predicate over IDs. A missing value is distinct from every
valid cell ID, so its equality rejects each row. -/
def accepts (dictionary : Dictionary Value) {rows arity : Nat}
    (columns : Columns dictionary rows arity) (equalities : Equalities Value arity)
    (row : Fin rows) : Bool :=
  equalities.all fun equality =>
    decide (dictionary.encode equality.2 = some (columns equality.1 row))

/-- ID comparisons and typed row equalities have exactly the same truth value.
This uses both dictionary round trips and the shared row position. -/
theorem accepts_exact (dictionary : Dictionary Value) {rows arity : Nat}
    (columns : Columns dictionary rows arity) (equalities : Equalities Value arity)
    (row : Fin rows) :
    accepts dictionary columns equalities row = true ↔
      Holds (reconstruct dictionary columns row) equalities := by
  simp only [accepts, List.all_eq_true, decide_eq_true_eq, encoding_exact,
    Holds, reconstruct]

/-- Filter a supplied row sequence without changing retained positions or order.
The supplied sequence may be a full scan or an already justified posting. -/
def select (dictionary : Dictionary Value) {rows arity : Nat}
    (columns : Columns dictionary rows arity) (equalities : Equalities Value arity)
    (input : List (Fin rows)) : List (Fin rows) :=
  input.filter (accepts dictionary columns equalities)

/-- Selected membership is exactly input membership plus all typed equalities.
Completeness is relative to the supplied input, not an unexamined relation scan. -/
theorem selection_exact (dictionary : Dictionary Value) {rows arity : Nat}
    (columns : Columns dictionary rows arity) (equalities : Equalities Value arity)
    (input : List (Fin rows)) (row : Fin rows) :
    row ∈ select dictionary columns equalities input ↔
      row ∈ input ∧ Holds (reconstruct dictionary columns row) equalities := by
  simp only [select, List.mem_filter, accepts_exact]

/-- Selection preserves the input as an ordered subsequence; in particular it
never invents, reorders or multiplies occurrences. -/
theorem selection_subsequence (dictionary : Dictionary Value) {rows arity : Nat}
    (columns : Columns dictionary rows arity) (equalities : Equalities Value arity)
    (input : List (Fin rows)) : List.Sublist (select dictionary columns equalities input) input :=
  List.filter_sublist

/-- Increasing input positions remain increasing and therefore unique. This is
inherited from list filtering, not a property inferred from equal tuple values. -/
theorem selection_increasing (dictionary : Dictionary Value) {rows arity : Nat}
    (columns : Columns dictionary rows arity) (equalities : Equalities Value arity)
    (input : List (Fin rows))
    (ordered : input.Pairwise (fun left right => left.val < right.val)) :
    (select dictionary columns equalities input).Pairwise
      (fun left right => left.val < right.val) :=
  ordered.filter (accepts dictionary columns equalities)

/-- An empty equality list retains the whole input, including nullary rows. -/
theorem no_equalities_preserve_input (dictionary : Dictionary Value) {rows arity : Nat}
    (columns : Columns dictionary rows arity) (input : List (Fin rows)) :
    select dictionary columns [] input = input := by
  simp [select, accepts]

/-- If a requested whole value is absent from the dictionary, no row can satisfy
that equality. This states lookup absence, not construction failure. -/
theorem absent_value_selects_nothing (dictionary : Dictionary Value) {rows arity : Nat}
    (columns : Columns dictionary rows arity) (equalities : Equalities Value arity)
    (input : List (Fin rows)) (equality : Fin arity × Value)
    (requested : equality ∈ equalities) (absent : dictionary.encode equality.2 = none) :
    select dictionary columns equalities input = [] := by
  apply List.eq_nil_iff_forall_not_mem.mpr
  intro row member
  have accepted := (List.mem_filter.mp member).2
  have value_present := (List.all_eq_true.mp accepted) equality requested
  simp [absent] at value_present

/-- Prefiltering preserves the complete ordered sequence of full matches when
matching entails every prefilter equality. Other matcher conditions remain with
the original matcher; equality consistency does not prove a full match.

The filter composition conjoins both predicates. On a matching row the equality
premise makes that conjunction true; on every other row both paths reject. -/
theorem full_matches_preserved (dictionary : Dictionary Value) {rows arity : Nat}
    (columns : Columns dictionary rows arity) (equalities : Equalities Value arity)
    (input : List (Fin rows)) (fullMatch : Fin rows → Bool)
    (required : ∀ row ∈ input, fullMatch row = true →
      Holds (reconstruct dictionary columns row) equalities) :
    (select dictionary columns equalities input).filter fullMatch = input.filter fullMatch := by
  simp only [select, List.filter_filter]
  apply List.filter_congr
  intro row inside
  cases accepted : fullMatch row with
  | false => simp
  | true =>
    have selected := (accepts_exact dictionary columns equalities row).mpr
      (required row inside accepted)
    simp [selected]

/-- External row positions carry the identity of one immutable relation snapshot.
The owner is not inferred from arity, row count or lifetime overlap. -/
structure ScopedRows (Owner : Type v) where
  owner : Owner
  positions : List Nat

/-- Applicability validates both the owner and every row position before exposing
a selection. A checked runtime view may enforce these facts by construction. -/
def applicable [DecidableEq Owner] (owner : Owner) (rows : Nat)
    (selection : ScopedRows Owner) : Bool :=
  decide (selection.owner = owner) && selection.positions.all (fun row => decide (row < rows))

/-- A selection is applicable precisely to its owner with all positions in range.
Owner tokens are assumed to identify fixed snapshots throughout the operation. -/
theorem applicability_exact [DecidableEq Owner] (owner : Owner) (rows : Nat)
    (selection : ScopedRows Owner) :
    applicable owner rows selection = true ↔
      selection.owner = owner ∧ ∀ row ∈ selection.positions, row < rows := by
  simp [applicable, List.all_eq_true]

end Zetesis.ColumnRelations
