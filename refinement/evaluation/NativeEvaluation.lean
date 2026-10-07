import NativeOperands
import NativeMembership
import NativeStructure

open Aeneas Aeneas.Std Result
open ZetesisNativeExtract
open Zetesis.Refinement

/-!
# Native node dispatch and its work receipt

The generated dispatcher reads the common NodeView, so inline pairs and arena
rows have one evaluation proof. Children must belong to the initialized truth
prefix. The original/frozen distinction is the caller's node mask, outside this
dispatch; this module does not assume that any supplied mask is original truth.
-/
namespace NativeEvaluation

/-- The original Boolean operation of a decoded node over the supplied prefix.
Finite connectives use the existing OperandArena reduction. -/
def value (candidate : theory.Interpretation) (truths : List Bool) : theory.NodeView → Bool
  | .Atom atom => NativeMembership.denotes candidate atom.val
  | .False => false
  | .Implies left right => !(truths[left.val]?.getD false) || truths[right.val]?.getD false
  | .And row => OperandArena.reduce .conj
      (row.val.map (fun child => truths[child.val]?.getD false))
  | .Or row => OperandArena.reduce .disj
      (row.val.map (fun child => truths[child.val]?.getD false))

/-- Every referenced occurrence is charged, including both implication sides.
The containing table loop charges the separate one-unit node visit. -/
def occurrences : theory.NodeView → Nat
  | .Atom _ | .False => 0
  | .Implies _ _ => 2
  | .And row | .Or row => row.val.length

/-- A tick either refuses without work or advances exactly once. Both outcomes
are derived from the generated poll and counter operations. -/
theorem tick_cases (work : oracle.Work) :
    (∃ reason, oracle.Work.tick work = ok (.Err reason, work)) ∨
    (∃ next, oracle.Work.tick work = ok (.Ok (), next) ∧
      NativeOperands.Receipt work next 1) := by
  cases observed : NativeControl.observation work.cancellation with
  | some reason => exact Or.inl ⟨reason, NativeControl.tick_stopped work reason observed⟩
  | none =>
    by_cases remaining : work.statistics.work.val < work.limits.max_work.val
    · obtain ⟨counter, counted, actual⟩ := NativeControl.tick_advances work observed remaining
      exact Or.inr ⟨{ work with statistics := { work.statistics with work := counter } },
        actual, ⟨rfl, rfl, rfl, counted⟩⟩
    · exact Or.inl ⟨.WorkLimit,
        NativeControl.tick_at_limit work observed (Nat.le_of_not_gt remaining)⟩

/-- Native dispatch either returns its exact Boolean value after visiting every
operand, or a typed stop after a strict prefix of operands. The returned work
records every completed tick and preserves the control, limits and subset count.
No budget-sufficiency or assumed dispatcher-result equation is a premise. -/
theorem node_refines (node : theory.NodeView) (candidate : theory.Interpretation)
    (values : Slice Bool) (work : oracle.Work)
    (stored : NativeMembership.Represented candidate)
    (children : NativeStructure.ChildrenBefore values.val.length node) :
    ∃ result returned used,
      oracle.evaluate_node node candidate values work = ok (result, returned) ∧
      NativeOperands.Receipt work returned used ∧ used ≤ occurrences node ∧
      match result with
      | .Ok truth => used = occurrences node ∧ truth = value candidate values.val node
      | .Err _ => used < occurrences node := by
  cases node with
  | Atom atom =>
    refine ⟨.Ok (NativeMembership.denotes candidate atom.val), work, 0, ?_,
      NativeOperands.Receipt.refl work, Nat.le_refl _, rfl, rfl⟩
    simp [oracle.evaluate_node, NativeMembership.contains_refines candidate atom stored]
  | False =>
    exact ⟨.Ok false, work, 0, rfl, NativeOperands.Receipt.refl work,
      Nat.le_refl _, rfl, rfl⟩
  | And row =>
    obtain ⟨result, returned, used, actual, receipt, bounded, boundary⟩ :=
      NativeOperands.evaluate_refines row values true work children
    refine ⟨result, returned, used, actual, receipt, bounded, ?_⟩
    cases result with
    | Err reason => exact boundary
    | Ok truth =>
      exact ⟨boundary.1, boundary.2.trans (NativeOperands.identity_fold values true row.val)⟩
  | Or row =>
    obtain ⟨result, returned, used, actual, receipt, bounded, boundary⟩ :=
      NativeOperands.evaluate_refines row values false work children
    refine ⟨result, returned, used, actual, receipt, bounded, ?_⟩
    cases result with
    | Err reason => exact boundary
    | Ok truth =>
      exact ⟨boundary.1, boundary.2.trans (NativeOperands.identity_fold values false row.val)⟩
  | Implies left right =>
    obtain ⟨antecedent, readLeft, exactLeft⟩ := WP.spec_imp_exists
      (Slice.index_usize_spec values left children.1)
    obtain ⟨consequent, readRight, exactRight⟩ := WP.spec_imp_exists
      (Slice.index_usize_spec values right children.2)
    rcases tick_cases work with ⟨reason, stopped⟩ | ⟨first, tickFirst, receiptFirst⟩
    · refine ⟨.Err reason, work, 0, ?_, NativeOperands.Receipt.refl work,
        by simp [occurrences], by simp [occurrences]⟩
      simp [oracle.evaluate_node, stopped,
        core.result.Result.Insts.CoreOpsTry.branch,
        core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]
    · rcases tick_cases first with ⟨reason, stopped⟩ | ⟨second, tickSecond, receiptSecond⟩
      · refine ⟨.Err reason, first, 1, ?_, receiptFirst, by simp [occurrences], by simp [occurrences]⟩
        simp [oracle.evaluate_node, tickFirst, readLeft, stopped,
          core.result.Result.Insts.CoreOpsTry.branch,
          core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]
      · refine ⟨.Ok (!antecedent || consequent), second, 2, ?_,
          receiptFirst.trans receiptSecond, Nat.le_refl _, rfl, ?_⟩
        · cases antecedent <;>
            simp [oracle.evaluate_node, tickFirst, readLeft, tickSecond, readRight,
              core.result.Result.Insts.CoreOpsTry.branch]
        · simp only [value, exactLeft, exactRight,
            List.getElem?_eq_getElem children.1, List.getElem?_eq_getElem children.2,
            Option.getD_some]

end NativeEvaluation
