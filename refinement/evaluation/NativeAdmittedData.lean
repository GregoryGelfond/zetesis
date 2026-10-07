import NativeAdmissionValidation
import NativeOccurrenceCount
import Zetesis.PackedInterpretations

open Aeneas Aeneas.Std Result
open ZetesisNativeExtract
open NativeStructure NativeAdmissionValidation NativeBoundsValidation NativeOccurrenceCount

/-!
# Actual native paired-data admission

This finite specification follows the generated phase order. Physical arena
cells and logical child occurrences have separate checks against the operand
limit. The latter sum is recomputed from complete raw rows; the input's cached
count is never an admission premise. Success preserves nodes, arena, roots and
all their order and multiplicity, replacing only that count.
-/
namespace NativeAdmittedData

/-- The single paired view exposed by the input parts. -/
def view (parts : theory.FormulaParts) : theory.FormulaView :=
  { nodes := alloc.vec.Vec.deref parts.nodes, operands := alloc.vec.Vec.deref parts.operands }

/-- The finite ordered admission verdict, including recomputed metadata. -/
def validate (atoms : Usize) (parts : theory.FormulaParts)
    (roots : alloc.vec.Vec Usize) (limits : theory.AdmissionLimits) :
    core.result.Result theory.Data theory.AdmissionError :=
  if limits.max_atoms.val < atoms.val then .Err .Limit
  else if limits.max_nodes.val < parts.nodes.val.length then .Err .Limit
  else if limits.max_roots.val < roots.val.length then .Err .Limit
  else if limits.max_operands.val < parts.operands.val.length then .Err .Limit
  else if Usize.max < atoms.val + 63 then .Err .Limit
  else match result (occurrences parts.nodes.val) with
    | .Err reason => .Err reason
    | .Ok count =>
      if limits.max_operands.val < count.val then .Err .Limit
      else match scan atoms.val 0 (view parts).operands parts.nodes.val with
        | .Err reason => .Err reason
        | .Ok _ => match bounds parts.nodes.val.length .Root roots.val with
          | .Err reason => .Err reason
          | .Ok _ => .Ok { atoms, parts := { parts with occurrences := count }, roots }

/-- Actual admission terminates with exactly the finite phase verdict. Checked
padding, occurrence overflow and each first structural refusal retain priority. -/
theorem admit_exact (atoms : Usize) (parts : theory.FormulaParts)
    (roots : alloc.vec.Vec Usize) (limits : theory.AdmissionLimits) :
    theory.admit atoms parts roots limits = ok (validate atoms parts roots limits) := by
  have counted : theory.count_occurrences (alloc.vec.Vec.deref parts.nodes) =
      ok (result (occurrences parts.nodes.val)) := by
    simpa only [alloc.vec.Vec.deref, Slice.from_val] using count_occurrences_exact (alloc.vec.Vec.deref parts.nodes)
  have rootsLength : Slice.len (alloc.vec.Vec.deref roots) = alloc.vec.Vec.len roots := by
    apply UScalar.eq_of_val_eq
    simp [alloc.vec.Vec.deref]
  have viewed : theory.FormulaParts.view parts = ok (view parts) := rfl
  have scanned : theory.validate_nodes atoms (view parts) =
      ok (scan atoms.val 0 (view parts).operands parts.nodes.val) := by
    simpa only [view, alloc.vec.Vec.deref, Slice.from_val] using validate_nodes_exact atoms (view parts)
  have rooted : theory.validate_roots (alloc.vec.Vec.len parts.nodes) (alloc.vec.Vec.deref roots) =
      ok (bounds parts.nodes.val.length .Root roots.val) := by
    simpa only [alloc.vec.Vec.len_val, alloc.vec.Vec.length, alloc.vec.Vec.deref, Slice.from_val] using validate_roots_exact
      (alloc.vec.Vec.len parts.nodes) (alloc.vec.Vec.deref roots)
  have padded : match Usize.checked_add atoms 63#usize with
      | some total => atoms.val + 63 ≤ Usize.max ∧ total.val = atoms.val + 63 ∧
          total.bv = atoms.bv + (63#usize).bv
      | none => Usize.max < atoms.val + 63 := Usize.checked_add_bv_spec atoms 63#usize
  by_cases atomsOver : limits.max_atoms.val < atoms.val
  · simp [theory.admit, theory.admit_dimensions, validate, atomsOver,
      core.result.Result.Insts.CoreOpsTry.branch,
      core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]
  by_cases nodesOver : limits.max_nodes.val < parts.nodes.val.length
  · simp [theory.admit, theory.admit_dimensions, validate, atomsOver, nodesOver,
      core.result.Result.Insts.CoreOpsTry.branch,
      core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]
  by_cases rootsOver : limits.max_roots.val < roots.val.length
  · simp [theory.admit, theory.admit_dimensions, rootsLength, validate, atomsOver, nodesOver, rootsOver,
      core.result.Result.Insts.CoreOpsTry.branch,
      core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]
  by_cases arenaOver : limits.max_operands.val < parts.operands.val.length
  · simp [theory.admit, theory.admit_dimensions, rootsLength, validate, atomsOver, nodesOver, rootsOver, arenaOver,
      core.result.Result.Insts.CoreOpsTry.branch,
      core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]
  cases added : Usize.checked_add atoms 63#usize with
  | none =>
    rw [added] at padded
    have overflow : Usize.max < atoms.val + 63 := by simpa using padded
    simp [theory.admit, theory.admit_dimensions, rootsLength, validate, atomsOver, nodesOver, rootsOver, arenaOver,
      added, overflow, lift, core.result.Result.Insts.CoreOpsTry.branch,
      core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]
  | some total =>
    rw [added] at padded
    have fits : ¬ Usize.max < atoms.val + 63 := by
      have bounded : atoms.val + 63 ≤ Usize.max := by simpa using padded.1
      omega
    simp only [theory.admit, theory.admit_dimensions, rootsLength, validate, atomsOver, nodesOver, rootsOver, arenaOver, fits,
      alloc.vec.Vec.len_val, added, gt_iff_lt, UScalar.lt_equiv, Option.isNone_some,
      core.option.Option.is_none, lift, bind_tc_ok, if_false, counted]
    cases totalCount : result (occurrences parts.nodes.val) with
    | Err reason =>
      simp [core.result.Result.Insts.CoreOpsTry.branch,
        core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]
    | Ok count =>
      by_cases countOver : limits.max_operands.val < count.val
      · simp [core.result.Result.Insts.CoreOpsTry.branch, countOver,
          core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]
      · simp only [core.result.Result.Insts.CoreOpsTry.branch, bind_tc_ok,
          countOver, if_false, viewed, scanned]
        cases rows : scan atoms.val 0 (view parts).operands parts.nodes.val with
        | Err reason =>
          simp [core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]
        | Ok accepted =>
          cases accepted
          simp only [bind_tc_ok, rooted]
          cases rootsChecked : bounds parts.nodes.val.length .Root roots.val with
          | Err reason =>
            simp [core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]
          | Ok accepted => cases accepted; simp

/-- Successful data is the same paired input with an exact recomputed logical
count, within all dimensions, and with every decoded row and root validated. -/
def Accepted (atoms : Usize) (parts : theory.FormulaParts)
    (roots : alloc.vec.Vec Usize) (limits : theory.AdmissionLimits) (data : theory.Data) : Prop :=
  ∃ count : Usize,
    count.val = occurrences parts.nodes.val ∧
    data = { atoms, parts := { parts with occurrences := count }, roots } ∧
    atoms.val ≤ limits.max_atoms.val ∧ parts.nodes.val.length ≤ limits.max_nodes.val ∧
    roots.val.length ≤ limits.max_roots.val ∧ parts.operands.val.length ≤ limits.max_operands.val ∧
    atoms.val + 63 ≤ Usize.max ∧ count.val ≤ limits.max_operands.val ∧
    WellFormed atoms.val (view parts) ∧
    ∀ root ∈ roots.val, root.val < parts.nodes.val.length

/-- The finite verdict accepts exactly the complete contract, without an
assumed successful validator or trusted cached occurrence count. -/
theorem validate_accepts_iff (atoms : Usize) (parts : theory.FormulaParts)
    (roots : alloc.vec.Vec Usize) (limits : theory.AdmissionLimits) (data : theory.Data) :
    validate atoms parts roots limits = .Ok data ↔ Accepted atoms parts roots limits data := by
  unfold validate Accepted
  by_cases atomsOver : limits.max_atoms.val < atoms.val
  · simp [atomsOver, Nat.not_le.mpr atomsOver]
  by_cases nodesOver : limits.max_nodes.val < parts.nodes.val.length
  · simp [atomsOver, nodesOver, Nat.not_le.mpr nodesOver]
  by_cases rootsOver : limits.max_roots.val < roots.val.length
  · simp [atomsOver, nodesOver, rootsOver, Nat.not_le.mpr rootsOver]
  by_cases arenaOver : limits.max_operands.val < parts.operands.val.length
  · simp [atomsOver, nodesOver, rootsOver, arenaOver, Nat.not_le.mpr arenaOver]
  by_cases paddedOver : Usize.max < atoms.val + 63
  · simp [atomsOver, nodesOver, rootsOver, arenaOver, paddedOver, Nat.not_le.mpr paddedOver]
  simp only [atomsOver, nodesOver, rootsOver, arenaOver, paddedOver, if_false,
    Nat.le_of_not_gt atomsOver, Nat.le_of_not_gt nodesOver, Nat.le_of_not_gt rootsOver,
    Nat.le_of_not_gt arenaOver, Nat.le_of_not_gt paddedOver, true_and]
  cases counted : result (occurrences parts.nodes.val) with
  | Err reason =>
    have noCount : ∀ count : Usize, count.val ≠ occurrences parts.nodes.val := by
      intro count exact
      have returned : result (occurrences parts.nodes.val) = .Ok count :=
        (result_accepts_iff _ count).mpr exact
      rw [counted] at returned
      cases returned
    simp [noCount]
  | Ok count =>
    have exactCount : count.val = occurrences parts.nodes.val :=
      (result_accepts_iff _ count).mp counted
    have unique : ∀ other : Usize, other.val = occurrences parts.nodes.val ↔ other = count := by
      intro other
      constructor
      · intro same; exact UScalar.eq_of_val_eq (same.trans exactCount.symm)
      · intro same; simpa only [same] using exactCount
    simp only [unique, exists_eq_left]
    by_cases countOver : limits.max_operands.val < count.val
    · simp [countOver, Nat.not_le.mpr countOver]
    simp only [countOver, if_false, Nat.le_of_not_gt countOver, true_and]
    have rowsContract : scan atoms.val 0 (view parts).operands parts.nodes.val = .Ok () ↔
        WellFormed atoms.val (view parts) := by
      simpa only [WellFormed, view, alloc.vec.Vec.deref, Slice.from_val] using
        scan_accepts_iff atoms.val 0 (view parts).operands parts.nodes.val
    cases rows : scan atoms.val 0 (view parts).operands parts.nodes.val with
    | Err reason =>
      have invalid : ¬ WellFormed atoms.val (view parts) := by
        rw [← rowsContract, rows]; simp
      simp [invalid]
    | Ok accepted =>
      cases accepted
      have valid : WellFormed atoms.val (view parts) := rowsContract.mp rows
      cases rootsChecked : bounds parts.nodes.val.length .Root roots.val with
      | Err reason =>
        have invalid : ¬ ∀ root ∈ roots.val, root.val < parts.nodes.val.length := by
          rw [← bounds_accepts_iff _ .Root roots.val, rootsChecked]; simp
        simp [invalid]
      | Ok accepted =>
        cases accepted
        have rootsValid : ∀ root ∈ roots.val, root.val < parts.nodes.val.length :=
          (bounds_accepts_iff _ .Root roots.val).mp rootsChecked
        simp only [core.result.Result.Ok.injEq, valid, true_and]
        constructor
        · intro same
          exact ⟨same.symm, rootsValid⟩
        · rintro ⟨same, _⟩
          exact same.symm

/-- Actual success is equivalent to the full structural and resource contract. -/
theorem admit_accepts_iff (atoms : Usize) (parts : theory.FormulaParts)
    (roots : alloc.vec.Vec Usize) (limits : theory.AdmissionLimits) (data : theory.Data) :
    theory.admit atoms parts roots limits = ok (.Ok data) ↔
      Accepted atoms parts roots limits data := by
  rw [admit_exact, Result.ok.injEq, validate_accepts_iff]

/-- Actual typed refusals are exactly the finite ordered phase refusals. -/
theorem admit_refuses_iff (atoms : Usize) (parts : theory.FormulaParts)
    (roots : alloc.vec.Vec Usize) (limits : theory.AdmissionLimits)
    (reason : theory.AdmissionError) :
    theory.admit atoms parts roots limits = ok (.Err reason) ↔
      validate atoms parts roots limits = .Err reason := by
  rw [admit_exact, Result.ok.injEq]

/-- A returned admitted value retains complete table structure and roots and
stores the exact occurrence count, independently of its supplied cache value. -/
theorem admitted_structure (atoms : Usize) (parts : theory.FormulaParts)
    (roots : alloc.vec.Vec Usize) (limits : theory.AdmissionLimits) (data : theory.Data)
    (admitted : theory.admit atoms parts roots limits = ok (.Ok data)) :
    WellFormed data.atoms.val (view data.parts) ∧
      (∀ root ∈ data.roots.val, root.val < data.parts.nodes.val.length) ∧
      data.parts.occurrences.val = occurrences data.parts.nodes.val ∧
      data.parts.operands.val.length ≤ limits.max_operands.val ∧
      data.parts.occurrences.val ≤ limits.max_operands.val := by
  obtain ⟨count, counted, same, _, _, _, arenaFits, _, countFits, valid, rootsValid⟩ :=
    (admit_accepts_iff atoms parts roots limits data).mp admitted
  subst data
  exact ⟨valid, rootsValid, counted, arenaFits, countFits⟩

/-- The admitted padded atom count also bounds both packed word counts. -/
theorem admitted_word_counts (atoms : Usize) (parts : theory.FormulaParts)
    (roots : alloc.vec.Vec Usize) (limits : theory.AdmissionLimits) (data : theory.Data)
    (admitted : theory.admit atoms parts roots limits = ok (.Ok data)) :
    data.atoms.val + 31 ≤ Usize.max ∧
      Zetesis.Refinement.PackedInterpretations.count64 data.atoms.val ≤ Usize.max ∧
      Zetesis.Refinement.PackedInterpretations.count32 data.atoms.val ≤ Usize.max := by
  obtain ⟨count, _, same, _, _, _, _, padded, _⟩ :=
    (admit_accepts_iff atoms parts roots limits data).mp admitted
  subst data
  simp only [Zetesis.Refinement.PackedInterpretations.count64, Zetesis.Refinement.PackedInterpretations.count32]
  exact ⟨by omega, (Nat.div_le_self _ _).trans padded,
    (Nat.div_le_self _ _).trans (by omega)⟩

end NativeAdmittedData
