import Progress

open Aeneas Aeneas.Std Result
open ZetesisExtract

/-!
# Truth prefixes for the concrete formula table

This finite fold describes the value sequence against which actual generated
steps are checked. Child indices and optional mask positions are totalized here;
ordered-node and mask-coverage premises justify the concrete reads. The fold
is not a replacement extracted implementation or a claim about array costs.
-/
namespace EvaluationSpecification

/-- Every child index names a node earlier in construction order. -/
def ChildrenBefore (position : Nat) : theory.Node → Prop
  | .Atom _ | .False => True
  | .And left right | .Or left right | .Implies left right =>
    left.val < position ∧ right.val < position

/-- A whole table uses only earlier child positions. Atom storage is a separate
interpretation invariant, not part of this dependency-order predicate. -/
def Ordered (table : List theory.Node) : Prop :=
  ∀ index (inside : index < table.length), ChildrenBefore index table[index]

/-- A supplied optional mask selects the current truth. Absent mask means the
original pass; a missing entry in a present mask has false as its total default. -/
def maskedAt (frozen : Option (Slice Bool)) (position : Nat) (truth : Bool) : Bool :=
  truth && (frozen.map (fun mask => mask.val[position]?.getD false)).getD true

/-- Formula values in construction order, from an empty prefix. -/
def values (candidate : theory.Interpretation) (frozen : Option (Slice Bool))
    (table : List theory.Node) : List Bool :=
  table.foldl (fun earlier node => earlier ++
    [maskedAt frozen earlier.length (Evaluation.value candidate earlier node)]) []

/-- Values of exactly the visited prefix, retaining the original mask positions. -/
def prefixValues (candidate : theory.Interpretation) (frozen : Option (Slice Bool))
    (table : List theory.Node) (count : Nat) : List Bool :=
  values candidate frozen (table.take count)

/-- Concrete and mathematical masking use the same represented position. -/
theorem masked_exact (frozen : Option (Slice Bool)) (index : Usize) (truth : Bool) :
    Evaluation.masked frozen index truth = maskedAt frozen index.val truth := rfl

/-- A structural child bound depends only on prefix length, not its truth values. -/
theorem children_present (node : theory.Node) (truths : List Bool)
    (position : Nat) (length : truths.length = position)
    (before : ChildrenBefore position node) : Evaluation.ChildrenPresent truths node := by
  cases node with
  | Atom _ | False => trivial
  | And _ _ | Or _ _ | Implies _ _ =>
    simpa only [ChildrenBefore, Evaluation.ChildrenPresent, length] using before

/-- Each processed node contributes exactly one value. -/
theorem values_length (candidate : theory.Interpretation) (frozen : Option (Slice Bool))
    (table : List theory.Node) : (values candidate frozen table).length = table.length := by
  have foldLength (remaining : List theory.Node) (earlier : List Bool) :
      (remaining.foldl (fun truths node => truths ++
        [maskedAt frozen truths.length (Evaluation.value candidate truths node)]) earlier).length =
      earlier.length + remaining.length := by
    induction remaining generalizing earlier with
    | nil => simp
    | cons node rest ih => simp [List.foldl_cons, ih, Nat.add_comm, Nat.add_left_comm]
  simpa [values] using foldLength table []

/-- A bounded visited prefix has exactly its cursor's number of truth values. -/
theorem prefix_length (candidate : theory.Interpretation) (frozen : Option (Slice Bool))
    (table : List theory.Node) (count : Nat) (bounded : count ≤ table.length) :
    (prefixValues candidate frozen table count).length = count := by
  simp [prefixValues, values_length, List.length_take, Nat.min_eq_left bounded]

/-- Processing a table suffix after its last node appends precisely that node's
masked truth, evaluated against the unchanged earlier prefix. -/
theorem values_append (candidate : theory.Interpretation) (frozen : Option (Slice Bool))
    (table : List theory.Node) (node : theory.Node) :
    values candidate frozen (table ++ [node]) = values candidate frozen table ++
      [maskedAt frozen table.length (Evaluation.value candidate (values candidate frozen table) node)] := by
  simp only [values, List.foldl_append, List.foldl_cons, List.foldl_nil]
  change _ ++ [maskedAt frozen (values candidate frozen table).length _] = _
  rw [values_length]

/-- The next prefix is the old truth sequence followed by the next node value.
The index premise supplies the actual table element, rather than a default node. -/
theorem prefix_succ (candidate : theory.Interpretation) (frozen : Option (Slice Bool))
    (table : List theory.Node) (count : Nat) (inside : count < table.length) :
    prefixValues candidate frozen table (count + 1) = prefixValues candidate frozen table count ++
      [maskedAt frozen count (Evaluation.value candidate
        (prefixValues candidate frozen table count) table[count])] := by
  simp only [prefixValues]
  rw [List.take_add_one, List.getElem?_eq_getElem inside]
  simp only [Option.toList_some, values_append, List.length_take, Nat.min_eq_left (Nat.le_of_lt inside)]

/-- At the end, a truth prefix is the whole table's value sequence. -/
theorem prefix_complete (candidate : theory.Interpretation) (frozen : Option (Slice Bool))
    (table : List theory.Node) :
    prefixValues candidate frozen table table.length = values candidate frozen table := by
  simp [prefixValues]

end EvaluationSpecification
