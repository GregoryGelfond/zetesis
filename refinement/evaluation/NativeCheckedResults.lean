import Native.Funs

open Aeneas Aeneas.Std Result
open ZetesisNativeExtract

/-!
# Retaining the checked interpretation

These laws concern the actual generated result accessors and consuming
conversion. They preserve the stored subject, verdict and statistics without
another oracle call. Conversion admits exactly the stored Stable verdict; its
other branch retains the whole rejected decision.

A verdict field alone is not proof of membership. The semantic composition must
also establish that the decision came from the actual public check of that
subject. These laws neither permit a supplied verdict to establish membership
nor establish enumeration coverage.
-/
namespace NativeCheckedResults

/-- The candidate accessor returns the exact retained interpretation. -/
theorem candidate_exact (decision : checked.CheckedInterpretation) :
    checked.CheckedInterpretation.impl.candidate decision = ok decision.candidate := by
  rfl

/-- The verdict accessor returns the decision's own verdict. -/
theorem verdict_exact (decision : checked.CheckedInterpretation) :
    checked.CheckedInterpretation.verdict decision = ok decision.check.verdict := by
  rfl

/-- The statistics accessor returns the work recorded by that decision. -/
theorem statistics_exact (decision : checked.CheckedInterpretation) :
    checked.CheckedInterpretation.statistics decision = ok decision.check.statistics := by
  rfl

/-- The actual acceptance accessor returns true exactly for the stored Stable
verdict. This is a representation law, not a membership theorem for an arbitrary
record. -/
theorem accepted_iff (decision : checked.CheckedInterpretation) :
    checked.CheckedInterpretation.accepted decision = ok true ↔
      decision.check.verdict = .Stable := by
  cases verdict : decision.check.verdict <;>
    simp [checked.CheckedInterpretation.accepted, oracle.Check.accepted, verdict]

/-- A successful consuming conversion admits precisely a Stable decision and
retains the exact candidate in the returned typed interpretation.

Proof: unfold the actual acceptance and conversion operations, and distinguish
the three stored verdicts. Only Stable takes the successful branch. -/
theorem conversion_success_iff (decision : checked.CheckedInterpretation)
    (result : checked.StableInterpretation) :
    checked.CheckedInterpretation.into_stable_interpretation decision = ok (.Ok result) ↔
      decision.check.verdict = .Stable ∧ result.interpretation = decision.candidate := by
  cases verdict : decision.check.verdict <;>
    simp [checked.CheckedInterpretation.into_stable_interpretation,
      checked.CheckedInterpretation.accepted, oracle.Check.accepted, verdict]
  cases result
  simp only [checked.StableInterpretation.mk.injEq]
  exact eq_comm

/-- A refused conversion retains the entire rejected decision. It does not
replace the subject, discard the negative verdict, or repeat membership checking.
The proof follows the same three actual verdict branches. -/
theorem conversion_refusal_iff (decision rejected : checked.CheckedInterpretation) :
    checked.CheckedInterpretation.into_stable_interpretation decision = ok (.Err rejected) ↔
      decision.check.verdict ≠ .Stable ∧ rejected = decision := by
  cases verdict : decision.check.verdict <;>
    simp [checked.CheckedInterpretation.into_stable_interpretation,
      checked.CheckedInterpretation.accepted, oracle.Check.accepted, verdict, eq_comm]

/-- The typed interpretation's theory accessor retains the subject's owner and
data; it does not construct another theory. -/
theorem theory_exact (result : checked.StableInterpretation) :
    checked.StableInterpretation.theory result = ok result.interpretation.theory := by
  rfl

/-- Borrowing the typed interpretation exposes the exact retained subject. -/
theorem interpretation_exact (result : checked.StableInterpretation) :
    checked.StableInterpretation.impl.interpretation result = ok result.interpretation := by
  rfl

/-- Consuming the typed interpretation returns that same subject. No evaluation,
word-vector clone or allocation appears in this generated operation. -/
theorem moved_exact (result : checked.StableInterpretation) :
    checked.StableInterpretation.into_interpretation result = ok result.interpretation := by
  rfl

end NativeCheckedResults
