import Zetesis.DagSharing
import Zetesis.CertifiedExecution

/-!
# Boolean DAG evaluation and indexed producer support

A topological Boolean fold computes the original truth of the existing finite
DagSharing representation. Indexed producer bodies read that same truth table;
an absent body explicitly denotes truth. Head support is a Boolean OR reduction,
and the finite candidate scan requires support for every present semantic atom.
These executable mathematical operations compose with ranked-support stability
and exact completion of residual verdicts.

The representation correspondence is proved from the operations, not assumed as
an evaluator premise. The producer links are syntactic equalities to the checked
producer grammar. Complete carrier coverage, original producer membership and
the strict positive rank remain explicit certificate obligations.

DagSharing's total decoding assigns falsum to an unavailable reference. The
Boolean fold uses the matching false totalization; this is not permission for
an implementation to read outside an admitted table. Rust extraction, index and
identity validation, source coverage, packed words, atomics, barriers, readback,
work/memory bounds and device execution are not verified here. The list fold is
a finite specification, not an array-complexity or GPU performance model.
-/

namespace Zetesis.TightEvaluation
universe u
variable {α : Type u}
open Ferraris TightPlans

/-- A total Boolean candidate denotes its true semantic atoms. -/
def interpretation (candidate : α → Bool) : Atoms α := fun atom => candidate atom = true

/-- Original formula truth, including material implication before any reduct. -/
def formulaValue (candidate : α → Bool) : Formula α → Bool
  | .atom atom => candidate atom
  | .bot => false
  | .conj left right => formulaValue candidate left && formulaValue candidate right
  | .disj left right => formulaValue candidate left || formulaValue candidate right
  | .imp left right => !formulaValue candidate left || formulaValue candidate right

/-- The Boolean tree evaluator denotes exactly original Ferraris satisfaction. -/
theorem formula_value_true (candidate : α → Bool) (formula : Formula α) :
    formulaValue candidate formula = true ↔ Satisfies (interpretation candidate) formula := by
  induction formula with
  | atom atom => rfl
  | bot => simp [formulaValue, Satisfies]
  | conj left right leftExact rightExact =>
    simp [formulaValue, Satisfies, leftExact, rightExact]
  | disj left right leftExact rightExact =>
    simp [formulaValue, Satisfies, leftExact, rightExact]
  | imp left right leftExact rightExact =>
    cases leftTruth : formulaValue candidate left <;>
      cases rightTruth : formulaValue candidate right <;>
      simp_all [formulaValue, Satisfies]

/-- Evaluate one DAG node against only the already computed prefix. -/
def nodeValue (candidate : α → Bool) (earlier : List Bool) : DagSharing.Node α → Bool
  | .atom atom => candidate atom
  | .bot => false
  | .conj left right => earlier.getD left false && earlier.getD right false
  | .disj left right => earlier.getD left false || earlier.getD right false
  | .imp left right => !earlier.getD left false || earlier.getD right false

/-- The truth prefix grows in the same order as the admitted node table. -/
def values (candidate : α → Bool) (table : List (DagSharing.Node α)) : List Bool :=
  table.foldl (fun earlier node => earlier ++ [nodeValue candidate earlier node]) []

/-- One computed node agrees with decoding its children into original formulas. -/
theorem node_value_decode (candidate : α → Bool) (earlier : List (Formula α))
    (node : DagSharing.Node α) :
    nodeValue candidate (earlier.map (formulaValue candidate)) node =
      formulaValue candidate (DagSharing.decode earlier node) := by
  have lookup (index : Nat) :
      (earlier.map (formulaValue candidate)).getD index false =
        formulaValue candidate (earlier.getD index .bot) := by
    simp only [List.getD_eq_getElem?_getD, List.getElem?_map]
    cases earlier[index]? <;> rfl
  cases node with
  | atom atom => rfl
  | bot => rfl
  | conj left right => simp only [nodeValue, DagSharing.decode, formulaValue, lookup]
  | disj left right => simp only [nodeValue, DagSharing.decode, formulaValue, lookup]
  | imp left right => simp only [nodeValue, DagSharing.decode, formulaValue, lookup]

/-- The actual Boolean fold constructs a pointwise exact original-truth table.
The induction preserves the whole computed prefix, including shared child uses. -/
theorem values_correspond (candidate : α → Bool) (table : List (DagSharing.Node α)) :
    values candidate table = (DagSharing.meanings table).map (formulaValue candidate) := by
  have prefixInvariant (remaining : List (DagSharing.Node α))
      (earlier : List (Formula α)) :
      remaining.foldl (fun truth node => truth ++ [nodeValue candidate truth node])
          (earlier.map (formulaValue candidate)) =
        (remaining.foldl (fun meanings node => meanings ++ [DagSharing.decode meanings node])
          earlier).map (formulaValue candidate) := by
    induction remaining generalizing earlier with
    | nil => rfl
    | cons node tail inductionHypothesis =>
      simp only [List.foldl_cons, node_value_decode]
      simpa only [List.map_append, List.map_cons, List.map_nil] using
        inductionHypothesis (earlier ++ [DagSharing.decode earlier node])
  exact prefixInvariant table []

/-- A table lookup therefore computes the original truth of its unfolded node. -/
theorem value_at (candidate : α → Bool) (table : List (DagSharing.Node α)) (index : Nat) :
    (values candidate table).getD index false =
      formulaValue candidate ((DagSharing.meanings table).getD index .bot) := by
  rw [values_correspond]
  simp only [List.getD_eq_getElem?_getD, List.getElem?_map]
  cases (DagSharing.meanings table)[index]? <;> rfl

/-- Every original root is checked; unasserted table entries add no constraint. -/
def rootsTrue (truth : List Bool) (roots : List Nat) : Bool :=
  roots.all (fun index => truth.getD index false)

theorem roots_true_iff (candidate : α → Bool) (table : List (DagSharing.Node α))
    (roots : List Nat) :
    rootsTrue (values candidate table) roots = true ↔
      Models (interpretation candidate) (DagSharing.assertions table roots) := by
  simp only [rootsTrue, List.all_eq_true, value_at, formula_value_true,
    Models, DagSharing.assertions, List.mem_map]
  constructor
  · intro checked formula represented
    obtain ⟨index, member, rfl⟩ := represented
    exact checked index member
  · intro model index member
    exact model _ ⟨index, member, rfl⟩

/-- Body absence is an explicit fact marker, not a borrowed node identifier. -/
structure IndexedProducer (α : Type u) where
  head : α
  body : Option Nat

def bodyValue (truth : List Bool) : Option Nat → Bool
  | none => true
  | some index => truth.getD index false

def bodyFormula (table : List (DagSharing.Node α)) : Option Nat → Formula α
  | none => truthBody.formula
  | some index => (DagSharing.meanings table).getD index .bot

/-- Both explicit facts and indexed bodies read the same original interpretation. -/
theorem body_value_true (candidate : α → Bool) (table : List (DagSharing.Node α))
    (body : Option Nat) :
    bodyValue (values candidate table) body = true ↔
      Satisfies (interpretation candidate) (bodyFormula table body) := by
  cases body with
  | none => simp [bodyValue, bodyFormula, truthBody, Body.formula, Ferraris.Neg, Satisfies]
  | some index => simp only [bodyValue, bodyFormula, value_at, formula_value_true]

/-- Bidirectional syntactic coverage connects indexed rows to original producers.
Rows may repeat, and producer rows with the same head need not have the same body. -/
def Represents (table : List (DagSharing.Node α)) (rows : List (IndexedProducer α))
    (rules : List (Producer α)) : Prop :=
  (∀ row ∈ rows, ∃ rule ∈ rules,
    row.head = rule.head ∧ bodyFormula table row.body = rule.body.formula) ∧
  (∀ rule ∈ rules, ∃ row ∈ rows,
    row.head = rule.head ∧ bodyFormula table row.body = rule.body.formula)

/-- Independent true producer bodies are reduced by OR for each semantic head. -/
def headSupported [DecidableEq α] (truth : List Bool)
    (rows : List (IndexedProducer α)) (atom : α) : Bool :=
  rows.any (fun row => decide (row.head = atom) && bodyValue truth row.body)

/-- Splitting producer work preserves per-head support by joining with OR.
The result does not require disjoint heads or unique producer occurrences. -/
theorem head_support_append [DecidableEq α] (truth : List Bool)
    (left right : List (IndexedProducer α)) (atom : α) :
    headSupported truth (left ++ right) atom =
      (headSupported truth left atom || headSupported truth right atom) := by
  simp [headSupported]

/-- A complete group computes the same support for each atom it owns as the
whole producer family. Group membership must be exactly original membership
restricted by the head's owner.

A supporting row in the group is an original supporting row. Conversely, an
original supporting row has the queried head, hence the same owner, so coverage
places it in the group. Both evaluations inspect the same body's truth.
This Boolean law permits repeated rows and different row orders; preserving
occurrence counts and charged work is a separate implementation obligation. -/
theorem head_support_group [DecidableEq α] {Group : Type _} (truth : List Bool)
    (rows groupRows : List (IndexedProducer α)) (owner : α → Group)
    (group : Group) (atom : α) (owned : owner atom = group)
    (coverage : ∀ row, row ∈ groupRows ↔ row ∈ rows ∧ owner row.head = group) :
    headSupported truth groupRows atom = headSupported truth rows atom := by
  apply Bool.eq_iff_iff.mpr
  simp only [headSupported, List.any_eq_true, Bool.and_eq_true, decide_eq_true_eq]
  constructor
  · rintro ⟨row, grouped, sameHead, enabled⟩
    have original : row ∈ rows := ((coverage row).mp grouped).1
    exact ⟨row, original, sameHead, enabled⟩
  · rintro ⟨row, original, sameHead, enabled⟩
    have sameOwner : owner row.head = group := by
      rw [sameHead]
      exact owned
    have grouped : row ∈ groupRows := (coverage row).mpr ⟨original, sameOwner⟩
    exact ⟨row, grouped, sameHead, enabled⟩

/-- The computed head reduction has exactly the original true-body witnesses. -/
theorem head_support_true [DecidableEq α] (candidate : α → Bool)
    (table : List (DagSharing.Node α)) (rows : List (IndexedProducer α))
    (rules : List (Producer α)) (linked : Represents table rows rules) (atom : α) :
    headSupported (values candidate table) rows atom = true ↔
      ∃ rule ∈ rules, rule.head = atom ∧ Satisfies (interpretation candidate) rule.body.formula := by
  have rowWitness : headSupported (values candidate table) rows atom = true ↔
      ∃ row ∈ rows, row.head = atom ∧ Satisfies (interpretation candidate) (bodyFormula table row.body) := by
    simp [headSupported, body_value_true]
  rw [rowWitness]
  constructor
  · rintro ⟨row, member, head, body⟩
    obtain ⟨rule, original, sameHead, sameBody⟩ := linked.1 row member
    exact ⟨rule, original, sameHead.symm.trans head, sameBody ▸ body⟩
  · rintro ⟨rule, member, head, body⟩
    obtain ⟨row, stored, sameHead, sameBody⟩ := linked.2 rule member
    exact ⟨row, stored, sameHead.trans head, sameBody ▸ body⟩

/-- Absent atoms demand no support; every present carrier atom is inspected. -/
def supportTrue [DecidableEq α] (candidate : α → Bool) (carrier : List α)
    (truth : List Bool) (rows : List (IndexedProducer α)) : Bool :=
  carrier.all (fun atom => !candidate atom || headSupported truth rows atom)

theorem support_true_iff [DecidableEq α] (candidate : α → Bool) (carrier : List α)
    (table : List (DagSharing.Node α)) (rows : List (IndexedProducer α))
    (rules : List (Producer α)) (linked : Represents table rows rules)
    (covered : ∀ atom, candidate atom = true → atom ∈ carrier) :
    supportTrue candidate carrier (values candidate table) rows = true ↔
      Supported (interpretation candidate) rules := by
  have atomCheck (atom : α) :
      (!candidate atom || headSupported (values candidate table) rows atom) = true ↔
        (candidate atom = true → ∃ rule ∈ rules, rule.head = atom ∧
          Satisfies (interpretation candidate) rule.body.formula) := by
    rw [← head_support_true candidate table rows rules linked atom]
    cases candidate atom <;> simp
  simp only [supportTrue, List.all_eq_true, atomCheck]
  constructor
  · intro checked atom present
    exact checked atom (covered atom present) present
  · intro supported atom _ present
    exact supported atom present

/-- A failed finite support scan refutes stability when the producer grammar
covers every original root. Unlike certified acceptance, this direction needs
no positive rank: every stable model of the covered grammar is supported.

The row representation and present-atom coverage connect the Boolean scan to
that necessity theorem. This justifies a negative membership result, not an
original-model rejection and not a claim that an interrupted scan completed. -/
theorem unsupported_refutes [DecidableEq α] (candidate : α → Bool) (carrier : List α)
    (table : List (DagSharing.Node α)) (rows : List (IndexedProducer α))
    (rules : List (Producer α)) (theory : Theory α)
    (linked : Represents table rows rules)
    (covered : ∀ atom, candidate atom = true → atom ∈ carrier)
    (roots : Covered theory rules)
    (failed : supportTrue candidate carrier (values candidate table) rows = false) :
    ¬ Stable (interpretation candidate) theory := by
  intro stable
  have required_support : Supported (interpretation candidate) rules :=
    stable_supported (interpretation candidate) theory rules roots stable
  have scan_succeeds : supportTrue candidate carrier (values candidate table) rows = true :=
    (support_true_iff candidate carrier table rows rules linked covered).mpr required_support
  have incompatible : (false : Bool) = true := failed.symm.trans scan_succeeds
  cases incompatible

/-- The finite original-root and support scans produce a sound ranked verdict.
The original theory is the whole asserted DAG; no candidate restriction replaces it. -/
theorem computed_verdict_sound [DecidableEq α] (candidate : α → Bool) (carrier : List α)
    (table : List (DagSharing.Node α)) (roots : List Nat)
    (rows : List (IndexedProducer α)) (rules : List (Producer α)) (rank : α → Nat)
    (linked : Represents table rows rules)
    (covered : ∀ atom, candidate atom = true → atom ∈ carrier)
    (original : OriginalProducers (DagSharing.assertions table roots) rules)
    (ranked : Ranked rank rules) :
    CertifiedExecution.Sound (DagSharing.assertions table roots) (interpretation candidate)
      (CertifiedExecution.supportVerdict (rootsTrue (values candidate table) roots)
        (supportTrue candidate carrier (values candidate table) rows)) := by
  exact CertifiedExecution.ranked_support_verdict_sound _ _ rules rank _ _ original ranked
    (roots_true_iff candidate table roots)
    (support_true_iff candidate carrier table rows rules linked covered).mp

/-- Exact residual completion composes with the computed certificate.
No successful completion, source coverage or physical execution is asserted. -/
theorem completed_computation_exact [DecidableEq α] (candidate : α → Bool) (carrier : List α)
    (table : List (DagSharing.Node α)) (roots : List Nat)
    (rows : List (IndexedProducer α)) (rules : List (Producer α)) (rank : α → Nat)
    (linked : Represents table rows rules)
    (covered : ∀ atom, candidate atom = true → atom ∈ carrier)
    (original : OriginalProducers (DagSharing.assertions table roots) rules)
    (ranked : Ranked rank rules) (exact : Option Bool) (result : Bool)
    (oracle : ∀ answer, exact = some answer →
      (answer = true ↔ Stable (interpretation candidate) (DagSharing.assertions table roots)))
    (done : CertifiedExecution.complete
      (CertifiedExecution.supportVerdict (rootsTrue (values candidate table) roots)
        (supportTrue candidate carrier (values candidate table) rows)) exact = some result) :
    result = true ↔ Stable (interpretation candidate) (DagSharing.assertions table roots) := by
  have checkedVerdict := computed_verdict_sound candidate carrier table roots rows rules rank
    linked covered original ranked
  exact CertifiedExecution.completed_membership_exact _ _ _ exact result checkedVerdict oracle done

/-! ## Updating a retained truth table

A checker that keeps the truth table of the previous candidate can bring it to
the next candidate by re-evaluating, in table order, exactly the nodes whose
inputs changed: an atom node whose atom's membership changed, or a connective
with an operand whose truth changed. Every other node keeps its retained truth.
The laws below show that this yields the new candidate's table exactly. Row
re-reduction, the maintained support counts and the unsupported-atom and
false-root summaries are separate obligations; the Rust update realizes this
node-table rule with a worklist and a reverse index, which are not modeled. -/

/-- Whether a node's inputs differ between two candidates: its atom's membership,
or the truth of an operand in the two candidates' computed prefixes. -/
def inputChanged (old new : α → Bool) (oldEarlier newEarlier : List Bool) :
    DagSharing.Node α → Bool
  | .atom atom => old atom != new atom
  | .bot => false
  | .conj left right | .disj left right | .imp left right =>
    oldEarlier.getD left false != newEarlier.getD left false ||
      oldEarlier.getD right false != newEarlier.getD right false

/-- One update step over the pair of the retained table and the updated table:
the retained table grows by the old candidate's truth of the node, and the
updated table re-evaluates the node only when its inputs changed, keeping the
retained truth otherwise. -/
def updateStep (old new : α → Bool) (tables : List Bool × List Bool)
    (node : DagSharing.Node α) : List Bool × List Bool :=
  let retained := nodeValue old tables.1 node
  (tables.1 ++ [retained],
    tables.2 ++ [if inputChanged old new tables.1 tables.2 node then
      nodeValue new tables.2 node else retained])

/-- The table an update produces from the old candidate's table. -/
def updatedValues (old new : α → Bool) (table : List (DagSharing.Node α)) : List Bool :=
  (table.foldl (updateStep old new) ([], [])).2

/-- A node whose inputs did not change has the same truth under both candidates.
An atom node reads the same membership; a connective reads equal operand truth. -/
theorem unchanged_inputs_keep_truth (old new : α → Bool) (oldEarlier newEarlier : List Bool)
    (node : DagSharing.Node α)
    (unchanged : inputChanged old new oldEarlier newEarlier node = false) :
    nodeValue old oldEarlier node = nodeValue new newEarlier node := by
  cases node with
  | atom atom =>
    have same : old atom = new atom := by simpa [inputChanged] using unchanged
    simp [nodeValue, same]
  | bot => rfl
  | conj left right =>
    have same : oldEarlier.getD left false = newEarlier.getD left false ∧
        oldEarlier.getD right false = newEarlier.getD right false := by
      simpa [inputChanged] using unchanged
    simp only [nodeValue]
    rw [same.1, same.2]
  | disj left right =>
    have same : oldEarlier.getD left false = newEarlier.getD left false ∧
        oldEarlier.getD right false = newEarlier.getD right false := by
      simpa [inputChanged] using unchanged
    simp only [nodeValue]
    rw [same.1, same.2]
  | imp left right =>
    have same : oldEarlier.getD left false = newEarlier.getD left false ∧
        oldEarlier.getD right false = newEarlier.getD right false := by
      simpa [inputChanged] using unchanged
    simp only [nodeValue]
    rw [same.1, same.2]

/-- Whichever branch an update step takes, it appends each candidate's own
truth of the node: a re-evaluated node by definition, a kept node by
`unchanged_inputs_keep_truth`. -/
theorem update_step_appends_truth (old new : α → Bool) (oldEarlier newEarlier : List Bool)
    (node : DagSharing.Node α) :
    updateStep old new (oldEarlier, newEarlier) node =
      (oldEarlier ++ [nodeValue old oldEarlier node],
        newEarlier ++ [nodeValue new newEarlier node]) := by
  cases changed : inputChanged old new oldEarlier newEarlier node with
  | true => simp [updateStep, changed]
  | false =>
    have kept := unchanged_inputs_keep_truth old new oldEarlier newEarlier node changed
    simp [updateStep, changed, kept]

/-- Updating runs both candidates' table folds side by side, from any prefixes. -/
theorem update_fold_pairs (old new : α → Bool) (table : List (DagSharing.Node α))
    (oldEarlier newEarlier : List Bool) :
    table.foldl (updateStep old new) (oldEarlier, newEarlier) =
      (table.foldl (fun earlier node => earlier ++ [nodeValue old earlier node]) oldEarlier,
        table.foldl (fun earlier node => earlier ++ [nodeValue new earlier node]) newEarlier) := by
  induction table generalizing oldEarlier newEarlier with
  | nil => rfl
  | cons node tail inductionHypothesis =>
    simp only [List.foldl_cons, update_step_appends_truth]
    exact inductionHypothesis _ _

/-- Re-evaluating, in table order, exactly the nodes whose inputs changed, and
keeping the retained truth of every other node, yields the new candidate's
truth table. -/
theorem updated_values_exact (old new : α → Bool) (table : List (DagSharing.Node α)) :
    updatedValues old new table = values new table := by
  simp only [updatedValues, values, update_fold_pairs]

/-- The truth the update keeps is the old candidate's own table. -/
theorem update_retains_old_values (old new : α → Bool) (table : List (DagSharing.Node α)) :
    (table.foldl (updateStep old new) ([], [])).1 = values old table := by
  simp only [values, update_fold_pairs]

end Zetesis.TightEvaluation
