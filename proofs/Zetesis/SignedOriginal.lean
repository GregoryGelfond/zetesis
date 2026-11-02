import Zetesis.MixedFormulaGraph
import Zetesis.StableProducerKnowledge

/-!
# Signed original node identities

Resolving a signed original ID on a single
immutable mixed graph binds both the coordinate and polarity. The original and frozen columns come from the
indexed pass. A successful indexed node read is related to the exact prefix
and descriptor of the raw source node in its original position. -/
namespace Zetesis.Refinement.SignedOriginal
open Ferraris TightEvaluation MixedFormulaGraph NumericWitnessReplay LearnedRegions
variable {α : Type}

/-- IDs and signs are both required. An out-of-range slot never matches false. -/
structure Key where
  id : Nat
  sign : Bool
  deriving Repr

/-- Both the position and the expected Boolean must be present in the original column. -/
def hasSign (truth : Array Bool) (k : Key) : Bool := truth[k.id]? == some k.sign

/-- Combine the indexed and denotational passes for the same successfully decoded owner. -/
theorem run_columns {outer tested : α → Bool} {cols : Columns} {nodes : List (Node α)}
    {fs : List (Formula α)} {got : ColumnsTruth}
    (decoded : decode cols [] nodes = some fs)
    (ran : run outer tested cols ⟨#[],#[]⟩ nodes = some got) :
    got.view = readings outer tested fs := by
  have exactRun := run_refines_evaluate outer tested cols nodes ⟨#[],#[]⟩
  have exactFormula := evaluate_exact outer tested cols nodes []
  rw [decoded] at exactFormula
  rw [ran] at exactRun
  exact Option.some.inj (exactRun.trans exactFormula)

/-- A successful signed match names an existing decoded formula with that original truth. -/
theorem signed_id_exact {outer tested : α → Bool} {cols : Columns} {nodes : List (Node α)}
    {fs : List (Formula α)} {got : ColumnsTruth}
    (decoded : decode cols [] nodes = some fs)
    (ran : run outer tested cols ⟨#[],#[]⟩ nodes = some got) (k : Key) :
    hasSign got.original k = true ↔ ∃ f, fs[k.id]? = some f ∧ formulaValue outer f = k.sign := by
  have original := congrArg OperandTable.Truth.original (run_columns decoded ran)
  change got.original.toList = fs.map (formulaValue outer) at original
  simp only [hasSign, beq_iff_eq, ← Array.getElem?_toList, original, List.getElem?_map]
  cases found : fs[k.id]? <;> simp

/-- The frozen read at a valid ID names precisely the reduct of the formula stored there. -/
theorem frozen_id {outer tested : α → Bool} {cols : Columns} {nodes : List (Node α)}
    {fs : List (Formula α)} {got : ColumnsTruth}
    (decoded : decode cols [] nodes = some fs)
    (ran : run outer tested cols ⟨#[],#[]⟩ nodes = some got) (i : Nat) :
    got.reduct[i]? = fs[i]?.map (fun f => formulaValue tested (Reduct (interpretation outer) f)) := by
  have frozen := congrArg OperandTable.Truth.reduct (run_columns decoded ran)
  change got.reduct.toList = fs.map (fun f => formulaValue tested (Reduct (interpretation outer) f)) at frozen
  rw [← Array.getElem?_toList, frozen, List.getElem?_map]

/-- Decoding is one ordered schedule, so the cut just before a named node
recovers its actual accepted predecessor state. -/
theorem decode_append (cols : Columns) (earlier : List (Formula α)) (left right : List (Node α)) :
    decode cols earlier (left ++ right) = (decode cols earlier left).bind (fun p => decode cols p right) := by
  induction left generalizing earlier with
  | nil => rfl
  | cons raw rest inductionHypothesis =>
    cases found : resolve cols raw with
    | none => simp [decode, found]
    | some layer =>
      by_cases fit : fits cols.operands earlier.length layer = true
      · simp [decode, found, fit, guard, inductionHypothesis]
      · simp [decode, found, fit, guard]; rfl

/-- Successful decoding appends one entry for every raw node and preserves the earlier entries. -/
theorem decode_extends (cols : Columns) (earlier : List (Formula α)) (nodes : List (Node α))
    {fs : List (Formula α)} (done : decode cols earlier nodes = some fs) :
    ∃ tail, fs = earlier ++ tail ∧ tail.length = nodes.length := by
  induction nodes generalizing earlier with
  | nil => exact ⟨[], by simpa [decode] using (Option.some.inj done).symm, rfl⟩
  | cons raw rest advance =>
    cases found : resolve cols raw with
    | none => simp [decode, found] at done
    | some layer =>
      by_cases fit : fits cols.operands earlier.length layer = true
      · simp [decode, found, fit, guard] at done
        obtain ⟨tail, same, size⟩ := advance _ done
        exact ⟨meaning cols.operands earlier layer :: tail, by simpa using same, by simp [size]⟩
      · simp [decode, found, fit, guard] at done
        contradiction

/-- The resolved layer at the actual raw ordinal denotes exactly the stored
formula there. All its child coordinates refer to the preceding prefix. -/
theorem at_ordinal {cols : Columns} (before : List (Node α)) (raw : Node α) (after : List (Node α))
    {fs : List (Formula α)} (done : decode cols [] (before ++ raw :: after) = some fs) :
    ∃ earlier layer suffix,
      resolve cols raw = some layer ∧ fits cols.operands earlier.length layer = true ∧
      earlier.length = before.length ∧ fs = earlier ++ meaning cols.operands earlier layer :: suffix := by
  rw [decode_append] at done
  cases prefixRead : decode cols [] before with
  | none => simp [prefixRead] at done
  | some earlier =>
    simp [prefixRead] at done
    obtain ⟨prefixTail, eq, len⟩ := decode_extends cols [] before prefixRead
    have position : earlier.length = before.length := by simpa [eq] using len
    cases got : resolve cols raw with
    | none => simp [decode, got] at done
    | some layer =>
      by_cases fit : fits cols.operands earlier.length layer = true
      · simp [decode, got, fit, guard] at done
        obtain ⟨tail, eq, _⟩ := decode_extends cols _ after done
        exact ⟨earlier, layer, tail, rfl, fit, position, by simpa using eq⟩
      · simp [decode, got, fit, guard] at done
        contradiction

/-- The same signed guard occurrence in two reads of this particular graph
transfers a numeric node's frozen result. For a live guard it suffices to
transfer the frozen readings of its actual incoming condition IDs. A masked
guard needs none. Child agreement is the subproblem for a preceding layer,
not an assumption about the root's own frozen result. -/
theorem numeric_id_replay (before : List (Node α)) (raw : Node α) (after : List (Node α))
    (cols : Columns) {fs : List (Formula α)} (decoded : decode cols [] (before ++ raw :: after) = some fs)
    (old new tested : α → Bool) {left right : ColumnsTruth}
    (oldRun : run old tested cols ⟨#[],#[]⟩ (before ++ raw :: after) = some left)
    (newRun : run new tested cols ⟨#[],#[]⟩ (before ++ raw :: after) = some right)
    (row : Row) (test : Guard) (source : resolve cols raw = some (.numeric row test))
    (sign : Bool) (oldMask : hasSign left.original ⟨before.length, sign⟩ = true)
    (newMask : hasSign right.original ⟨before.length, sign⟩ = true)
    (children : sign = true → ∀ i, left.reduct[row.condition i]? = right.reduct[row.condition i]?) :
    left.reduct[before.length]? = right.reduct[before.length]? := by
  classical
  obtain ⟨earlier, layer, tail, found, fit, position, table⟩ := at_ordinal before raw after decoded
  rw [source] at found
  cases found
  let child := fun i => earlier.getD i (.bot : Formula α)
  let root := NumericWitnessReplay.meaning row test child
  have entry : fs[before.length]? = some root := by
    simp [← position, table, MixedFormulaGraph.meaning, root, child]
  obtain ⟨F, foundOld, oldSign⟩ := (signed_id_exact decoded oldRun _).mp oldMask
  obtain ⟨G, foundNew, newSign⟩ := (signed_id_exact decoded newRun _).mp newMask
  change fs[before.length]? = some F at foundOld
  change fs[before.length]? = some G at foundNew
  rw [entry] at foundOld foundNew
  cases foundOld; cases foundNew
  have guardAgreement : Satisfies (interpretation old) root ↔ Satisfies (interpretation new) root := by
    rw [← formula_value_true, ← formula_value_true, oldSign, newSign]
  have inputs (live : Satisfies (interpretation old) root) :
      AggregateReduct.active (interpretation tested) (fun i => Reduct (interpretation old) (child (row.condition i))) =
      AggregateReduct.active (interpretation tested) (fun i => Reduct (interpretation new) (child (row.condition i))) := by
    funext i
    have active : sign = true := oldSign.symm.trans ((formula_value_true old _).mpr live)
    have transfer := children active i
    have inside : row.condition i < earlier.length := of_decide_eq_true (List.all_eq_true.mp fit i (List.mem_finRange i))
    have lookup : fs[row.condition i]? = some (child (row.condition i)) := by
      simp [table, List.getElem?_append_left inside, child, List.getD_eq_getElem?_getD, List.getElem?_eq_getElem inside]
    rw [frozen_id decoded oldRun, frozen_id decoded newRun, lookup] at transfer
    simp only [Option.map_some, Option.some.injEq] at transfer
    apply Bool.eq_iff_iff.mpr
    simpa [AggregateReduct.active, ← formula_value_true] using Bool.eq_iff_iff.mp transfer
  have survives : Satisfies (interpretation tested) (Reduct (interpretation old) root) ↔
      Satisfies (interpretation tested) (Reduct (interpretation new) root) := by
    rw [NumericWitnessReplay.frozen_read, NumericWitnessReplay.frozen_read]
    by_cases live : Satisfies (interpretation old) root
    · rw [inputs live]
      exact and_congr_left' guardAgreement
    · exact ⟨fun h => (live h.1).elim, fun h => (live (guardAgreement.mpr h.1)).elim⟩
  rw [frozen_id decoded oldRun, frozen_id decoded newRun, entry]
  simpa [formula_value_true] using congrArg some (Bool.eq_iff_iff.mpr
    (((formula_value_true tested _).trans survives).trans (formula_value_true tested _).symm))

end Zetesis.Refinement.SignedOriginal
