import NativeSemantics
import NativeSpecification

open Aeneas Aeneas.Std
open ZetesisNativeExtract
open Zetesis Ferraris TightEvaluation

/-!
# Original and frozen native truth tables

A native decoded table unfolds one formula per logical node. The original truth
fold establishes the mask used by the frozen pass. The finite induction uses
the existing Ferraris layer laws through NativeSemantics; tested interpretations
are arbitrary. These are specifications of the native view table; the separate
actual-loop theorem connects generated execution to its value fold.
-/
namespace NativeDenotation

/-- Decode all native views in order, retaining earlier formula meanings. -/
def meanings (table : List theory.NodeView) : List (Formula Nat) :=
  table.foldl (fun earlier node => earlier ++ [NativeSemantics.meaning earlier node]) []

/-- One logical native node contributes exactly one decoded formula. -/
theorem meanings_length (table : List theory.NodeView) : (meanings table).length = table.length := by
  have size (rest : List theory.NodeView) (earlier : List (Formula Nat)) :
      (rest.foldl (fun before node => before ++ [NativeSemantics.meaning before node]) earlier).length =
        earlier.length + rest.length := by
    induction rest generalizing earlier with
    | nil => simp
    | cons node rest ih => simp [List.foldl_cons, ih, Nat.add_assoc, Nat.add_comm]
  simpa [meanings] using size table []

/-- Appending a node keeps every earlier decoded formula intact. -/
theorem meanings_append (table : List theory.NodeView) (node : theory.NodeView) :
    meanings (table ++ [node]) = meanings table ++ [NativeSemantics.meaning (meanings table) node] := by
  simp [meanings, List.foldl_append]

/-- The dependency-order premise for a last node separates from its prefix. -/
theorem ordered_append (table : List theory.NodeView) (node : theory.NodeView)
    (ordered : NativeSpecification.Ordered (table ++ [node])) :
    NativeSpecification.Ordered table ∧ NativeStructure.ChildrenBefore table.length node := by
  constructor
  · intro index inside
    have admitted : NativeStructure.ChildrenBefore index
        ((table ++ [node])[index]'(by simp; omega)) := by
      exact ordered index (by simp; omega)
    simpa [List.getElem_append_left inside] using admitted
  · have admitted : NativeStructure.ChildrenBefore table.length
        ((table ++ [node])[table.length]'(by simp)) := by
      exact ordered table.length (by simp)
    simpa using admitted

/-- The original fold computes exactly every decoded formula's original truth.
There is no mask-agreement premise; the original table is derived here. -/
theorem original (candidate : theory.Interpretation) (table : List theory.NodeView)
    (ordered : NativeSpecification.Ordered table) :
    NativeSpecification.values candidate none table =
      (meanings table).map (formulaValue (NativeMembership.denotes candidate)) := by
  induction table using List.reverseRecOn with
  | nil => rfl
  | append_singleton table node ih =>
    obtain ⟨before, children⟩ := ordered_append table node ordered
    have prefixExact : NativeSpecification.values candidate none table =
        (meanings table).map (formulaValue (NativeMembership.denotes candidate)) := by
      exact ih before
    have layer : NativeEvaluation.value candidate
        ((meanings table).map (formulaValue (NativeMembership.denotes candidate))) node =
          formulaValue (NativeMembership.denotes candidate)
            (NativeSemantics.meaning (meanings table) node) := by
      exact NativeSemantics.original candidate (meanings table) node
        (by simpa only [meanings_length] using children)
    rw [NativeSpecification.values_append, meanings_append]
    simp only [prefixExact, List.map_append, List.map_cons, List.map_nil,
      NativeSpecification.maskedAt, Option.map_none, Option.getD_none, Bool.and_true, layer]

/-- Every mask cell within this table is its actual decoded original truth.
The mask may contain a later suffix that this prefix does not read. -/
def MaskAgrees (outer : theory.Interpretation) (mask : Slice Bool)
    (table : List theory.NodeView) : Prop :=
  ∀ index (inside : index < table.length),
    mask.val[index]? = some (formulaValue (NativeMembership.denotes outer)
      ((meanings table)[index]'(by rw [meanings_length]; exact inside)))

/-- A whole-table original mask remains correct for the unchanged earlier rows. -/
theorem mask_prefix (outer : theory.Interpretation) (mask : Slice Bool)
    (table : List theory.NodeView) (node : theory.NodeView)
    (agrees : MaskAgrees outer mask (table ++ [node])) : MaskAgrees outer mask table := by
  intro index inside
  have cell : mask.val[index]? = some (formulaValue (NativeMembership.denotes outer)
      ((meanings (table ++ [node]))[index]'(by rw [meanings_length]; simp; omega))) := by
    exact agrees index (by simp; omega)
  have decodedInside : index < (meanings table).length := by simpa [meanings_length] using inside
  simpa only [meanings_append, List.getElem_append_left decodedInside] using cell

/-- The last mask cell is the original truth of the last decoded native layer. -/
theorem mask_last (outer : theory.Interpretation) (mask : Slice Bool)
    (table : List theory.NodeView) (node : theory.NodeView)
    (agrees : MaskAgrees outer mask (table ++ [node])) :
    mask.val[table.length]? = some (formulaValue (NativeMembership.denotes outer)
      (NativeSemantics.meaning (meanings table) node)) := by
  have cell : mask.val[table.length]? = some (formulaValue (NativeMembership.denotes outer)
      ((meanings (table ++ [node]))[table.length]'(by rw [meanings_length]; simp))) := by
    exact agrees table.length (by simp)
  simpa [meanings_append, ← meanings_length table] using cell

/-- Given the actual original mask, the frozen fold computes the explicit
Ferraris reduct under every tested interpretation. The proof uses no tested-
subset restriction and no alternative necessary-support semantics. -/
theorem frozen (outer tested : theory.Interpretation) (mask : Slice Bool)
    (table : List theory.NodeView) (ordered : NativeSpecification.Ordered table)
    (agrees : MaskAgrees outer mask table) :
    NativeSpecification.values tested (some mask) table =
      (meanings table).map (fun formula => formulaValue (NativeMembership.denotes tested)
        (Reduct (interpretation (NativeMembership.denotes outer)) formula)) := by
  induction table using List.reverseRecOn with
  | nil => rfl
  | append_singleton table node ih =>
    obtain ⟨before, children⟩ := ordered_append table node ordered
    have prefixExact : NativeSpecification.values tested (some mask) table =
        (meanings table).map (fun formula => formulaValue (NativeMembership.denotes tested)
          (Reduct (interpretation (NativeMembership.denotes outer)) formula)) := by
      exact ih before (mask_prefix outer mask table node agrees)
    have lastCell : mask.val[table.length]? = some (formulaValue (NativeMembership.denotes outer)
        (NativeSemantics.meaning (meanings table) node)) := by
      exact mask_last outer mask table node agrees
    have layer : (NativeEvaluation.value tested
        ((meanings table).map (fun child => formulaValue (NativeMembership.denotes tested)
          (Reduct (interpretation (NativeMembership.denotes outer)) child))) node &&
          formulaValue (NativeMembership.denotes outer)
            (NativeSemantics.meaning (meanings table) node)) =
        formulaValue (NativeMembership.denotes tested)
          (Reduct (interpretation (NativeMembership.denotes outer))
            (NativeSemantics.meaning (meanings table) node)) := by
      exact NativeSemantics.frozen outer tested (meanings table) node
        (by simpa only [meanings_length] using children)
    rw [NativeSpecification.values_append, meanings_append]
    simp only [prefixExact, List.map_append, List.map_cons, List.map_nil,
      NativeSpecification.maskedAt, Option.map_some, Option.getD_some, lastCell, layer]

/-- A mask returned as the original value fold satisfies the mask premise by
that fold's proved original semantics, rather than assumed Boolean agreement. -/
theorem original_mask (outer : theory.Interpretation) (mask : Slice Bool)
    (table : List theory.NodeView) (ordered : NativeSpecification.Ordered table)
    (produced : mask.val = NativeSpecification.values outer none table) :
    MaskAgrees outer mask table := by
  intro index inside
  have decodedInside : index < (meanings table).length := by simpa [meanings_length] using inside
  rw [produced, original outer table ordered]
  simp [List.getElem?_map, List.getElem?_eq_getElem decodedInside]

end NativeDenotation
