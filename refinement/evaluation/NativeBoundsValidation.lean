import NativeStructure
import Iteration

open Aeneas Aeneas.Std Result ControlFlow
open ZetesisNativeExtract

/-!
# Complete native child and root validation

The actual generated child loops and root loop perform the same finite ordered
bound check, with different typed refusals. Their bodies are identified with
one common step, whose termination follows from the remaining slice length.
The successful verdict requires every occurrence to fit; repeated IDs and empty
rows remain valid. No getter agreement or successful validation is assumed.
-/
namespace NativeBoundsValidation

/-- Check every occurrence in order, stopping at the first out-of-range ID. -/
def bounds (limit : Nat) (reason : theory.AdmissionError) :
    List Usize → core.result.Result Unit theory.AdmissionError
  | [] => .Ok ()
  | entry :: rest => if entry.val < limit then bounds limit reason rest else .Err reason

/-- The finite scan accepts precisely when every occurrence lies below the bound. -/
theorem bounds_accepts_iff (limit : Nat) (reason : theory.AdmissionError)
    (entries : List Usize) :
    bounds limit reason entries = .Ok () ↔ ∀ entry ∈ entries, entry.val < limit := by
  induction entries with
  | nil => simp [bounds]
  | cons entry rest inductionHypothesis =>
    by_cases inside : entry.val < limit <;> simp [bounds, inside, inductionHypothesis]

/-- A refusal identifies the supplied refusal kind and an out-of-range occurrence. -/
theorem bounds_refuses_iff (limit : Nat) (reason refused : theory.AdmissionError)
    (entries : List Usize) :
    bounds limit reason entries = .Err refused ↔
      reason = refused ∧ ∃ entry ∈ entries, limit ≤ entry.val := by
  induction entries with
  | nil => simp [bounds]
  | cons entry rest inductionHypothesis =>
    by_cases inside : entry.val < limit
    · simp [bounds, inside, inductionHypothesis, Nat.not_le.mpr inside]
    · have outside : limit ≤ entry.val := Nat.le_of_not_gt inside
      simp [bounds, inside, outside]

/-- The shared step reads the actual slice iterator and preserves its next
cursor. Its only early completion is the supplied structural refusal. -/
def body (limit : Usize) (reason : theory.AdmissionError)
    (cursor : core.slice.iter.Iter Usize) :
    Result (ControlFlow (core.slice.iter.Iter Usize)
      (core.result.Result Unit theory.AdmissionError)) := do
  let (value, next) ← core.slice.iter.IteratorSliceIter.next cursor
  match value with
  | none => ok (.done (.Ok ()))
  | some entry =>
    if entry.val < limit.val then ok (.cont next) else ok (.done (.Err reason))

/-- The actual imported loop operator applied to the common checked step. -/
def run (limit : Usize) (reason : theory.AdmissionError)
    (cursor : core.slice.iter.Iter Usize) :
    Result (core.result.Result Unit theory.AdmissionError) :=
  loop (body limit reason) cursor

/-- One unfolding exposes precisely one checked step. -/
theorem run_unfold (limit : Usize) (reason : theory.AdmissionError)
    (cursor : core.slice.iter.Iter Usize) :
    run limit reason cursor = (do
      let transition ← body limit reason cursor
      match transition with
      | .done result => ok result
      | .cont next => run limit reason next) := by
  rw [run, loop]
  congr 1
  funext transition
  cases transition <;> rfl

/-- The imported loop always returns the exact finite suffix verdict.
Induction on the remaining length discharges termination, including refusal. -/
theorem run_exact (limit : Usize) (reason : theory.AdmissionError)
    (cursor : core.slice.iter.Iter Usize)
    (within : cursor.i ≤ cursor.slice.val.length) :
    run limit reason cursor =
      ok (bounds limit.val reason (cursor.slice.val.drop cursor.i)) := by
  have construct (remaining : Nat) :
      ∀ current : core.slice.iter.Iter Usize,
        current.i ≤ current.slice.val.length →
        current.slice.val.length - current.i = remaining →
        run limit reason current =
          ok (bounds limit.val reason (current.slice.val.drop current.i)) := by
    induction remaining with
    | zero =>
      intro current _ exhaustedCount
      have exhausted : current.slice.val.length ≤ current.i := by omega
      rw [run_unfold]
      simp [body, EvaluatorIteration.slice_next_exhausted current exhausted,
        List.drop_eq_nil_of_le exhausted, bounds]
    | succ remaining inductionHypothesis =>
      intro current _ counted
      have inside : current.i < current.slice.val.length := by omega
      have suffix : current.slice.val.drop current.i =
          current.slice.val[current.i] :: current.slice.val.drop (current.i + 1) :=
        List.drop_eq_getElem_cons inside
      rw [run_unfold]
      simp only [body, EvaluatorIteration.slice_next_present current inside, bind_tc_ok]
      rw [suffix]
      by_cases bounded : current.slice.val[current.i].val < limit.val
      · have continued : run limit reason { current with i := current.i + 1 } =
            ok (bounds limit.val reason (current.slice.val.drop (current.i + 1))) :=
          inductionHypothesis { current with i := current.i + 1 }
            (by dsimp only; omega) (by dsimp only; omega)
        simpa [bounded, bounds] using continued
      · simp [bounded, bounds]
  exact construct _ cursor within rfl

/-- The generated conjunction child step is the common Edge-refusing step. -/
theorem children_body0 (index : Usize) (cursor : core.slice.iter.Iter Usize) :
    theory.validate_children_loop0.body index cursor = body index .Edge cursor := by
  unfold theory.validate_children_loop0.body body
  congr 1
  funext outcome
  rcases outcome with ⟨entry, next⟩
  cases entry with
  | none => rfl
  | some child =>
    by_cases inside : child.val < index.val <;> simp [UScalar.le_equiv, inside]

/-- The disjunction child step has the same complete occurrence contract. -/
theorem children_body1 (index : Usize) (cursor : core.slice.iter.Iter Usize) :
    theory.validate_children_loop1.body index cursor = body index .Edge cursor := by
  exact children_body0 index cursor

/-- A generated root step uses the same scan with Root as its refusal kind. -/
theorem roots_body (count : Usize) (cursor : core.slice.iter.Iter Usize) :
    theory.validate_roots_loop.body count cursor = body count .Root cursor := by
  unfold theory.validate_roots_loop.body body
  congr 1
  funext outcome
  rcases outcome with ⟨entry, next⟩
  cases entry with
  | none => rfl
  | some root =>
    by_cases outside : count.val ≤ root.val
    · have inside : ¬ root.val < count.val := Nat.not_lt.mpr outside
      simp [theory.validate_root, UScalar.le_equiv, outside, inside,
        core.result.Result.Insts.CoreOpsTry.branch,
        core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]
    · have inside : root.val < count.val := Nat.lt_of_not_ge outside
      simp [theory.validate_root, UScalar.le_equiv, outside, inside,
        core.result.Result.Insts.CoreOpsTry.branch]

/-- The generated conjunction loop checks its entire remaining child sequence. -/
theorem children_loop0_exact (index : Usize) (cursor : core.slice.iter.Iter Usize)
    (within : cursor.i ≤ cursor.slice.val.length) :
    theory.validate_children_loop0 cursor index =
      ok (bounds index.val .Edge (cursor.slice.val.drop cursor.i)) := by
  have sameBody : theory.validate_children_loop0.body index = body index .Edge :=
    funext (children_body0 index)
  simpa only [theory.validate_children_loop0, sameBody, run] using
    run_exact index .Edge cursor within

/-- The generated disjunction loop checks its entire remaining child sequence. -/
theorem children_loop1_exact (index : Usize) (cursor : core.slice.iter.Iter Usize)
    (within : cursor.i ≤ cursor.slice.val.length) :
    theory.validate_children_loop1 cursor index =
      ok (bounds index.val .Edge (cursor.slice.val.drop cursor.i)) := by
  have sameBody : theory.validate_children_loop1.body index = body index .Edge :=
    funext (children_body1 index)
  simpa only [theory.validate_children_loop1, sameBody, run] using
    run_exact index .Edge cursor within

/-- The generated root scan always returns its exact finite ordered verdict. -/
theorem validate_roots_exact (count : Usize) (roots : Slice Usize) :
    theory.validate_roots count roots = ok (bounds count.val .Root roots.val) := by
  have sameBody : theory.validate_roots_loop.body count = body count .Root :=
    funext (roots_body count)
  have started : SharedSlice.Insts.CoreIterTraitsCollectIntoIteratorSharedIter.into_iter roots =
      ok { slice := roots, i := 0 } := rfl
  simp only [theory.validate_roots, started, bind_tc_ok,
    theory.validate_roots_loop, sameBody]
  simpa only [run, List.drop_zero] using
    run_exact count .Root { slice := roots, i := 0 } (Nat.zero_le _)

/-- Actual root acceptance is precisely complete root coverage within the table. -/
theorem validate_roots_accepts_iff (count : Usize) (roots : Slice Usize) :
    theory.validate_roots count roots = ok (.Ok ()) ↔
      ∀ root ∈ roots.val, root.val < count.val := by
  rw [validate_roots_exact, Result.ok.injEq, bounds_accepts_iff]

/-- A root refusal is exactly an out-of-table root and retains its typed kind. -/
theorem validate_roots_refuses_iff (count : Usize) (roots : Slice Usize)
    (reason : theory.AdmissionError) :
    theory.validate_roots count roots = ok (.Err reason) ↔
      .Root = reason ∧ ∃ root ∈ roots.val, count.val ≤ root.val := by
  rw [validate_roots_exact, Result.ok.injEq, bounds_refuses_iff]

end NativeBoundsValidation
