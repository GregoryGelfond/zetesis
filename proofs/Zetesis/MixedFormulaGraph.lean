import Zetesis.NumericWitnessReplay
import Zetesis.OperandTable

/-!
# Original and frozen readings of a mixed formula graph

A successful pass starts at the raw node and descriptor indices of one immutable
table. Ordinary nodes use the operand arena. Numeric nodes select a row through
the appropriate descriptor table, validate its domain and borrow both columns.
Every child must precede the layer that reads it. The meanings below unfold each
node into an ordinary Ferraris formula; the evaluation does not construct those
formulas or enumerate the masks of a numeric row.

This is an executable mathematical model of admitted views. Its indices, sums
and ranks are mathematical integers. Machine arithmetic, owner authentication
and extraction from compiled Rust remain implementation obligations.
-/

namespace Zetesis.Refinement.MixedFormulaGraph

open Ferraris TightEvaluation
open NumericWitnessReplay (Row Guard)

variable {α : Type}

/-- Domain tag on one pair of numeric columns. -/
inductive Domain where
  | weights
  | ordinal (rankCount : Nat)
  deriving Repr

/-- Borrowed extent of a numeric row in both parallel columns. -/
structure Span where
  start : Nat
  size : Nat
  domain : Domain

/-- The cut uses the endpoint of its own ordinal carrier. -/
inductive Cut where
  | prefix (rank : Nat)
  | suffix (rank : Nat)
  deriving Repr

/-- All table lookups of this pass are relative to these same columns. -/
structure Columns where
  operands : List Nat
  conditions : List Nat
  values : List Nat
  rows : List Span
  thresholds : List (Nat × Int)
  cuts : List (Nat × Cut)

/-- Raw numeric nodes name a descriptor, not a row or a condition node. -/
inductive Node (α : Type) where
  | ordinary (node : OperandTable.Node α)
  | threshold (descriptor : Nat)
  | extremum (descriptor : Nat)

/-- One resolved layer, with its ordered occurrences still intact. -/
inductive Layer (α : Type) where
  | ordinary (node : OperandTable.Node α)
  | numeric (row : Row) (test : Guard)

/-- Resolve the descriptor and check both span bounds, the domain tag and
ordinal rank bounds. A missing or incompatible description declines the read. -/
def resolve (cols : Columns) : Node α → Option (Layer α)
  | .ordinary node => some (.ordinary node)
  | .threshold id => do
    let (rowId, bound) ← cols.thresholds[id]?
    let span ← cols.rows[rowId]?
    let .weights := span.domain | none
    let row ← NumericWitnessReplay.read cols.conditions cols.values span.start span.size
    return .numeric row (.threshold bound)
  | .extremum id => do
    let (rowId, cut) ← cols.cuts[id]?
    let span ← cols.rows[rowId]?
    let .ordinal ranks := span.domain | none
    let row ← NumericWitnessReplay.read cols.conditions cols.values span.start span.size
    let rank := match cut with | .prefix k | .suffix k => k
    guard (ranks ≥ 2 && rank ≤ ranks && (List.finRange row.size).all (fun i => row.value i < ranks))
    return .numeric row (match cut with
      | .prefix k => .prefix (ranks - 1) k
      | .suffix k => .suffix (ranks - 1) k)

/-- Validate every edge independently of whether it will affect truth. -/
def fits (arena : List Nat) (size : Nat) : Layer α → Bool
  | .ordinary (.atom _) | .ordinary .bot => true
  | .ordinary (.imp a b) => a < size && b < size
  | .ordinary (.group _ span) => (OperandArena.validate arena size span).isSome
  | .numeric row _ => (List.finRange row.size).all (fun i => row.condition i < size)

/-- Reference unfolding. The fallback for missing children is unreachable on
a layer whose edges have passed `fits`. -/
def meaning (arena : List Nat) (earlier : List (Formula α)) : Layer α → Formula α
  | .ordinary node => OperandTable.decode arena earlier node
  | .numeric row test => NumericWitnessReplay.meaning row test (fun i => earlier.getD i .bot)

/-- Read a single layer over predecessor truth. Masking with its original
reading is performed by the paired pass after both reads succeed. -/
def value (world : α → Bool) (arena : List Nat) (earlier : List Bool) : Layer α → Option Bool
  | .ordinary node => OperandTable.read world arena earlier node
  | .numeric row test => some (NumericWitnessReplay.accepts row test (fun i => earlier.getD (row.condition i) false))

/-- The checked edge test includes every requirement of the ordinary operand view. -/
theorem ordinary_fits {node : OperandTable.Node α} {arena : List Nat} {size : Nat}
    (checked : fits arena size (.ordinary node) = true) : OperandTable.Valid arena size node := by
  cases node with
  | atom | bot => trivial
  | imp a b => simpa [fits, OperandTable.Valid] using checked
  | group kind span =>
    obtain ⟨result, found⟩ := Option.isSome_iff_exists.mp checked
    exact ((OperandArena.validate_exact arena size span result).mp found).1

/-- A mapped prefix supplies the exact active mask of an admitted numeric row. -/
theorem row_bits (earlier : List (Formula α)) (row : Row) (arena : List Nat) (test : Guard)
    (checked : fits (α := α) arena earlier.length (.numeric row test) = true)
    (world : α → Bool) (transform : Formula α → Formula α) :
    (fun i => (earlier.map (fun f => formulaValue world (transform f))).getD (row.condition i) false) =
      AggregateReduct.active (interpretation world)
        (fun i => transform (earlier.getD (row.condition i) .bot)) := by
  funext i
  have inside : row.condition i < earlier.length := by
    have each := List.all_eq_true.mp checked i (List.mem_finRange i)
    exact of_decide_eq_true each
  apply Bool.eq_iff_iff.mpr
  simpa [List.getD_eq_getElem?_getD, List.getElem?_map, List.getElem?_eq_getElem inside,
    AggregateReduct.active] using formula_value_true world (transform earlier[row.condition i])

/-- Original and frozen child masks give the exact two readings of the
decoded layer. Numeric eligibility is gathered by occurrence and the whole
frozen guard is masked by the original bit, even for an empty row. -/
theorem layer_exact (outer tested : α → Bool) (arena : List Nat)
    (earlier : List (Formula α)) (layer : Layer α) (checked : fits arena earlier.length layer = true) :
    let f := meaning arena earlier layer
    value outer arena (earlier.map (formulaValue outer)) layer = some (formulaValue outer f) ∧
    (value tested arena (earlier.map (fun c => formulaValue tested (Reduct (interpretation outer) c))) layer).map
      (fun inner => inner && formulaValue outer f) =
      some (formulaValue tested (Reduct (interpretation outer) f)) := by
  cases layer with
  | ordinary node => exact ⟨OperandTable.read_original_exact outer arena earlier node (ordinary_fits checked),
      OperandTable.read_reduct_exact outer tested arena earlier node (ordinary_fits checked)⟩
  | numeric row test =>
    classical
    let children := fun i => earlier.getD i (.bot : Formula α)
    have original := AggregateReduct.original (interpretation outer) (fun i => children (row.condition i))
      (NumericWitnessReplay.accepts row test) _ (AggregateReduct.masks_complete _)
    have oldBits := row_bits earlier row arena test checked outer id
    simp only [id_eq] at oldBits
    have newBits := row_bits earlier row arena test checked tested (Reduct (interpretation outer))
    change some (NumericWitnessReplay.accepts row test _) = some _ ∧ some (_ && _) = some _
    constructor
    · apply congrArg some
      apply Bool.eq_iff_iff.mpr
      rw [oldBits]
      exact original.symm.trans (formula_value_true outer _).symm
    · apply congrArg some
      apply Bool.eq_iff_iff.mpr
      simp only [meaning]
      rw [Bool.and_eq_true, newBits, formula_value_true, formula_value_true,
        NumericWitnessReplay.frozen_read]
      exact and_comm

/-- Reference decoding refuses malformed descriptors and edges at their first
layer, and preserves the exact order of a shared table. -/
def decode (cols : Columns) (earlier : List (Formula α)) : List (Node α) → Option (List (Formula α))
  | [] => some earlier
  | raw :: rest => do
    let layer ← resolve cols raw
    guard (fits cols.operands earlier.length layer)
    decode cols (earlier ++ [meaning cols.operands earlier layer]) rest

/-- One topological visit for each mixed node. Both interpretations may be
arbitrary. Original and frozen bit positions use the same immutable table. -/
def evaluate (outer tested : α → Bool) (cols : Columns) (earlier : OperandTable.Truth) :
    List (Node α) → Option OperandTable.Truth
  | [] => some earlier
  | raw :: rest => do
    let layer ← resolve cols raw
    guard (fits cols.operands earlier.original.length layer)
    let original ← value outer cols.operands earlier.original layer
    let inner ← value tested cols.operands earlier.reduct layer
    evaluate outer tested cols ⟨earlier.original ++ [original], earlier.reduct ++ [inner && original]⟩ rest

/-- Interpret a decoded prefix in both worlds. -/
noncomputable def readings (outer tested : α → Bool) (earlier : List (Formula α)) : OperandTable.Truth :=
  ⟨earlier.map (formulaValue outer), earlier.map (fun f => formulaValue tested (Reduct (interpretation outer) f))⟩

/-- For every raw mixed graph, from any correctly interpreted prefix, the
checked pass either fails decoding or computes exactly the two complete
Ferraris truth vectors. Induct on the remaining nodes; the layer theorem
extends the invariant by one position. Starting at the empty prefix removes
any assumption about a precomputed original mask. -/
theorem evaluate_exact (outer tested : α → Bool) (cols : Columns) (nodes : List (Node α))
    (earlier : List (Formula α)) :
    evaluate outer tested cols (readings outer tested earlier) nodes =
      (decode cols earlier nodes).map (readings outer tested) := by
  induction nodes generalizing earlier with
  | nil => rfl
  | cons raw rest advance =>
    cases found : resolve cols raw with
    | none => simp [evaluate, decode, found]
    | some layer =>
      by_cases checked : fits cols.operands earlier.length layer = true
      · obtain ⟨old, frozen⟩ := layer_exact outer tested cols.operands earlier layer checked
        cases got : value tested cols.operands (earlier.map (fun c => formulaValue tested (Reduct (interpretation outer) c))) layer with
        | none => rw [show value tested cols.operands _ layer = none from got] at frozen; contradiction
        | some inner =>
          rw [show value tested cols.operands _ layer = some inner from got] at frozen
          simp only [Option.map_some, Option.some.injEq] at frozen
          simpa [evaluate, decode, found, checked, readings, old, got, frozen, guard] using
            advance (earlier ++ [meaning cols.operands earlier layer])
      · simp [evaluate, decode, found, readings, checked, guard]
        rfl

/-- A complete root borrow preserves order and repetitions. Reading those
same indices from the value column gives the conjunction of their values. -/
theorem root_column_exact (formulas : List (Formula α)) (roots : List Nat) (truth : Formula α → Bool)
    {theory : Theory α} (found : OperandArena.gather formulas roots = some theory) :
    IndexedEvaluation.roots (formulas.map truth) roots = some (theory.all truth) := by
  induction roots generalizing theory with
  | nil => simp [OperandArena.gather] at found; subst theory; rfl
  | cons first rest next =>
    cases entry : formulas[first]? with
    | none => simp [OperandArena.gather, entry] at found
    | some formula =>
      cases tail : OperandArena.gather formulas rest with
      | none => simp [OperandArena.gather, entry, tail] at found
      | some members =>
        simp [OperandArena.gather, entry, tail] at found
        subst theory
        cases value : truth formula <;>
          simp [IndexedEvaluation.roots, List.getElem?_map, entry, value, next tail]

/-- The asserted frozen roots of an admitted mixed table are true exactly
when the tested interpretation models the frozen Ferraris theory. Both the
complete node vector and the root identities come from one graph. -/
theorem frozen_roots_exact (outer tested : α → Bool) (cols : Columns) (nodes : List (Node α))
    (roots : List Nat) {formulas theory : List (Formula α)} (decoded : decode cols [] nodes = some formulas)
    (asserted : OperandArena.gather formulas roots = some theory) :
    ((evaluate outer tested cols ⟨[], []⟩ nodes).bind fun table => IndexedEvaluation.roots table.reduct roots)
      = some true ↔ Models (interpretation tested) (ReductTheory (interpretation outer) theory) := by
  have evaluated := evaluate_exact outer tested cols nodes []
  simp [decoded] at evaluated
  change evaluate outer tested cols (readings outer tested []) nodes = _ at evaluated
  change ((evaluate outer tested cols (readings outer tested []) nodes).bind _) = _ ↔ _
  rw [evaluated]
  simp only [Option.bind_some, readings]
  rw [root_column_exact _ _ _ asserted]
  simp [Models, ReductTheory, List.all_eq_true, formula_value_true]

/-- Indexed predecessor reads in operand order; failure withholds the layer. -/
def gatherBits (bit : Nat → Option Bool) : List Nat → Option (List Bool)
  | [] => some []
  | i :: rest => do return (← bit i) :: (← gatherBits bit rest)

/-- Indexed gathering through a list preserves the ordered operand read. -/
theorem gather_bits_list (bits : List Bool) (ids : List Nat) :
    gatherBits (fun i => bits[i]?) ids = OperandArena.gather bits ids := by
  induction ids with
  | nil => rfl
  | cons i rest next => simp [gatherBits, OperandArena.gather, next]

/-- The runtime keeps truth in indexed columns. This read takes their size and
lookup without rebuilding a list for each node. -/
def indexedValue (world : α → Bool) (arena : List Nat) (size : Nat)
    (bit : Nat → Option Bool) : Layer α → Option Bool
  | .ordinary (.atom a) => some (world a)
  | .ordinary .bot => some false
  | .ordinary (.imp a b) => do return !(← bit a) || (← bit b)
  | .ordinary (.group kind span) => do
    let ids ← OperandArena.validate arena size span
    return OperandArena.reduce kind (← gatherBits bit ids)
  | .numeric row test => some (NumericWitnessReplay.accepts row test (fun i => (bit (row.condition i)).getD false))

/-- The indexed single-layer reader agrees with the list-level reader on the same input. -/
theorem indexed_list (world : α → Bool) (arena : List Nat) (bits : List Bool) (layer : Layer α) :
    indexedValue world arena bits.length (fun i => bits[i]?) layer = value world arena bits layer := by
  cases layer with
  | numeric row test => simp [value, indexedValue, List.getD_eq_getElem?_getD]
  | ordinary node =>
    cases node <;> simp [value, indexedValue, OperandTable.read, OperandArena.evaluate, gather_bits_list]

/-- Two independently appendable truth columns. -/
structure ColumnsTruth where
  original : Array Bool
  reduct : Array Bool

/-- Expose the two arrays to the list-level Ferraris correspondence. -/
def ColumnsTruth.view (bits : ColumnsTruth) : OperandTable.Truth :=
  ⟨bits.original.toList, bits.reduct.toList⟩

/-- Executable checked pass. At each stage both arrays already contain exactly
the preceding nodes. Each successor appends one original and one masked bit. -/
def run (outer tested : α → Bool) (cols : Columns) (bits : ColumnsTruth) :
    List (Node α) → Option ColumnsTruth
  | [] => some bits
  | raw :: rest => do
    let layer ← resolve cols raw
    guard (fits cols.operands bits.original.size layer)
    let orig ← indexedValue outer cols.operands bits.original.size (fun i => bits.original[i]?) layer
    let inner ← indexedValue tested cols.operands bits.reduct.size (fun i => bits.reduct[i]?) layer
    run outer tested cols ⟨bits.original.push orig, bits.reduct.push (inner && orig)⟩ rest

/-- An array lookup agrees with the reference reading of its list view. -/
theorem indexed_array (world : α → Bool) (arena : List Nat) (bits : Array Bool) (layer : Layer α) :
    indexedValue world arena bits.size (fun i => bits[i]?) layer = value world arena bits.toList layer := by
  simpa only [Array.length_toList, Array.getElem?_toList] using indexed_list world arena bits.toList layer

/-- Indexed append and checked reads produce the same accepted or refused
trace as the reference table, for every starting array and suffix. -/
theorem run_refines_evaluate (outer tested : α → Bool) (cols : Columns)
    (nodes : List (Node α)) (bits : ColumnsTruth) :
    (run outer tested cols bits nodes).map ColumnsTruth.view = evaluate outer tested cols bits.view nodes := by
  induction nodes generalizing bits with
  | nil => rfl
  | cons raw rest advance =>
    cases resolved : resolve cols raw with
    | none => simp [run, evaluate, resolved]
    | some layer =>
      by_cases valid : fits cols.operands bits.original.size layer = true
      · cases orig : value outer cols.operands bits.original.toList layer with
        | none => simp [run, evaluate, resolved, ColumnsTruth.view, valid, guard, indexed_array, orig]
        | some first =>
          cases frozen : value tested cols.operands bits.reduct.toList layer with
          | none => simp [run, evaluate, resolved, ColumnsTruth.view, valid, guard, indexed_array, orig, frozen]
          | some second =>
            simpa [run, evaluate, resolved, ColumnsTruth.view, valid, guard, indexed_array, orig, frozen]
              using advance ⟨bits.original.push first, bits.reduct.push (second && first)⟩
      · simp [run, evaluate, resolved, ColumnsTruth.view, valid, guard]; rfl

end Zetesis.Refinement.MixedFormulaGraph
