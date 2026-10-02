import Zetesis.ReductEvaluation

/-!
# Admitted indices in formula evaluation

A formula table is evaluated through checked child and mask reads. An absent
entry returns `none`; it is never interpreted as falsum. For an admitted DAG
and a covering frozen mask, every read succeeds and the completed values equal
the existing original or reduct evaluator. Root checks have the same boundary.

The proof follows the initialized prefix: its length is the number of processed
nodes, every child names that prefix, and the mask covers the next node. This
establishes index safety in the authored Lean algorithm without assuming the
evaluator's result. Rust extraction, machine arithmetic, atom storage, resource
stops and ownership remain separate obligations. The model does not prescribe
error precedence for malformed tables or count short-circuited Boolean reads.
-/

namespace Zetesis.Refinement.IndexedEvaluation

universe u
variable {α : Type u}

/-- Evaluate a node using only present child values. -/
def node (candidate : α → Bool) (earlier : List Bool) : DagSharing.Node α → Option Bool
  | .atom atom => some (candidate atom)
  | .bot => some false
  | .conj left right => do return (← earlier[left]?) && (← earlier[right]?)
  | .disj left right => do return (← earlier[left]?) || (← earlier[right]?)
  | .imp left right => do return !(← earlier[left]?) || (← earlier[right]?)

/-- Apply a present frozen bit; an unmasked pass leaves the value unchanged. -/
def applyMask (mask : Option (List Bool)) (index : Nat) (value : Bool) : Option Bool :=
  match mask with
  | none => some value
  | some frozen => (frozen[index]?).map (value && ·)

/-- Append one value after its child and optional mask reads succeed. -/
def step (candidate : α → Bool) (mask : Option (List Bool))
    (earlier : List Bool) (entry : DagSharing.Node α) : Option (List Bool) := do
  let value ← node candidate earlier entry
  let masked ← applyMask mask earlier.length value
  return earlier ++ [masked]

/-- Process nodes in construction order, stopping at an absent indexed value. -/
def evaluate (candidate : α → Bool) (mask : Option (List Bool))
    (table : List (DagSharing.Node α)) : Option (List Bool) :=
  table.foldlM (step candidate mask) []

private def referenceMask (mask : Option (List Bool)) (index : Nat) (value : Bool) : Bool :=
  mask.elim value (fun frozen => value && frozen.getD index false)

private def reference (candidate : α → Bool) (mask : Option (List Bool))
    (table : List (DagSharing.Node α)) : List Bool :=
  table.foldl (fun earlier entry => earlier ++
    [referenceMask mask earlier.length (TightEvaluation.nodeValue candidate earlier entry)]) []

/-- Admitted child indices make the checked operation equal the total
mathematical node evaluator. In particular, no default child value is used. -/
theorem node_exact (candidate : α → Bool) (earlier : List Bool)
    (entry : DagSharing.Node α) (admitted : DagSharing.ValidNode earlier.length entry) :
    node candidate earlier entry = some (TightEvaluation.nodeValue candidate earlier entry) := by
  cases entry with
  | atom atom => rfl
  | bot => rfl
  | conj left right | disj left right | imp left right =>
    simp only [DagSharing.ValidNode] at admitted
    simp [node, TightEvaluation.nodeValue, List.getD_eq_getElem?_getD,
      List.getElem?_eq_getElem admitted.1, List.getElem?_eq_getElem admitted.2]

/-- On an admitted DAG, checked evaluation completes and equals the total
reference fold. The optional mask must cover the complete table.

The induction appends one admitted node. The previous completed prefix has
exactly the previous table's length, so child admission gives actual stored
values. Mask coverage supplies the current frozen bit. Their successful step
extends the same prefix in both computations.
-/
theorem evaluate_exact (candidate : α → Bool) (mask : Option (List Bool))
    (table : List (DagSharing.Node α)) (admitted : DagSharing.WellFormed table)
    (covered : ∀ frozen, mask = some frozen → table.length ≤ frozen.length) :
    evaluate candidate mask table = some (table.foldl (fun earlier entry =>
      earlier ++ [mask.elim (TightEvaluation.nodeValue candidate earlier entry)
        (fun frozen => TightEvaluation.nodeValue candidate earlier entry &&
          frozen.getD earlier.length false)]) []) := by
  change evaluate candidate mask table = some (reference candidate mask table)
  have referenceLength (mask : Option (List Bool)) (table : List (DagSharing.Node α)) :
      (reference candidate mask table).length = table.length := by
    have prefixLength (remaining : List (DagSharing.Node α)) (earlier : List Bool) :
        (remaining.foldl (fun truth entry => truth ++
          [referenceMask mask truth.length (TightEvaluation.nodeValue candidate truth entry)])
          earlier).length = earlier.length + remaining.length := by
      induction remaining generalizing earlier with
      | nil => simp
      | cons entry rest inductionHypothesis =>
        simp only [List.foldl_cons, inductionHypothesis, List.length_append,
          List.length_cons, List.length_nil]
        omega
    simpa [reference] using prefixLength table []
  induction admitted with
  | nil => rfl
  | @snoc table entry admitted valid inductionHypothesis =>
    have oldCoverage : ∀ frozen, mask = some frozen → table.length ≤ frozen.length := by
      intro frozen same
      have full := covered frozen same
      simp only [List.length_append, List.length_singleton] at full
      omega
    have oldExact := inductionHypothesis oldCoverage
    have prefixLength := referenceLength mask table
    have childReads := node_exact candidate (reference candidate mask table) entry
      (by simpa only [prefixLength] using valid)
    have maskRead : applyMask mask (reference candidate mask table).length
        (TightEvaluation.nodeValue candidate (reference candidate mask table) entry) =
        some (referenceMask mask (reference candidate mask table).length
          (TightEvaluation.nodeValue candidate (reference candidate mask table) entry)) := by
      cases mask with
      | none => rfl
      | some frozen =>
        have inside : (reference candidate (some frozen) table).length < frozen.length := by
          have full := covered frozen rfl
          simp only [List.length_append, List.length_singleton] at full
          rw [prefixLength]
          omega
        simp [applyMask, referenceMask, List.getD_eq_getElem?_getD,
          List.getElem?_eq_getElem inside]
    simp only [evaluate, List.foldlM_append] at oldExact ⊢
    rw [oldExact]
    simp only [List.foldlM_cons, List.foldlM_nil, bind_pure]
    change step candidate mask (reference candidate mask table) entry = _
    have referenceStep : reference candidate mask (table ++ [entry]) =
        reference candidate mask table ++
          [referenceMask mask (reference candidate mask table).length
            (TightEvaluation.nodeValue candidate (reference candidate mask table) entry)] := by
      simp only [reference, List.foldl_append, List.foldl_cons, List.foldl_nil]
    rw [referenceStep]
    simp [step, childReads, maskRead]

/-- Original evaluation of an admitted table succeeds without using defaults. -/
theorem original_exact (candidate : α → Bool) (table : List (DagSharing.Node α))
    (admitted : DagSharing.WellFormed table) :
    evaluate candidate none table = some (TightEvaluation.values candidate table) := by
  simpa [TightEvaluation.values] using
    evaluate_exact candidate none table admitted (by simp)

/-- A computed original mask covers every node. Checked evaluation with that
mask therefore computes the frozen reduct, for any tested interpretation. -/
theorem reduct_exact (outer tested : α → Bool) (table : List (DagSharing.Node α))
    (admitted : DagSharing.WellFormed table) :
    evaluate tested (some (TightEvaluation.values outer table)) table =
      some (ReductEvaluation.values outer tested table) := by
  have maskLength : (TightEvaluation.values outer table).length = table.length := by
    rw [TightEvaluation.values_correspond, List.length_map, DagSharing.meanings_length]
  have result := evaluate_exact tested (some (TightEvaluation.values outer table)) table
    admitted (by intro frozen same; cases same; exact Nat.le_of_eq maskLength.symm)
  exact result

/-- Test roots in their supplied order. A false root ends the conjunction;
an absent root before that point is incomplete evaluation, not falsehood. -/
def roots (truth : List Bool) : List Nat → Option Bool
  | [] => some true
  | root :: rest => do
    let value ← truth[root]?
    if value then roots truth rest else some false

/-- When every root exists, the checked root scan is the ordinary conjunction.
The proof follows the scan and preserves its early false result. -/
theorem roots_exact (truth : List Bool) (assertions : List Nat)
    (admitted : ∀ root ∈ assertions, root < truth.length) :
    roots truth assertions = some (TightEvaluation.rootsTrue truth assertions) := by
  induction assertions with
  | nil => rfl
  | cons root rest inductionHypothesis =>
    have inside := admitted root (by simp)
    have tailAdmitted : ∀ index ∈ rest, index < truth.length := by
      intro index member
      exact admitted index (by simp [member])
    simp only [roots, List.getElem?_eq_getElem inside]
    rw [inductionHypothesis tailAdmitted]
    simp only [TightEvaluation.rootsTrue, List.all_cons, List.getD_eq_getElem?_getD,
      List.getElem?_eq_getElem inside, Option.getD_some]
    cases truth[root] <;> rfl

/-- Compute the frozen reduct and check its asserted roots. An absent entry
actually read is represented by `none`, distinct from a completed negative
result. A false root stops the scan before any later root is read. -/
def satisfiesReduct (outer tested : α → Bool) (table : List (DagSharing.Node α))
    (assertions : List Nat) : Option Bool := do
  let frozen ← evaluate outer none table
  let truth ← evaluate tested (some frozen) table
  roots truth assertions

/-- The two checked passes and root scan complete on admitted input. Both
positive and negative truth results are preserved; neither is `none`. -/
theorem satisfies_reduct_exact (outer tested : α → Bool)
    (table : List (DagSharing.Node α)) (assertions : List Nat)
    (admitted : DagSharing.WellFormed table)
    (rootBounds : ∀ root ∈ assertions, root < table.length) :
    satisfiesReduct outer tested table assertions =
      some (TightEvaluation.rootsTrue (ReductEvaluation.values outer tested table) assertions) := by
  have checkedRoots := roots_exact (ReductEvaluation.values outer tested table) assertions
    (by simpa only [ReductEvaluation.values_length] using rootBounds)
  unfold satisfiesReduct
  rw [original_exact outer table admitted]
  change (evaluate tested (some (TightEvaluation.values outer table)) table).bind
    (fun truth => roots truth assertions) = _
  rw [reduct_exact outer tested table admitted]
  exact checkedRoots

/-- With admitted children and roots, the checked computation returns true
exactly for a model of the frozen Ferraris reduct. The computed-mask theorem
supplies the values; their proved length transports the admitted root bounds. -/
theorem satisfies_reduct_iff (outer tested : α → Bool)
    (table : List (DagSharing.Node α)) (assertions : List Nat)
    (admitted : DagSharing.WellFormed table)
    (rootBounds : ∀ root ∈ assertions, root < table.length) :
    satisfiesReduct outer tested table assertions = some true ↔
      Ferraris.Models (TightEvaluation.interpretation tested)
        (Ferraris.ReductTheory (TightEvaluation.interpretation outer)
          (DagSharing.assertions table assertions)) := by
  rw [satisfies_reduct_exact outer tested table assertions admitted rootBounds,
    Option.some.injEq]
  exact ReductEvaluation.roots_true_iff outer tested table assertions

end Zetesis.Refinement.IndexedEvaluation
