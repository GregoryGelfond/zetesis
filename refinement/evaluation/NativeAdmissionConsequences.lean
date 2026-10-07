import NativeAdmittedData

open Aeneas Aeneas.Std Result
open ZetesisNativeExtract
open NativeStructure NativeAdmissionValidation NativeBoundsValidation NativeOccurrenceCount

/-!
# First refusals and complete native widths

These consequences expose first-failure location and exact decoded width from
actual admission. They use the same paired rows and ordered scans, rather than
constructing a second semantic table or accepting metadata on trust.
-/
namespace NativeAdmissionConsequences

/-- Passing a prefix advances precisely by its stored node count. -/
theorem scan_valid_prefix (atoms first : Nat) (arena : Slice Usize)
    (passed suffix : List theory.Node) (valid : ValidNodes atoms first arena passed) :
    scan atoms first arena (passed ++ suffix) =
      scan atoms (first + passed.length) arena suffix := by
  induction passed generalizing first with
  | nil => simp
  | cons entry rest inductionHypothesis =>
    obtain ⟨head, tail⟩ := (validNodes_cons atoms first arena entry rest).mp valid
    have accepted : storedCheck atoms first arena entry = .Ok () :=
      (storedCheck_accepts_iff atoms first arena entry).mpr head
    have continued : scan atoms (first + 1) arena (rest ++ suffix) =
        scan atoms ((first + 1) + rest.length) arena suffix :=
      inductionHypothesis (first + 1) tail
    have offset : (first + 1) + rest.length = first + (rest.length + 1) := by omega
    rw [offset] at continued
    simpa only [List.cons_append, scan, accepted, List.length_cons] using continued

/-- Every refusal identifies a valid prefix and its first refused row. The
position remains relative to the original table, including repeated nodes. -/
theorem scan_first_refusal (atoms first : Nat) (arena : Slice Usize)
    (nodes : List theory.Node) (reason : theory.AdmissionError) :
    scan atoms first arena nodes = .Err reason ↔
      ∃ passed entry suffix,
        nodes = passed ++ entry :: suffix ∧ ValidNodes atoms first arena passed ∧
        storedCheck atoms (first + passed.length) arena entry = .Err reason := by
  constructor
  · intro refused
    induction nodes generalizing first with
    | nil => simp [scan] at refused
    | cons entry rest inductionHypothesis =>
      cases checked : storedCheck atoms first arena entry with
      | Err found =>
        have same : found = reason := by simpa [scan, checked] using refused
        refine ⟨[], entry, rest, rfl, validNodes_nil atoms first arena, ?_⟩
        simpa only [List.length_nil, Nat.add_zero, same] using checked
      | Ok accepted =>
        cases accepted
        have later : scan atoms (first + 1) arena rest = .Err reason := by
          simpa only [scan, checked] using refused
        obtain ⟨passed, failed, suffix, shape, valid, rejected⟩ :=
          inductionHypothesis (first + 1) later
        refine ⟨entry :: passed, failed, suffix, ?_, ?_, ?_⟩
        · simp only [List.cons_append, shape]
        · exact (validNodes_cons atoms first arena entry passed).mpr
            ⟨(storedCheck_accepts_iff atoms first arena entry).mp checked, valid⟩
        · simpa only [List.length_cons, Nat.add_assoc, Nat.add_left_comm, Nat.add_comm] using rejected
  · rintro ⟨passed, entry, suffix, shape, valid, refused⟩
    rw [shape, scan_valid_prefix atoms first arena passed (entry :: suffix) valid, scan, refused]

/-- A bounds scan can return only its explicitly supplied refusal. -/
theorem bounds_ne_allocation (limit : Nat) (reason : theory.AdmissionError)
    (different : reason ≠ .Allocation) (entries : List Usize) :
    bounds limit reason entries ≠ .Err .Allocation := by
  intro refused
  exact different ((bounds_refuses_iff limit reason .Allocation entries).mp refused).1

/-- Native storage decoding can refuse arity or span, never allocation. -/
theorem storedView_ne_allocation (arena : Slice Usize) (node : theory.Node) :
    NativeRows.storedView arena node ≠ .Err .Allocation := by
  cases node with
  | Atom atom => simp [NativeRows.storedView]
  | False => simp [NativeRows.storedView]
  | Implies left right => simp [NativeRows.storedView]
  | AndPair pair => simp [NativeRows.storedView]
  | OrPair pair => simp [NativeRows.storedView]
  | AndSpan span =>
    by_cases short : span.length.val < 3
    · simp [NativeRows.storedView, NativeRows.spanResult, short]
    · by_cases fits : span.start.val + span.length.val ≤ arena.val.length <;>
        simp [NativeRows.storedView, NativeRows.spanResult, short, fits]
  | OrSpan span =>
    by_cases short : span.length.val < 3
    · simp [NativeRows.storedView, NativeRows.spanResult, short]
    · by_cases fits : span.start.val + span.length.val ≤ arena.val.length <;>
        simp [NativeRows.storedView, NativeRows.spanResult, short, fits]

/-- The one-row structural phase never reports allocation refusal. -/
theorem storedCheck_ne_allocation (atoms position : Nat) (arena : Slice Usize)
    (node : theory.Node) : storedCheck atoms position arena node ≠ .Err .Allocation := by
  cases decoded : NativeRows.storedView arena node with
  | Err reason =>
    have different : reason ≠ .Allocation := by
      intro same
      exact storedView_ne_allocation arena node (same ▸ decoded)
    simp [storedCheck, decoded, different]
  | Ok view =>
    simp only [storedCheck, decoded]
    cases view with
    | Atom atom => simp [viewCheck]; split <;> simp
    | False => simp [viewCheck, children, bounds]
    | Implies left right => exact bounds_ne_allocation position .Edge (by simp) _
    | And row => exact bounds_ne_allocation position .Edge (by simp) _
    | Or row => exact bounds_ne_allocation position .Edge (by simp) _

/-- No finite node scan invents an allocation refusal. -/
theorem scan_ne_allocation (atoms first : Nat) (arena : Slice Usize)
    (nodes : List theory.Node) : scan atoms first arena nodes ≠ .Err .Allocation := by
  intro refused
  obtain ⟨passed, entry, _, _, _, failed⟩ :=
    (scan_first_refusal atoms first arena nodes .Allocation).mp refused
  exact storedCheck_ne_allocation atoms (first + passed.length) arena entry failed

/-- Admission's complete finite computation never reports Allocation. External
allocation belongs to the later constructor phase and its outer result. -/
theorem admit_never_allocation (atoms : Usize) (parts : theory.FormulaParts)
    (roots : alloc.vec.Vec Usize) (limits : theory.AdmissionLimits) :
    theory.admit atoms parts roots limits ≠ ok (.Err .Allocation) := by
  rw [NativeAdmittedData.admit_exact]
  intro refused
  have verdict : NativeAdmittedData.validate atoms parts roots limits = .Err .Allocation :=
    by simpa only [Result.ok.injEq] using refused
  unfold NativeAdmittedData.validate at verdict
  split at verdict <;> try cases verdict
  split at verdict <;> try cases verdict
  split at verdict <;> try cases verdict
  split at verdict <;> try cases verdict
  split at verdict <;> try cases verdict
  cases counted : result (occurrences parts.nodes.val) with
  | Err reason =>
    simp only [counted, core.result.Result.Err.injEq] at verdict
    have impossible : Usize.max < occurrences parts.nodes.val ∧ .Limit = reason :=
      (result_refuses_iff _ reason).mp counted
    cases verdict ▸ impossible.2
  | Ok count =>
    simp only [counted] at verdict
    split at verdict <;> try cases verdict
    cases rows : scan atoms.val 0 (NativeAdmittedData.view parts).operands parts.nodes.val with
    | Err reason =>
      simp only [rows, core.result.Result.Err.injEq] at verdict
      exact scan_ne_allocation atoms.val 0 (NativeAdmittedData.view parts).operands
        parts.nodes.val (verdict ▸ rows)
    | Ok accepted =>
      cases accepted
      simp only [rows] at verdict
      cases rootsChecked : bounds parts.nodes.val.length .Root roots.val with
      | Err reason =>
        simp only [rootsChecked, core.result.Result.Err.injEq] at verdict
        exact bounds_ne_allocation parts.nodes.val.length .Root (by simp)
          roots.val (verdict ▸ rootsChecked)
      | Ok accepted => simp [rootsChecked] at verdict

/-- Successful decoding has the complete declared logical width, including
both inline entries and every wide occurrence, without sorting or deduplication. -/
theorem decoded_width (arena : Slice Usize) (raw : theory.Node) (view : theory.NodeView)
    (decoded : NativeRows.storedView arena raw = .Ok view) :
    (children view).length = (width raw).val := by
  cases raw with
  | Atom atom => cases decoded; rfl
  | False => cases decoded; rfl
  | Implies left right => cases decoded; rfl
  | AndPair pair =>
    simp only [NativeRows.storedView, core.result.Result.Ok.injEq] at decoded
    rw [← decoded]
    simp [children, width]
  | OrPair pair =>
    simp only [NativeRows.storedView, core.result.Result.Ok.injEq] at decoded
    rw [← decoded]
    simp [children, width]
  | AndSpan span =>
    by_cases short : span.length.val < 3
    · simp [NativeRows.storedView, NativeRows.spanResult, short] at decoded
    · by_cases fits : span.start.val + span.length.val ≤ arena.val.length
      · simp only [NativeRows.storedView, NativeRows.spanResult, short, if_false,
          fits, if_true, core.result.Result.Ok.injEq] at decoded
        rw [← decoded]
        exact NativeRows.row_length arena span fits
      · simp [NativeRows.storedView, NativeRows.spanResult, short, fits] at decoded
  | OrSpan span =>
    by_cases short : span.length.val < 3
    · simp [NativeRows.storedView, NativeRows.spanResult, short] at decoded
    · by_cases fits : span.start.val + span.length.val ≤ arena.val.length
      · simp only [NativeRows.storedView, NativeRows.spanResult, short, if_false,
          fits, if_true, core.result.Result.Ok.injEq] at decoded
        rw [← decoded]
        exact NativeRows.row_length arena span fits
      · simp [NativeRows.storedView, NativeRows.spanResult, short, fits] at decoded

end NativeAdmissionConsequences
