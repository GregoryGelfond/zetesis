import NativeBoundsValidation

open Aeneas Aeneas.Std Result ControlFlow
open ZetesisNativeExtract

/-!
# Exact logical operand accounting

Native admission counts logical child occurrences independently of the physical
operand arena's length. Repeated IDs and overlapping spans therefore remain
separate occurrences. The generated finite scan returns the exact natural sum
when representable, and Limit on overflow; cached input metadata is not read.
-/
namespace NativeOccurrenceCount

/-- The declared logical width of one raw row. Span validity is checked later,
so malformed rows still contribute their declared width to this earlier phase. -/
def width : theory.Node → Usize
  | .Atom _ | .False => 0#usize
  | .Implies _ _ | .AndPair _ | .OrPair _ => 2#usize
  | .AndSpan span | .OrSpan span => span.length

/-- The mathematical sum counts every logical occurrence in the stored table. -/
def occurrences : List theory.Node → Nat
  | [] => 0
  | node :: rest => (width node).val + occurrences rest

/-- The generated per-node count is exactly its declared logical width. -/
theorem node_occurrences_exact (node : theory.Node) :
    theory.Node.occurrences node = ok (width node) := by
  cases node <;> rfl

/-- A fitting natural count has its exact machine representation, without wrap. -/
def machineCount (count : Nat) (fits : count ≤ Usize.max) : Usize :=
  UScalar.ofNatCore count (by
    have positive : 0 < 2 ^ UScalarTy.numBits .Usize := by positivity
    simp only [Usize.max, Usize.numBits] at fits
    exact Nat.lt_of_le_pred positive fits)

/-- Converting a fitting count preserves its natural value. -/
theorem machineCount_val (count : Nat) (fits : count ≤ Usize.max) :
    (machineCount count fits).val = count := by
  rfl

/-- Representable totals are returned exactly; larger totals retain Limit. -/
def result (count : Nat) : core.result.Result Usize theory.AdmissionError :=
  if fits : count ≤ Usize.max then .Ok (machineCount count fits) else .Err .Limit

/-- Every existing machine value fits the same checked conversion. -/
theorem result_value (value : Usize) : result value.val = .Ok value := by
  have fits : value.val ≤ Usize.max := by
    have bounded : value.val < 2 ^ UScalarTy.numBits .Usize := value.hBounds
    simp only [Usize.max, Usize.numBits]
    omega
  rw [result, dif_pos fits]
  congr 1

/-- A successful count is exactly its natural total; its machine bound follows
from the returned scalar, rather than being assumed by the caller. -/
theorem result_accepts_iff (count : Nat) (value : Usize) :
    result count = .Ok value ↔ value.val = count := by
  by_cases fits : count ≤ Usize.max
  · rw [result, dif_pos fits, core.result.Result.Ok.injEq]
    constructor
    · intro same
      rw [← same, machineCount_val]
    · intro same
      apply UScalar.eq_of_val_eq
      rw [machineCount_val, same]
  · have different : value.val ≠ count := by
      intro same
      have bounded : value.val < 2 ^ UScalarTy.numBits .Usize := value.hBounds
      simp only [Usize.max, Usize.numBits] at fits
      omega
    simp [result, fits, different]

/-- Only overflow causes this phase's refusal, and it remains Limit. -/
theorem result_refuses_iff (count : Nat) (reason : theory.AdmissionError) :
    result count = .Err reason ↔ Usize.max < count ∧ .Limit = reason := by
  by_cases fits : count ≤ Usize.max
  · simp [result, fits, Nat.not_lt.mpr fits]
  · simp [result, fits, Nat.lt_of_not_ge fits]

/-- One unfolding of the actual checked-sum loop exposes its actual body. -/
theorem loop_unfold (cursor : core.slice.iter.Iter theory.Node) (count : Usize) :
    theory.count_occurrences_loop cursor count = (do
      let transition ← theory.count_occurrences_loop.body cursor count
      match transition with
      | .done verdict => ok verdict
      | .cont (next, total) => theory.count_occurrences_loop next total) := by
  rw [theory.count_occurrences_loop, loop]
  congr 1
  funext transition
  cases transition with
  | done verdict => rfl
  | cont state => cases state; rfl

/-- The generated sum loop terminates with the exact accumulated suffix total.
Induction decreases the unread node count. A checked-add refusal implies that
the final natural total also overflows; no later row can reduce the sum. -/
theorem loop_exact (cursor : core.slice.iter.Iter theory.Node) (count : Usize)
    (within : cursor.i ≤ cursor.slice.val.length) :
    theory.count_occurrences_loop cursor count =
      ok (result (count.val + occurrences (cursor.slice.val.drop cursor.i))) := by
  have construct (remaining : Nat) :
      ∀ (current : core.slice.iter.Iter theory.Node) (total : Usize),
        current.i ≤ current.slice.val.length →
        current.slice.val.length - current.i = remaining →
        theory.count_occurrences_loop current total =
          ok (result (total.val + occurrences (current.slice.val.drop current.i))) := by
    induction remaining with
    | zero =>
      intro current total _ exhaustedCount
      have exhausted : current.slice.val.length ≤ current.i := by omega
      rw [loop_unfold]
      simp [theory.count_occurrences_loop.body,
        EvaluatorIteration.slice_next_exhausted current exhausted,
        List.drop_eq_nil_of_le exhausted, occurrences, result_value]
    | succ remaining inductionHypothesis =>
      intro current total _ counted
      have inside : current.i < current.slice.val.length := by omega
      have suffix : current.slice.val.drop current.i =
          current.slice.val[current.i] :: current.slice.val.drop (current.i + 1) :=
        List.drop_eq_getElem_cons inside
      have checked : match Usize.checked_add total (width current.slice.val[current.i]) with
          | some next => total.val + (width current.slice.val[current.i]).val ≤ Usize.max ∧
              next.val = total.val + (width current.slice.val[current.i]).val ∧
              next.bv = total.bv + (width current.slice.val[current.i]).bv
          | none => Usize.max < total.val + (width current.slice.val[current.i]).val :=
        Usize.checked_add_bv_spec total (width current.slice.val[current.i])
      rw [loop_unfold, suffix, occurrences]
      cases added : Usize.checked_add total (width current.slice.val[current.i]) with
      | none =>
        rw [added] at checked
        have over : ¬ total.val + ((width current.slice.val[current.i]).val +
            occurrences (current.slice.val.drop (current.i + 1))) ≤ Usize.max := by omega
        simp [theory.count_occurrences_loop.body,
          EvaluatorIteration.slice_next_present current inside, node_occurrences_exact,
          added, lift, core.option.Option.ok_or, result, over,
          core.result.Result.Insts.CoreOpsTry.branch,
          core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]
      | some next =>
        rw [added] at checked
        have continued : theory.count_occurrences_loop { current with i := current.i + 1 } next =
            ok (result (next.val + occurrences (current.slice.val.drop (current.i + 1)))) :=
          inductionHypothesis { current with i := current.i + 1 } next
            (by dsimp only; omega) (by dsimp only; omega)
        simpa [theory.count_occurrences_loop.body,
          EvaluatorIteration.slice_next_present current inside, node_occurrences_exact,
          added, lift, core.option.Option.ok_or, checked.2.1, Nat.add_assoc,
          core.result.Result.Insts.CoreOpsTry.branch] using continued
  exact construct _ cursor count within rfl

/-- Counting a complete raw table returns its exact occurrence sum or Limit.
No assumption about the supplied arena or cached occurrence metadata is needed. -/
theorem count_occurrences_exact (nodes : Slice theory.Node) :
    theory.count_occurrences nodes = ok (result (occurrences nodes.val)) := by
  have started : SharedSlice.Insts.CoreIterTraitsCollectIntoIteratorSharedIter.into_iter nodes =
      ok { slice := nodes, i := 0 } := rfl
  simp only [theory.count_occurrences, started, bind_tc_ok]
  simpa only [List.drop_zero, UScalar.ofNatCore_val_eq, Nat.zero_add] using
    loop_exact { slice := nodes, i := 0 } 0#usize (Nat.zero_le _)

/-- Complete successful counting returns precisely the logical operand total. -/
theorem count_occurrences_accepts_iff (nodes : Slice theory.Node) (count : Usize) :
    theory.count_occurrences nodes = ok (.Ok count) ↔ count.val = occurrences nodes.val := by
  rw [count_occurrences_exact, Result.ok.injEq, result_accepts_iff]

end NativeOccurrenceCount
