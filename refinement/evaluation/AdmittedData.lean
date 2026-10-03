import AdmissionValidation

open Aeneas Aeneas.Std Result
open ZetesisExtract
open Zetesis Zetesis.Refinement

/-!
# The admitted data of a formula theory

`Theory::new` first runs the private step `admit`, which checks the atom, node
and root limits and the padded atom count, then validates every node and every
root, and finally returns the admitted data holding the supplied vectors. Only
then does `Theory::new` allocate the shared theory.

This module proves the generated `admit` against the authored validator
`TheoryAdmission.validate`, with the host's `Usize.max` as its maximum. Rust's
`checked_add(63)` succeeds exactly when the authored padded count fits. The
equation holds for every input, so `admit` always returns: a refusal is the
authored refusal, and success returns the supplied atom count and vectors
unchanged, in their stored order and multiplicity.

`TheoryConstruction` connects these results to the generated `Theory::new`
wrapper under an explicit allocation contract. It derives the stored value
needed by the structural results below. A fresh owner is a separate library
contract, needed only for ownership results.
-/

namespace AdmittedData

/-- The authored limits read from the generated limit record. -/
def limitsOf (limits : theory.AdmissionLimits) : TheoryAdmission.Limits :=
  ⟨limits.max_atoms.val, limits.max_nodes.val, limits.max_roots.val⟩

/-- The generated admission step returns the authored validator's verdict,
with the admitted data on success. The validator's maximum is the host's
`Usize.max`, so its padded-count check is Rust's `checked_add(63)`.

Proof: the dimension comparisons read their machine values; the checked
addition fails exactly when `Usize.max < atoms + 63`; the two validator calls
are the exact node and root scans; `validate_phases` gives the authored
validator the same order. Each refusal stops both at the same check. -/
theorem admit_exact (atoms : Usize) (nodes : alloc.vec.Vec theory.Node)
    (roots : alloc.vec.Vec Usize) (limits : theory.AdmissionLimits) :
    theory.admit atoms nodes roots limits =
      ok (match TheoryAdmission.validate Usize.max (limitsOf limits) atoms.val
          (nodes.val.map EvaluationSemantics.node) (roots.val.map UScalar.val) with
        | .ok _ => .Ok { atoms, nodes, roots }
        | .error reason => .Err (AdmissionValidation.refusal reason)) := by
  have nodesChecked : theory.validate_nodes atoms (alloc.vec.Vec.deref nodes) =
      ok (AdmissionValidation.verdict (TheoryAdmission.scan atoms.val 0
        (nodes.val.map EvaluationSemantics.node))) := by
    rw [AdmissionValidation.validate_nodes_exact]
    simp [alloc.vec.Vec.deref]
  have rootsChecked : theory.validate_roots (alloc.vec.Vec.len nodes)
      (alloc.vec.Vec.deref roots) =
      ok (AdmissionValidation.verdict (TheoryAdmission.rootScan nodes.val.length
        (roots.val.map UScalar.val))) := by
    rw [AdmissionValidation.validate_roots_exact]
    simp [alloc.vec.Vec.deref]
  have padded := Usize.checked_add_bv_spec atoms 63#usize
  rw [TheoryAdmission.validate_phases]
  simp only [List.length_map, limitsOf]
  by_cases atomsOver : limits.max_atoms.val < atoms.val
  · simp [theory.admit, atomsOver, AdmissionValidation.refusal]
  by_cases nodesOver : limits.max_nodes.val < nodes.val.length
  · simp [theory.admit, atomsOver, nodesOver, AdmissionValidation.refusal]
  by_cases rootsOver : limits.max_roots.val < roots.val.length
  · simp [theory.admit, atomsOver, nodesOver, rootsOver, AdmissionValidation.refusal]
  cases sum : Usize.checked_add atoms 63#usize with
  | none =>
    rw [sum] at padded
    have overflow : Usize.max < atoms.val + 63 := by simpa using padded
    simp [theory.admit, atomsOver, nodesOver, rootsOver, sum, overflow, lift,
      AdmissionValidation.refusal]
  | some total =>
    rw [sum] at padded
    have fits : ¬ Usize.max < atoms.val + 63 := by
      have bounded : atoms.val + 63 ≤ Usize.max := by simpa using padded.1
      omega
    simp only [theory.admit, atomsOver, nodesOver, rootsOver, fits, alloc.vec.Vec.len_val,
      sum, gt_iff_lt, UScalar.lt_equiv, Option.isNone_some, core.option.Option.is_none, lift,
      bind_tc_ok, if_false, or_self, nodesChecked]
    cases scanned : TheoryAdmission.scan atoms.val 0 (nodes.val.map EvaluationSemantics.node) with
    | error authored =>
      simp [AdmissionValidation.verdict, Except.bind,
        core.result.Result.Insts.CoreOpsTry.branch,
        core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]
    | ok accepted =>
      cases accepted
      simp only [AdmissionValidation.verdict, core.result.Result.Insts.CoreOpsTry.branch,
        bind_tc_ok, rootsChecked, Except.bind]
      cases rootScanned : TheoryAdmission.rootScan nodes.val.length
          (roots.val.map UScalar.val) with
      | error authored =>
        simp [core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]
      | ok rootAccepted =>
        simp

/-- Admission succeeds exactly when the limits, the padded count, the node
table and the asserted roots meet the authored contract, and then returns the
supplied data unchanged. -/
theorem admit_accepts_iff (atoms : Usize) (nodes : alloc.vec.Vec theory.Node)
    (roots : alloc.vec.Vec Usize) (limits : theory.AdmissionLimits) (data : theory.Data) :
    theory.admit atoms nodes roots limits = ok (.Ok data) ↔
      data = { atoms, nodes, roots } ∧
      atoms.val ≤ limits.max_atoms.val ∧ nodes.val.length ≤ limits.max_nodes.val ∧
      roots.val.length ≤ limits.max_roots.val ∧ atoms.val + 63 ≤ Usize.max ∧
      DagSharing.WellFormed (nodes.val.map EvaluationSemantics.node) ∧
      (∀ entry ∈ nodes.val,
        TheoryAdmission.AtomBound atoms.val (EvaluationSemantics.node entry)) ∧
      ∀ root ∈ roots.val, root.val < nodes.val.length := by
  rw [admit_exact, Result.ok.injEq]
  cases validated : TheoryAdmission.validate Usize.max (limitsOf limits) atoms.val
      (nodes.val.map EvaluationSemantics.node) (roots.val.map UScalar.val) with
  | error authored =>
    have refused : ¬ (TheoryAdmission.validate Usize.max (limitsOf limits) atoms.val
        (nodes.val.map EvaluationSemantics.node) (roots.val.map UScalar.val) = .ok ()) := by
      rw [validated]
      simp
    rw [TheoryAdmission.validate_exact] at refused
    simp only [reduceCtorEq, false_iff]
    rintro ⟨_, atomsFit, nodesFit, rootsFit, paddedFits, wellFormed, atomsInside, rootsInside⟩
    apply refused
    refine ⟨atomsFit, by simpa [limitsOf] using nodesFit, by simpa [limitsOf] using rootsFit,
      paddedFits, wellFormed, ?_, ?_⟩
    · simpa using atomsInside
    · simpa using rootsInside
  | ok accepted =>
    cases accepted
    have admitted := (TheoryAdmission.validate_exact _ _ _ _ _).mp validated
    obtain ⟨atomsFit, nodesFit, rootsFit, paddedFits, wellFormed, atomsInside, rootsInside⟩ :=
      admitted
    simp only [core.result.Result.Ok.injEq]
    constructor
    · intro same
      refine ⟨same.symm, atomsFit, by simpa [limitsOf] using nodesFit,
        by simpa [limitsOf] using rootsFit, paddedFits, wellFormed,
        by simpa using atomsInside, ?_⟩
      simpa using rootsInside
    · rintro ⟨same, _⟩
      exact same.symm

/-- A refusal is the authored validator's refusal, read as the generated
error. -/
theorem admit_refuses_iff (atoms : Usize) (nodes : alloc.vec.Vec theory.Node)
    (roots : alloc.vec.Vec Usize) (limits : theory.AdmissionLimits)
    (reason : theory.AdmissionError) :
    theory.admit atoms nodes roots limits = ok (.Err reason) ↔
      ∃ authored, TheoryAdmission.validate Usize.max (limitsOf limits) atoms.val
          (nodes.val.map EvaluationSemantics.node) (roots.val.map UScalar.val) =
          .error authored ∧
        reason = AdmissionValidation.refusal authored := by
  rw [admit_exact, Result.ok.injEq]
  cases TheoryAdmission.validate Usize.max (limitsOf limits) atoms.val
      (nodes.val.map EvaluationSemantics.node) (roots.val.map UScalar.val) with
  | ok accepted => simp
  | error authored =>
    simp only [core.result.Result.Err.injEq, Except.error.injEq]
    exact ⟨fun same => ⟨authored, rfl, same.symm⟩,
      fun ⟨_, sameAuthored, sameReason⟩ => sameAuthored ▸ sameReason.symm⟩

/-- Admission never reports an allocation refusal. -/
theorem admit_never_allocation (atoms : Usize) (nodes : alloc.vec.Vec theory.Node)
    (roots : alloc.vec.Vec Usize) (limits : theory.AdmissionLimits) :
    theory.admit atoms nodes roots limits ≠ ok (.Err .Allocation) := by
  intro reported
  obtain ⟨authored, _, sameReason⟩ :=
    (admit_refuses_iff atoms nodes roots limits .Allocation).mp reported
  exact AdmissionValidation.refusal_ne_allocation authored sameReason.symm

/-- Admitted data fits the host's word counts: the smaller padding, the
64-bit word count and the 32-bit word count are all representable. -/
theorem admitted_word_counts (atoms : Usize) (nodes : alloc.vec.Vec theory.Node)
    (roots : alloc.vec.Vec Usize) (limits : theory.AdmissionLimits) (data : theory.Data)
    (admitted : theory.admit atoms nodes roots limits = ok (.Ok data)) :
    data.atoms.val + 31 ≤ Usize.max ∧
      PackedInterpretations.count64 data.atoms.val ≤ Usize.max ∧
      PackedInterpretations.count32 data.atoms.val ≤ Usize.max := by
  have validated : TheoryAdmission.validate Usize.max (limitsOf limits) atoms.val
      (nodes.val.map EvaluationSemantics.node) (roots.val.map UScalar.val) = .ok () := by
    rw [admit_exact] at admitted
    cases accepted : TheoryAdmission.validate Usize.max (limitsOf limits) atoms.val
        (nodes.val.map EvaluationSemantics.node) (roots.val.map UScalar.val) with
    | ok value => cases value; rfl
    | error authored => simp [accepted] at admitted
  have same : data = { atoms, nodes, roots } :=
    ((admit_accepts_iff atoms nodes roots limits data).mp admitted).1
  subst same
  exact TheoryAdmission.word_counts_fit _ _ _ _ _ validated

/-- A theory whose stored value is admitted data satisfies the structural
premises of the membership and query theorems: ordered children and bounded
roots. `TheoryConstruction.returned_structure` supplies the value equation
from the generated wrapper and its allocation contract. The owner plays no part. -/
theorem admitted_program_structure (atoms : Usize) (nodes : alloc.vec.Vec theory.Node)
    (roots : alloc.vec.Vec Usize) (limits : theory.AdmissionLimits) (data : theory.Data)
    (admitted : theory.admit atoms nodes roots limits = ok (.Ok data))
    (program : theory.Theory) (stored : program.value = data) :
    EvaluationSpecification.Ordered program.value.nodes.val ∧
      ∀ root ∈ program.value.roots.val, root.val < program.value.nodes.val.length := by
  obtain ⟨same, _, _, _, _, _, _, rootsInside⟩ :=
    (admit_accepts_iff atoms nodes roots limits data).mp admitted
  have nodesAccepted : theory.validate_nodes atoms (alloc.vec.Vec.deref nodes) =
      ok (.Ok ()) := by
    rw [AdmissionValidation.validate_nodes_accepts_iff]
    have accepted := (admit_accepts_iff atoms nodes roots limits data).mp admitted
    exact ⟨by simpa [alloc.vec.Vec.deref] using accepted.2.2.2.2.2.1,
      by simpa [alloc.vec.Vec.deref] using accepted.2.2.2.2.2.2.1⟩
  have ordered : EvaluationSpecification.Ordered nodes.val := by
    have fromSlice := AdmissionValidation.accepted_ordered atoms (alloc.vec.Vec.deref nodes)
      nodesAccepted
    simpa [alloc.vec.Vec.deref] using fromSlice
  rw [stored, same]
  exact ⟨ordered, rootsInside⟩

end AdmittedData
