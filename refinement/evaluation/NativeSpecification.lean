import NativeEvaluation

open Aeneas Aeneas.Std Result
open ZetesisNativeExtract

/-!
# Truth prefixes for the native decoded table

This finite fold is a specification over the complete NodeViews returned by
paired storage. Totalized reads are used only to state values; backward-child
and mask-coverage premises justify actual reads. No binary shadow graph is built.
-/
namespace NativeSpecification

/-- Every view refers only to nodes in its already completed prefix. -/
def Ordered (table : List theory.NodeView) : Prop :=
  ∀ index (inside : index < table.length),
    NativeStructure.ChildrenBefore index table[index]

/-- A supplied optional mask selects the current truth. Absent mask means the
original pass; a missing entry in a present mask has false as its total default. -/
def maskedAt (frozen : Option (Slice Bool)) (position : Nat) (truth : Bool) : Bool :=
  truth && (frozen.map (fun mask => mask.val[position]?.getD false)).getD true

/-- Formula values in construction order, from an empty prefix. -/
def values (candidate : theory.Interpretation) (frozen : Option (Slice Bool))
    (table : List theory.NodeView) : List Bool :=
  table.foldl (fun earlier node => earlier ++
    [maskedAt frozen earlier.length (NativeEvaluation.value candidate earlier node)]) []

/-- Values of exactly the visited prefix, retaining the original mask positions. -/
def prefixValues (candidate : theory.Interpretation) (frozen : Option (Slice Bool))
    (table : List theory.NodeView) (count : Nat) : List Bool :=
  values candidate frozen (table.take count)

/-- Each processed node contributes exactly one value. -/
theorem values_length (candidate : theory.Interpretation) (frozen : Option (Slice Bool))
    (table : List theory.NodeView) : (values candidate frozen table).length = table.length := by
  have foldLength (remaining : List theory.NodeView) (earlier : List Bool) :
      (remaining.foldl (fun truths node => truths ++
        [maskedAt frozen truths.length (NativeEvaluation.value candidate truths node)]) earlier).length =
      earlier.length + remaining.length := by
    induction remaining generalizing earlier with
    | nil => simp
    | cons node rest ih => simp [List.foldl_cons, ih, Nat.add_comm, Nat.add_left_comm]
  simpa [values] using foldLength table []

/-- A bounded visited prefix has exactly its cursor's number of truth values. -/
theorem prefix_length (candidate : theory.Interpretation) (frozen : Option (Slice Bool))
    (table : List theory.NodeView) (count : Nat) (bounded : count ≤ table.length) :
    (prefixValues candidate frozen table count).length = count := by
  simp [prefixValues, values_length, List.length_take, Nat.min_eq_left bounded]

/-- Processing a table suffix after its last node appends precisely that node's
masked truth, evaluated against the unchanged earlier prefix. -/
theorem values_append (candidate : theory.Interpretation) (frozen : Option (Slice Bool))
    (table : List theory.NodeView) (node : theory.NodeView) :
    values candidate frozen (table ++ [node]) = values candidate frozen table ++
      [maskedAt frozen table.length (NativeEvaluation.value candidate (values candidate frozen table) node)] := by
  simp only [values, List.foldl_append, List.foldl_cons, List.foldl_nil]
  change _ ++ [maskedAt frozen (values candidate frozen table).length _] = _
  rw [values_length]

/-- The next prefix is the old truth sequence followed by the next node value.
The index premise supplies the actual table element, rather than a default node. -/
theorem prefix_succ (candidate : theory.Interpretation) (frozen : Option (Slice Bool))
    (table : List theory.NodeView) (count : Nat) (inside : count < table.length) :
    prefixValues candidate frozen table (count + 1) = prefixValues candidate frozen table count ++
      [maskedAt frozen count (NativeEvaluation.value candidate
        (prefixValues candidate frozen table count) table[count])] := by
  simp only [prefixValues]
  rw [List.take_add_one, List.getElem?_eq_getElem inside]
  simp only [Option.toList_some, values_append, List.length_take, Nat.min_eq_left (Nat.le_of_lt inside)]

/-- At the end, a truth prefix is the whole table's value sequence. -/
theorem prefix_complete (candidate : theory.Interpretation) (frozen : Option (Slice Bool))
    (table : List theory.NodeView) :
    prefixValues candidate frozen table table.length = values candidate frozen table := by
  simp [prefixValues]

/-- A complete prefix visits its logical nodes and every referenced occurrence. -/
def cost (table : List theory.NodeView) : Nat :=
  (table.map (fun row => 1 + NativeEvaluation.occurrences row)).sum

/-- Appending one completed node charges its node tick plus its operand count. -/
theorem cost_append (table : List theory.NodeView) (row : theory.NodeView) :
    cost (table ++ [row]) = cost table + (1 + NativeEvaluation.occurrences row) := by
  simp [cost]

/-- The complete cost separates logical node visits from operand occurrences. -/
theorem cost_dimensions (table : List theory.NodeView) :
    cost table = table.length + (table.map NativeEvaluation.occurrences).sum := by
  induction table with
  | nil => rfl
  | cons row rest ih => simp [cost, List.sum_cons] at *; omega

/-- Extending the visited prefix adds exactly the next logical node's charge. -/
theorem cost_prefix_succ (table : List theory.NodeView) (count : Nat)
    (inside : count < table.length) :
    cost (table.take (count + 1)) = cost (table.take count) +
      (1 + NativeEvaluation.occurrences table[count]) := by
  rw [List.take_add_one, List.getElem?_eq_getElem inside]
  simp only [Option.toList_some, cost_append]

end NativeSpecification
