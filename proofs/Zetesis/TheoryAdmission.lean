import Zetesis.IndexedEvaluation
import Zetesis.PackedInterpretations

/-!
# Admission of finite formula tables

The validator checks dimensions, the padded atom count, every node in order and
then the asserted roots. Successful validation derives the topological and root
premises used by checked reduct evaluation. Atom coordinates are also checked
against the declared universe. Rejection precedes construction of any theory.

This is an authored model of the checks in `Theory::new`, not Rust extraction.
Natural-number dimensions model already represented input lengths; this file
does not prove Vec allocation, Arc identity or every machine operation.
-/

namespace Zetesis.Refinement.TheoryAdmission

open DagSharing

/-- The structural reason for refusing a proposed finite theory. -/
inductive Error where
  | limit | atom | edge | root
  deriving DecidableEq, Repr

/-- Caller-supplied dimension ceilings. Machine word size is a separate bound. -/
structure Limits where
  atoms : Nat
  nodes : Nat
  roots : Nat
  deriving DecidableEq, Repr

/-- A node's atom, when present, belongs to the declared universe. -/
def AtomBound (size : Nat) : Node Nat → Prop
  | .atom atom => atom < size
  | _ => True

/-- Check one node at its actual construction index. Connectives may refer
only to preceding nodes; atoms may name any declared atom coordinate. -/
def node (size index : Nat) : Node Nat → Except Error Unit
  | .atom atom => if atom < size then .ok () else .error .atom
  | .bot => .ok ()
  | .conj left right | .disj left right | .imp left right =>
    if left < index ∧ right < index then .ok () else .error .edge

/-- The executable node check is exactly the atom and edge admission contract. -/
theorem node_exact (size index : Nat) (entry : Node Nat) :
    node size index entry = .ok () ↔ ValidNode index entry ∧ AtomBound size entry := by
  cases entry <;> simp [node, ValidNode, AtomBound]

/-- Walk the supplied nodes in order, retaining the first structural error. -/
def scan (size : Nat) : Nat → List (Node Nat) → Except Error Unit
  | _, [] => .ok ()
  | index, entry :: rest =>
    (node size index entry).bind (fun _ => scan size (index + 1) rest)

/-- Two consecutive structural checks succeed exactly when each succeeds.
A refusal in the first check prevents the second from running. -/
theorem checks_succeed (first second : Except Error Unit) :
    first.bind (fun _ => second) = .ok () ↔ first = .ok () ∧ second = .ok () := by
  cases first with
  | error error => simp [Except.bind]
  | ok value => cases value; simp [Except.bind]

/-- Scanning two consecutive segments advances by the first segment's actual
length. A refused prefix prevents the suffix from being scanned. -/
theorem scan_append (size index : Nat) (first second : List (Node Nat)) :
    scan size index (first ++ second) =
      (scan size index first).bind (fun _ => scan size (index + first.length) second) := by
  induction first generalizing index with
  | nil => simp [scan, Except.bind]
  | cons entry rest inductionHypothesis =>
    simp only [List.cons_append, scan, inductionHypothesis, List.length_cons]
    cases node size index entry <;> simp [Except.bind, Nat.add_assoc, Nat.add_comm]

/-- A successful remaining scan extends an already admitted prefix. This
constructs the inductive topological certificate from the actual node checks. -/
theorem scan_extends (size : Nat) (processed rest : List (Node Nat))
    (valid : WellFormed processed) (accepted : scan size processed.length rest = .ok ()) :
    WellFormed (processed ++ rest) := by
  induction rest generalizing processed with
  | nil => simpa using valid
  | cons entry rest inductionHypothesis =>
    have checks : node size processed.length entry = .ok () ∧
        scan size (processed.length + 1) rest = .ok () := by
      exact (checks_succeed _ _).mp accepted
    have nextValid : WellFormed (processed ++ [entry]) :=
      WellFormed.snoc valid ((node_exact size processed.length entry).mp checks.1).1
    have next := inductionHypothesis (processed ++ [entry]) nextValid (by simpa using checks.2)
    simpa [List.append_assoc] using next

/-- Every atom in a successfully scanned node lies in the declared universe. -/
theorem scan_atoms (size index : Nat) (table : List (Node Nat))
    (accepted : scan size index table = .ok ()) :
    ∀ entry ∈ table, AtomBound size entry := by
  induction table generalizing index with
  | nil => simp
  | cons entry rest inductionHypothesis =>
    have checks := (checks_succeed _ _).mp accepted
    intro tested member
    rcases List.mem_cons.mp member with rfl | remaining
    · exact ((node_exact size index _).mp checks.1).2
    · exact inductionHypothesis (index + 1) checks.2 tested remaining

/-- Conversely, a topological table with bounded atom coordinates passes the
scan. Induction follows the table's admitted prefixes and uses scan_append. -/
theorem scan_complete (size : Nat) (table : List (Node Nat))
    (valid : WellFormed table) (atoms : ∀ entry ∈ table, AtomBound size entry) :
    scan size 0 table = .ok () := by
  induction valid with
  | nil => rfl
  | @snoc table entry valid bounded inductionHypothesis =>
    have old := inductionHypothesis (fun item member => atoms item (List.mem_append_left _ member))
    have last := (node_exact size table.length entry).mpr
      ⟨bounded, atoms entry (by simp)⟩
    rw [scan_append, old]
    simp [scan, last, Except.bind]

/-- Successful validation derives the complete node certificate, and every
table satisfying that certificate passes. No admission oracle is assumed. -/
theorem scan_exact (size : Nat) (table : List (Node Nat)) :
    scan size 0 table = .ok () ↔
      WellFormed table ∧ ∀ entry ∈ table, AtomBound size entry := by
  constructor
  · intro accepted
    exact ⟨by simpa using scan_extends size [] table WellFormed.nil accepted,
      scan_atoms size 0 table accepted⟩
  · rintro ⟨valid, bounded⟩
    exact scan_complete size table valid bounded

/-- A refused scan reports its first refused node: every earlier node passes
its check at its actual position, and the reported error is that node's own.
A later node's error therefore cannot be reported in its place.

Proof: induct on the table. A refused head is the first refused node, and no
witness can lie beyond it because the witness's earlier nodes include the head.
An accepted head leaves the scan of the tail one position later, so a tail
witness and a witness beyond the head correspond by shifting one position. -/
theorem scan_refusal_exact (size index : Nat) (table : List (Node Nat))
    (reason : Error) :
    scan size index table = .error reason ↔
      ∃ position, ∃ inside : position < table.length,
        (∀ earlier, ∀ before : earlier < position,
          node size (index + earlier) table[earlier] = .ok ()) ∧
        node size (index + position) table[position] = .error reason := by
  induction table generalizing index with
  | nil =>
    simp [scan]
  | cons entry rest inductionHypothesis =>
    cases headCheck : node size index entry with
    | error headReason =>
      have refusedHere : scan size index (entry :: rest) = .error headReason := by
        simp [scan, headCheck, Except.bind]
      rw [refusedHere]
      constructor
      · intro sameReason
        cases sameReason
        refine ⟨0, Nat.succ_pos _, fun earlier before => absurd before (Nat.not_lt_zero _), ?_⟩
        simpa using headCheck
      · rintro ⟨position, inside, earlierPass, refusedAt⟩
        cases position with
        | zero =>
          have reported : Except.error headReason = (Except.error reason : Except Error Unit) := by
            simpa [headCheck] using refusedAt
          exact reported
        | succ later =>
          have headPasses : node size index entry = .ok () := by
            simpa using earlierPass 0 (Nat.succ_pos _)
          rw [headCheck] at headPasses
          cases headPasses
    | ok accepted =>
      cases accepted
      have continued : scan size index (entry :: rest) = scan size (index + 1) rest := by
        simp [scan, headCheck, Except.bind]
      have shifted (offset : Nat) : index + 1 + offset = index + (offset + 1) := by omega
      rw [continued, inductionHypothesis (index + 1)]
      constructor
      · rintro ⟨position, inside, earlierPass, refusedAt⟩
        refine ⟨position + 1, by simpa using inside, ?_, ?_⟩
        · intro earlier before
          cases earlier with
          | zero => simpa using headCheck
          | succ previous =>
            have tailPasses := earlierPass previous (by omega)
            rw [shifted] at tailPasses
            simpa using tailPasses
        · rw [shifted] at refusedAt
          simpa using refusedAt
      · rintro ⟨position, inside, earlierPass, refusedAt⟩
        cases position with
        | zero =>
          have headRefused : node size index entry = .error reason := by
            simpa using refusedAt
          rw [headCheck] at headRefused
          cases headRefused
        | succ later =>
          refine ⟨later, by simpa using inside, ?_, ?_⟩
          · intro earlier before
            have tailPasses := earlierPass (earlier + 1) (by omega)
            rw [← shifted] at tailPasses
            simpa using tailPasses
          · rw [← shifted] at refusedAt
            simpa using refusedAt

/-- Check one asserted root against the number of stored nodes: it must name a
stored node. -/
def root (count asserted : Nat) : Except Error Unit :=
  if asserted < count then .ok () else .error .root

/-- Walk the asserted roots in stored order, retaining the first refusal.
Repeated roots are checked as they occur. -/
def rootScan (count : Nat) : List Nat → Except Error Unit
  | [] => .ok ()
  | asserted :: rest => (root count asserted).bind (fun _ => rootScan count rest)

/-- The root scan accepts exactly when every asserted root names a stored node. -/
theorem rootScan_exact (count : Nat) (roots : List Nat) :
    rootScan count roots = .ok () ↔ ∀ asserted ∈ roots, asserted < count := by
  induction roots with
  | nil => simp [rootScan]
  | cons asserted rest inductionHypothesis =>
    have headCheck : root count asserted = .ok () ↔ asserted < count := by
      unfold root
      split <;> simp_all
    rw [rootScan, checks_succeed, headCheck, inductionHypothesis, List.forall_mem_cons]

/-- A refused root scan reports a root refusal, and some asserted root names no
stored node. Every root refusal has the same kind, so a refusal does not
identify which occurrence failed. -/
theorem rootScan_refusal_exact (count : Nat) (roots : List Nat) (reason : Error) :
    rootScan count roots = .error reason ↔
      reason = .root ∧ ∃ asserted ∈ roots, count ≤ asserted := by
  induction roots with
  | nil => simp [rootScan]
  | cons asserted rest inductionHypothesis =>
    by_cases stored : asserted < count
    · have continued : rootScan count (asserted :: rest) = rootScan count rest := by
        simp [rootScan, root, stored, Except.bind]
      rw [continued, inductionHypothesis]
      constructor
      · rintro ⟨sameReason, unstored, member, outside⟩
        exact ⟨sameReason, unstored, List.mem_cons_of_mem _ member, outside⟩
      · rintro ⟨sameReason, unstored, member, outside⟩
        rcases List.mem_cons.mp member with rfl | later
        · omega
        · exact ⟨sameReason, unstored, later, outside⟩
    · have refusedHere : rootScan count (asserted :: rest) = .error .root := by
        simp [rootScan, root, stored, Except.bind]
      rw [refusedHere]
      constructor
      · intro sameReason
        cases sameReason
        exact ⟨rfl, asserted, List.mem_cons_self, by omega⟩
      · rintro ⟨sameReason, _⟩
        rw [sameReason]

/-- Validate dimensions before any node, then nodes before roots. The padded
count check models successful `checked_add(63)` against the host maximum. -/
def validate (maximum : Nat) (limits : Limits) (size : Nat)
    (table : List (Node Nat)) (roots : List Nat) : Except Error Unit :=
  if limits.atoms < size ∨ limits.nodes < table.length ∨
      limits.roots < roots.length ∨ maximum < size + 63 then
    .error .limit
  else
    (scan size 0 table).bind (fun _ =>
      if roots.all (fun root => decide (root < table.length)) then .ok () else .error .root)

/-- Validation checks dimensions and the padded count, then scans the nodes,
then scans the asserted roots against the stored node count, stopping at the
first refusal. Its root clause is the root scan.

Proof: the root clause and the root scan agree because each accepts exactly
when every asserted root names a stored node, and each otherwise refuses with
a root refusal. -/
theorem validate_phases (maximum : Nat) (limits : Limits) (size : Nat)
    (table : List (Node Nat)) (roots : List Nat) :
    validate maximum limits size table roots =
      if limits.atoms < size ∨ limits.nodes < table.length ∨
          limits.roots < roots.length ∨ maximum < size + 63 then .error .limit
      else (scan size 0 table).bind (fun _ => rootScan table.length roots) := by
  have rootClause : (if roots.all (fun root => decide (root < table.length)) then
      Except.ok () else Except.error Error.root) = rootScan table.length roots := by
    by_cases allStored : ∀ asserted ∈ roots, asserted < table.length
    · rw [(rootScan_exact _ _).mpr allStored]
      simp only [List.all_eq_true, decide_eq_true_eq]
      rw [if_pos allStored]
    · have someUnstored : ∃ asserted ∈ roots, table.length ≤ asserted := by
        simpa using allStored
      rw [(rootScan_refusal_exact _ _ _).mpr ⟨rfl, someUnstored⟩]
      simp only [List.all_eq_true, decide_eq_true_eq]
      rw [if_neg allStored]
  simp only [validate, rootClause]

/-- Validation succeeds exactly when dimensions, word-count addition, nodes
and roots meet the structural contract. The scan theorem supplies the DAG
certificate, while the root reduction supplies every asserted index bound. -/
theorem validate_exact (maximum : Nat) (limits : Limits) (size : Nat)
    (table : List (Node Nat)) (roots : List Nat) :
    validate maximum limits size table roots = .ok () ↔
      size ≤ limits.atoms ∧ table.length ≤ limits.nodes ∧ roots.length ≤ limits.roots ∧
      size + 63 ≤ maximum ∧ WellFormed table ∧
      (∀ entry ∈ table, AtomBound size entry) ∧
      (∀ root ∈ roots, root < table.length) := by
  simp only [validate]
  split
  · rename_i refused
    simp only [reduceCtorEq, false_iff]
    intro ⟨atoms, nodes, count, wordCount, _⟩
    omega
  · rename_i dimensions
    rw [checks_succeed, scan_exact]
    have rootCheck :
        (if roots.all (fun root => decide (root < table.length)) then
          Except.ok () else Except.error Error.root) = .ok () ↔
          ∀ root ∈ roots, root < table.length := by simp
    rw [rootCheck]
    have limitsFit : size ≤ limits.atoms ∧ table.length ≤ limits.nodes ∧
        roots.length ≤ limits.roots ∧ size + 63 ≤ maximum := by omega
    constructor
    · rintro ⟨⟨valid, bounded⟩, rootsInside⟩
      exact ⟨limitsFit.1, limitsFit.2.1, limitsFit.2.2.1, limitsFit.2.2.2,
        valid, bounded, rootsInside⟩
    · rintro ⟨_, _, _, _, valid, bounded, rootsInside⟩
      exact ⟨⟨valid, bounded⟩, rootsInside⟩

/-- The admitted padded atom count also bounds both exported word counts and
the smaller padding addition. This is a natural-number range argument against
the supplied host maximum, not extraction of Rust's checked arithmetic. -/
theorem word_counts_fit (maximum : Nat) (limits : Limits) (size : Nat)
    (table : List (Node Nat)) (roots : List Nat)
    (accepted : validate maximum limits size table roots = .ok ()) :
    size + 31 ≤ maximum ∧ PackedInterpretations.count64 size ≤ maximum ∧
      PackedInterpretations.count32 size ≤ maximum := by
  have padded := ((validate_exact _ _ _ _ _).mp accepted).2.2.2.1
  unfold PackedInterpretations.count64 PackedInterpretations.count32
  omega

/-- A live export cursor can advance once without exceeding the supplied host
maximum. Reaching the endpoint never requires incrementing it again. -/
theorem export_successor_fits (maximum : Nat) (limits : Limits) (size : Nat)
    (table : List (Node Nat)) (roots : List Nat)
    (accepted : validate maximum limits size table roots = .ok ())
    (index : Nat) (active : index < PackedInterpretations.count32 size) :
    index + 1 ≤ maximum := by
  have count := (word_counts_fit maximum limits size table roots accepted).2.2
  omega

/-- Accepted validation supplies the structural premises of checked reduct
evaluation. The truth functions are arbitrary; no satisfaction is assumed. -/
theorem validated_reduct_exact (maximum : Nat) (limits : Limits) (size : Nat)
    (table : List (Node Nat)) (roots : List Nat)
    (accepted : validate maximum limits size table roots = .ok ())
    (outer tested : Nat → Bool) :
    IndexedEvaluation.satisfiesReduct outer tested table roots =
      some (TightEvaluation.rootsTrue (ReductEvaluation.values outer tested table) roots) := by
  obtain ⟨_, _, _, _, valid, _, rootBounds⟩ := (validate_exact _ _ _ _ _).mp accepted
  exact IndexedEvaluation.satisfies_reduct_exact outer tested table roots valid rootBounds

/-- Dimension failure is reported before a malformed node is inspected. -/
theorem dimensions_precede_nodes :
    validate 255 ⟨0, 0, 0⟩ 1 [.atom 1] [] = .error .limit := by rfl

/-- A self-reference is refused even when no root observes the node. -/
theorem self_reference_is_refused :
    validate 255 ⟨1, 1, 1⟩ 1 [.conj 0 0] [] = .error .edge := by rfl

/-- The atom check covers unasserted nodes as well as asserted formulas. -/
theorem unasserted_atom_is_checked :
    validate 255 ⟨1, 1, 1⟩ 1 [.atom 1] [] = .error .atom := by rfl

end Zetesis.Refinement.TheoryAdmission
