import Zetesis.TightEvaluation

/-!
# Computing satisfaction of a frozen formula reduct

Two finite folds evaluate a shared formula DAG. The first computes original
truth in the outer interpretation. The second computes truth in an arbitrary
tested interpretation, conjoining each node with its stored outer truth. The
mask is computed once; the tested interpretation never changes it.

`values_correspond` proves that the second fold produces the truth of each
explicit Ferraris reduct. `roots_true_iff` then identifies the completed root
test with satisfaction of the reduct theory. Neither theorem assumes evaluator
agreement or requires the tested interpretation to be a subset of the outer one.

The operations reuse `DagSharing` and `TightEvaluation`. Their unavailable
references denote falsum, so the mathematical correspondence is total. This is
not runtime admission: checked indices, atom encoding, ownership, resource
stops, extraction and shader execution remain separate refinement obligations.
The list folds specify a finite computation, not an array-complexity bound.
-/

namespace Zetesis.ReductEvaluation

universe u
variable {α : Type u}
open Ferraris TightEvaluation

/-- Evaluate in construction order using one immutable outer-truth table.
The current output length is the node's index in that table. -/
private def maskedValues (tested : α → Bool) (mask : List Bool)
    (table : List (DagSharing.Node α)) : List Bool :=
  table.foldl (fun earlier node => earlier ++
    [nodeValue tested earlier node && mask.getD earlier.length false]) []

/-- Compute original truth once, then test the frozen DAG in any interpretation.
Only the first fold reads the outer interpretation; every second-pass atom read
uses the tested interpretation. -/
def values (outer tested : α → Bool) (table : List (DagSharing.Node α)) : List Bool :=
  maskedValues tested (TightEvaluation.values outer table) table

/-- The two actual folds compute the explicit reduct's truth at every stored
node. The first fold establishes the mask invariant; the second preserves the
whole reduct-truth prefix. No evaluator agreement or subset premise is assumed. -/
theorem values_correspond (outer tested : α → Bool) (table : List (DagSharing.Node α)) :
    values outer tested table = (DagSharing.meanings table).map
      (fun formula => formulaValue tested (Reduct (interpretation outer) formula)) := by
  /- One masked layer has the same truth as its explicit reduct. Earlier entries
  already denote child reducts; the current mask bit is the original layer's truth.
  Unavailable children have the common falsum meaning in both representations. -/
  have node_value_reduct (earlier : List (Formula α)) (node : DagSharing.Node α) :
      (nodeValue tested (earlier.map (fun formula =>
          formulaValue tested (Reduct (interpretation outer) formula))) node &&
          formulaValue outer (DagSharing.decode earlier node)) =
        formulaValue tested (Reduct (interpretation outer) (DagSharing.decode earlier node)) := by
    classical
    have lookup (index : Nat) :
        (earlier.map (fun formula =>
          formulaValue tested (Reduct (interpretation outer) formula))).getD index false =
        formulaValue tested (Reduct (interpretation outer) (earlier.getD index .bot)) := by
      simp only [List.getD_eq_getElem?_getD, List.getElem?_map]
      cases earlier[index]? <;> rfl
    cases node with
    | atom atom =>
      cases truth : outer atom <;>
        simp [nodeValue, DagSharing.decode, Reduct, interpretation, formulaValue, truth]
    | bot => rfl
    | conj left right =>
      by_cases truth : Satisfies (interpretation outer) (DagSharing.decode earlier (.conj left right))
      · have mask : formulaValue outer (DagSharing.decode earlier (.conj left right)) = true :=
          (formula_value_true outer _).mpr truth
        simp only [nodeValue, lookup, mask, Bool.and_true]
        simp only [DagSharing.decode] at truth
        simp only [DagSharing.decode, Reduct, if_pos truth, formulaValue]
      · have mask : formulaValue outer (DagSharing.decode earlier (.conj left right)) = false := by
          cases evaluated : formulaValue outer (DagSharing.decode earlier (.conj left right))
          · rfl
          · exact False.elim (truth ((formula_value_true outer _).mp evaluated))
        simp only [mask, Bool.and_false]
        simp only [DagSharing.decode] at truth
        simp only [DagSharing.decode, Reduct, if_neg truth, formulaValue]
    | disj left right =>
      by_cases truth : Satisfies (interpretation outer) (DagSharing.decode earlier (.disj left right))
      · have mask : formulaValue outer (DagSharing.decode earlier (.disj left right)) = true :=
          (formula_value_true outer _).mpr truth
        simp only [nodeValue, lookup, mask, Bool.and_true]
        simp only [DagSharing.decode] at truth
        simp only [DagSharing.decode, Reduct, if_pos truth, formulaValue]
      · have mask : formulaValue outer (DagSharing.decode earlier (.disj left right)) = false := by
          cases evaluated : formulaValue outer (DagSharing.decode earlier (.disj left right))
          · rfl
          · exact False.elim (truth ((formula_value_true outer _).mp evaluated))
        simp only [mask, Bool.and_false]
        simp only [DagSharing.decode] at truth
        simp only [DagSharing.decode, Reduct, if_neg truth, formulaValue]
    | imp left right =>
      by_cases truth : Satisfies (interpretation outer) (DagSharing.decode earlier (.imp left right))
      · have mask : formulaValue outer (DagSharing.decode earlier (.imp left right)) = true :=
          (formula_value_true outer _).mpr truth
        simp only [nodeValue, lookup, mask, Bool.and_true]
        simp only [DagSharing.decode] at truth
        simp only [DagSharing.decode, Reduct, if_pos truth, formulaValue]
      · have mask : formulaValue outer (DagSharing.decode earlier (.imp left right)) = false := by
          cases evaluated : formulaValue outer (DagSharing.decode earlier (.imp left right))
          · rfl
          · exact False.elim (truth ((formula_value_true outer _).mp evaluated))
        simp only [mask, Bool.and_false]
        simp only [DagSharing.decode] at truth
        simp only [DagSharing.decode, Reduct, if_neg truth, formulaValue]

  /- An exact stored mask suffices for every computed prefix. This internal
  invariant is discharged by the actual first fold in `values_correspond`.

  Induct in construction order. Restrict the mask to the old prefix, apply the
  induction hypothesis there, then use `node_value_reduct` for the appended node.
  The mask position is exactly the length of that completed prefix. -/
  have masked_values_correspond (table : List (DagSharing.Node α)) (mask : List Bool)
      (exactMask : ∀ index, index < table.length → mask.getD index false =
        formulaValue outer ((DagSharing.meanings table).getD index .bot)) :
      maskedValues tested mask table = (DagSharing.meanings table).map
        (fun formula => formulaValue tested (Reduct (interpretation outer) formula)) := by
    have prefixInvariant (reversed : List (DagSharing.Node α)) :
        ∀ frozen : List Bool,
          (∀ index, index < reversed.reverse.length → frozen.getD index false =
            formulaValue outer ((DagSharing.meanings reversed.reverse).getD index .bot)) →
          maskedValues tested frozen reversed.reverse =
            (DagSharing.meanings reversed.reverse).map
              (fun formula => formulaValue tested (Reduct (interpretation outer) formula)) := by
      induction reversed with
      | nil =>
        intro frozen _exact
        rfl
      | cons node reversed inductionHypothesis =>
        intro frozen exactFrozen
        simp only [List.reverse_cons] at exactFrozen ⊢
        have oldMask (index : Nat) (inside : index < reversed.reverse.length) :
            frozen.getD index false =
              formulaValue outer ((DagSharing.meanings reversed.reverse).getD index .bot) := by
          have available : index < (reversed.reverse ++ [node]).length := by
            simp only [List.length_append, List.length_singleton]
            omega
          have sameMeaning :
              (DagSharing.meanings (reversed.reverse ++ [node])).getD index .bot =
                (DagSharing.meanings reversed.reverse).getD index .bot := by
            rw [DagSharing.meanings_snoc]
            exact DagSharing.getD_append_left _ _ _ _ (by
              simpa only [DagSharing.meanings_length] using inside)
          exact (exactFrozen index available).trans (congrArg (formulaValue outer) sameMeaning)
        have oldValues : maskedValues tested frozen reversed.reverse =
            (DagSharing.meanings reversed.reverse).map
              (fun formula => formulaValue tested (Reduct (interpretation outer) formula)) :=
          inductionHypothesis frozen oldMask
        have currentMask : frozen.getD reversed.reverse.length false =
            formulaValue outer (DagSharing.decode (DagSharing.meanings reversed.reverse) node) := by
          have available : reversed.reverse.length < (reversed.reverse ++ [node]).length := by
            simp
          have currentMeaning :
              (DagSharing.meanings (reversed.reverse ++ [node])).getD reversed.reverse.length .bot =
                DagSharing.decode (DagSharing.meanings reversed.reverse) node := by
            rw [DagSharing.meanings_snoc, ← DagSharing.meanings_length reversed.reverse]
            exact DagSharing.getD_snoc_end _ _ _
          exact (exactFrozen reversed.reverse.length available).trans
            (congrArg (formulaValue outer) currentMeaning)
        simp only [maskedValues, List.foldl_append, List.foldl_cons, List.foldl_nil]
        change maskedValues tested frozen reversed.reverse ++
          [nodeValue tested (maskedValues tested frozen reversed.reverse) node &&
            frozen.getD (maskedValues tested frozen reversed.reverse).length false] = _
        rw [oldValues]
        simp only [List.length_map, DagSharing.meanings_length, currentMask,
          node_value_reduct, DagSharing.meanings_snoc, List.map_append,
          List.map_cons, List.map_nil]
    have completed := prefixInvariant table.reverse mask (by
      simpa only [List.reverse_reverse] using exactMask)
    exact (by simpa only [List.reverse_reverse] using completed)

  have computedMask (index : Nat) (_inside : index < table.length) :
      (TightEvaluation.values outer table).getD index false =
        formulaValue outer ((DagSharing.meanings table).getD index .bot) :=
    TightEvaluation.value_at outer table index
  exact masked_values_correspond table _ computedMask

/-- The frozen truth table contains exactly one value for every input node. -/
theorem values_length (outer tested : α → Bool) (table : List (DagSharing.Node α)) :
    (values outer tested table).length = table.length := by
  rw [values_correspond, List.length_map, DagSharing.meanings_length]

/-- Reading a computed node gives the truth of its explicit reduct. An
unavailable index has the common falsum meaning, as in the original DAG fold. -/
theorem value_at (outer tested : α → Bool) (table : List (DagSharing.Node α)) (index : Nat) :
    (values outer tested table).getD index false =
      formulaValue tested
        (Reduct (interpretation outer) ((DagSharing.meanings table).getD index .bot)) := by
  rw [values_correspond]
  simp only [List.getD_eq_getElem?_getD, List.getElem?_map]
  cases (DagSharing.meanings table)[index]? <;> rfl

/-- Checking all original root positions in the computed frozen table is
exactly satisfaction of their Ferraris reduct theory. Root order, repeated
roots and sharing do not alter the conjunction. The tested interpretation is
arbitrary; this is satisfaction, not a minimality or answer-set claim.

Each root lookup is exact by `value_at`. Soundness transfers each listed root
to its reduct; completeness takes the original root witnessing each reduct. -/
theorem roots_true_iff (outer tested : α → Bool) (table : List (DagSharing.Node α))
    (roots : List Nat) :
    rootsTrue (values outer tested table) roots = true ↔
      Models (interpretation tested)
        (ReductTheory (interpretation outer) (DagSharing.assertions table roots)) := by
  have rootExact (index : Nat) :
      (values outer tested table).getD index false = true ↔
        Satisfies (interpretation tested)
          (Reduct (interpretation outer) ((DagSharing.meanings table).getD index .bot)) := by
    rw [value_at]
    exact formula_value_true tested _
  simp only [rootsTrue, List.all_eq_true, rootExact, Models, ReductTheory,
    DagSharing.assertions, List.mem_map]
  constructor
  · intro checked formula represented
    obtain ⟨original, ⟨index, member, rfl⟩, rfl⟩ := represented
    exact checked index member
  · intro model index member
    exact model _ ⟨_, ⟨index, member, rfl⟩, rfl⟩

end Zetesis.ReductEvaluation
