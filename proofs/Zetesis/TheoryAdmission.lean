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
