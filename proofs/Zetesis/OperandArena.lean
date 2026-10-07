import Zetesis.AdjacencyRows
import Zetesis.IndexedEvaluation
import Zetesis.NormalFerraris

/-!
# Checked operand spans for finite formula connectives

A conjunction or disjunction reads one consecutive span of child indices from
an immutable arena. Admission checks both the entire span and every backward
reference. Checked reads preserve operand order and repeated references; an
empty conjunction is true and an empty disjunction false. An unavailable span
or child is an incomplete read, never a completed Boolean value.

The layer correspondence uses the existing finite-connective Ferraris laws.
Original child values produce original truth; frozen child values produce the
explicit reduct's truth for arbitrary outer and tested interpretations. No
subset assumption is needed. These are mathematical representation laws over
lists and natural indices. They do not verify a Rust arena, machine addition,
allocation, interning, resource accounting, traversal cost or GPU execution.
The topological table fold and concrete extraction remain separate obligations.
-/

namespace Zetesis.Refinement.OperandArena

universe u v
variable {α : Type u} {Entry : Type v}
open Ferraris TightEvaluation

/-- A consecutive arena range, including an empty range at the arena's end. -/
structure Span where
  start : Nat
  length : Nat
  deriving DecidableEq

/-- The whole span must exist; truncating a malformed range is not admission. -/
abbrev Fits (size : Nat) (span : Span) : Prop := span.start + span.length ≤ size

/-- The exact ordered entries named by the span, once its bounds are admitted. -/
def contents (entries : List Entry) (span : Span) : List Entry :=
  AdjacencyRows.slice entries span.start span.length

/-- Check range bounds before returning any entries. Arithmetic is mathematical
natural arithmetic here; a machine implementation must check addition overflow. -/
def read (entries : List Entry) (span : Span) : Option (List Entry) :=
  if Fits entries.length span then some (contents entries span) else none

/-- Successful range reads are exactly the admitted, untruncated ordered span. -/
theorem read_exact (entries : List Entry) (span : Span) (result : List Entry) :
    read entries span = some result ↔
      Fits entries.length span ∧ contents entries span = result := by
  by_cases fits : Fits entries.length span <;> simp [read, fits]

/-- An admitted span contains exactly the requested number of occurrences. -/
theorem contents_length (entries : List Entry) (span : Span)
    (fits : Fits entries.length span) : (contents entries span).length = span.length := by
  simp only [contents, AdjacencyRows.slice, List.length_take, List.length_drop]
  apply Nat.min_eq_left
  unfold Fits at fits
  omega

/-- Allocating a consecutive operand row recovers that row exactly, including
its order, duplicates and empty case, regardless of surrounding arena entries. -/
theorem read_appended (before operands after : List Entry) :
    read (before ++ operands ++ after) ⟨before.length, operands.length⟩ = some operands := by
  apply (read_exact _ _ _).mpr
  constructor
  · simp [Fits]
  · simp [contents, AdjacencyRows.slice, List.append_assoc]

/-- Every occurrence in the range names an already evaluated node. Repeated
indices are admitted independently; no uniqueness requirement is imposed. -/
def Admitted (arena : List Nat) (earlier : Nat) (span : Span) : Prop :=
  Fits arena.length span ∧ ∀ index ∈ contents arena span, index < earlier

/-- Check range bounds and backward references before returning the child row. -/
def validate (arena : List Nat) (earlier : Nat) (span : Span) : Option (List Nat) := do
  let operands ← read arena span
  if operands.all (fun index => decide (index < earlier)) then some operands else none

/-- The executable admission check accepts exactly the complete backward row.
A failed span or a self/forward edge yields no admitted operand sequence. -/
theorem validate_exact (arena : List Nat) (earlier : Nat) (span : Span) (result : List Nat) :
    validate arena earlier span = some result ↔
      Admitted arena earlier span ∧ contents arena span = result := by
  by_cases fits : Fits arena.length span
  · by_cases backwards : ∀ index ∈ contents arena span, index < earlier
    · simp [validate, read, fits, Admitted, List.all_eq_true]
    · simp [validate, read, fits, Admitted, List.all_eq_true, backwards]
  · simp [validate, read, fits, Admitted]

/-- Gather child values in operand order. An absent entry stops the read;
it is not replaced by the identity or by false. -/
def gather (earlier : List Entry) : List Nat → Option (List Entry)
  | [] => some []
  | index :: rest => do
    let value ← earlier[index]?
    let values ← gather earlier rest
    return value :: values

/-- Backward references make every checked child lookup succeed. The fallback
only describes a total reference expression; it is never read on admitted input. -/
theorem gather_exact (earlier : List Entry) (fallback : Entry) (operands : List Nat)
    (backwards : ∀ index ∈ operands, index < earlier.length) :
    gather earlier operands = some (operands.map (fun index => earlier.getD index fallback)) := by
  induction operands with
  | nil => rfl
  | cons index rest inductionHypothesis =>
    have inside : index < earlier.length := backwards index (by simp)
    have restBackwards : ∀ child ∈ rest, child < earlier.length := by
      intro child member
      exact backwards child (by simp [member])
    simp [gather, List.getElem?_eq_getElem inside, inductionHypothesis restBackwards,
      List.getD_eq_getElem?_getD]

/-- The two finite connective kinds share the same checked arena read. -/
inductive Connective where
  | conj
  | disj
  deriving DecidableEq

/-- Reduce the ordered child truths, with the ordinary empty identities. -/
def reduce : Connective → List Bool → Bool
  | .conj, values => values.all id
  | .disj, values => values.any id

/-- A finite disjunction uses its first operand directly, then appends the
remaining operands on the left. No identity leaf is added to a nonempty group;
in particular, a pair decodes to exactly the existing binary constructor. -/
def disjunction : List (Formula α) → Formula α
  | [] => .bot
  | first :: rest => rest.foldl Formula.disj first

/-- A disjunctive fold holds exactly when its initial formula or one of the
appended formulas holds. Operand repetition does not change this statement. -/
theorem satisfies_disjunction_fold (M : Atoms α) (rest : List (Formula α))
    (first : Formula α) :
    Satisfies M (rest.foldl Formula.disj first) ↔
      Satisfies M first ∨ ∃ F ∈ rest, Satisfies M F := by
  induction rest generalizing first with
  | nil => simp
  | cons next rest inductionHypothesis =>
    simp only [List.foldl_cons, inductionHypothesis, Satisfies,
      List.mem_cons, exists_eq_or_imp]
    exact or_assoc

/-- The same disjunctive fold law holds for every frozen reduct, even when the
tested interpretation is not a subset of the outer candidate. -/
theorem reduct_disjunction_fold (M J : Atoms α) (rest : List (Formula α))
    (first : Formula α) :
    Satisfies J (Reduct M (rest.foldl Formula.disj first)) ↔
      Satisfies J (Reduct M first) ∨ ∃ F ∈ rest, Satisfies J (Reduct M F) := by
  induction rest generalizing first with
  | nil => simp
  | cons next rest inductionHypothesis =>
    simp only [List.foldl_cons, inductionHypothesis, RuleFactorization.reduct_disj,
      List.mem_cons, exists_eq_or_imp]
    exact or_assoc

/-- A decoded finite disjunction holds exactly when an operand holds. -/
theorem satisfies_disjunction (M : Atoms α) (operands : List (Formula α)) :
    Satisfies M (disjunction operands) ↔ ∃ F ∈ operands, Satisfies M F := by
  cases operands with
  | nil => simp [disjunction, Satisfies]
  | cons first rest => simp [disjunction, satisfies_disjunction_fold]

/-- Decoding a finite disjunction preserves each operand's frozen-reduct truth. -/
theorem reduct_disjunction (M J : Atoms α) (operands : List (Formula α)) :
    Satisfies J (Reduct M (disjunction operands)) ↔
      ∃ F ∈ operands, Satisfies J (Reduct M F) := by
  cases operands with
  | nil => simp [disjunction, Reduct, Satisfies]
  | cons first rest => simp [disjunction, reduct_disjunction_fold]

/-- Finite connectives decode without an identity leaf on nonempty groups. -/
def formula : Connective → List (Formula α) → Formula α
  | .conj, operands => NormalFerraris.conjunction operands
  | .disj, operands => disjunction operands

/-- A two-operand group retains the exact binary syntax, not merely equivalent
truth. This is the embedding needed by syntax-sensitive class certificates. -/
theorem formula_pair (kind : Connective) (left right : Formula α) :
    formula kind [left, right] = match kind with
      | .conj => .conj left right
      | .disj => .disj left right := by
  cases kind <;> rfl

/-- A single-operand group introduces no connective in the decoded formula. -/
theorem formula_singleton (kind : Connective) (operand : Formula α) :
    formula kind [operand] = operand := by
  cases kind <;> rfl

/-- Decode the range against the already unfolded child formulas. -/
def meaning (kind : Connective) (earlier : List (Formula α))
    (arena : List Nat) (span : Span) : Formula α :=
  formula kind ((contents arena span).map (fun index => earlier.getD index .bot))

/-- Admit the entire operand range, gather each occurrence, then reduce it.
Admission is independent of Boolean short circuiting, including empty identities. -/
def evaluate (kind : Connective) (earlier : List Bool)
    (arena : List Nat) (span : Span) : Option Bool := do
  let operands ← validate arena earlier.length span
  let values ← gather earlier operands
  return reduce kind values

/-- An admitted layer's checked reduction equals the ordered reference reads. -/
theorem evaluate_exact (kind : Connective) (earlier : List Bool)
    (arena : List Nat) (span : Span) (admitted : Admitted arena earlier.length span) :
    evaluate kind earlier arena span =
      some (reduce kind ((contents arena span).map (fun index => earlier.getD index false))) := by
  have admittedRow : validate arena earlier.length span = some (contents arena span) := by
    exact (validate_exact arena earlier.length span (contents arena span)).mpr ⟨admitted, rfl⟩
  have readValues : gather earlier (contents arena span) =
      some ((contents arena span).map (fun index => earlier.getD index false)) := by
    exact gather_exact earlier false (contents arena span) admitted.2
  simp [evaluate, admittedRow, readValues]

/-- Mapping an admitted entry table to Boolean values commutes with gathering
its ordered operand row and reducing it. This law applies to any entry type and
chosen fallback: every referenced entry exists, so the fallback is never read.
Repeated references and the empty-connective identities are preserved. -/
theorem evaluate_map (kind : Connective) (earlier : List Entry) (fallback : Entry)
    (value : Entry → Bool) (arena : List Nat) (span : Span)
    (admitted : Admitted arena earlier.length span) :
    evaluate kind (earlier.map value) arena span =
      some (reduce kind (((contents arena span).map
        (fun index => earlier.getD index fallback)).map value)) := by
  have mappedAdmission : Admitted arena (earlier.map value).length span := by
    simpa only [List.length_map] using admitted
  rw [evaluate_exact kind _ arena span mappedAdmission]
  congr 2
  rw [List.map_map]
  apply List.map_congr_left
  intro index member
  have inside : index < earlier.length := by
    exact admitted.2 index member
  simp [List.getD_eq_getElem?_getD, List.getElem?_map,
    List.getElem?_eq_getElem inside]

/-- Original child truths reduce to the exact original truth of the decoded
finite connective. Existing finite conjunction/disjunction laws discharge the
semantic step; range and child safety come from the checked-read lemmas. -/
theorem original_exact (kind : Connective) (earlier : List (Formula α))
    (candidate : α → Bool) (arena : List Nat) (span : Span)
    (admitted : Admitted arena earlier.length span) :
    evaluate kind (earlier.map (formulaValue candidate)) arena span =
      some (formulaValue candidate (meaning kind earlier arena span)) := by
  rw [evaluate_map kind earlier .bot (formulaValue candidate) arena span admitted]
  apply congrArg some
  apply Bool.eq_iff_iff.mpr
  cases kind with
  | conj =>
    simp only [reduce, meaning, formula, List.all_map, List.all_eq_true,
      Function.comp_apply, id_eq, formula_value_true,
      NormalFerraris.satisfies_conjunction, List.forall_mem_map]
  | disj =>
    simp only [reduce, meaning, formula, List.any_map, List.any_eq_true,
      Function.comp_apply, id_eq, formula_value_true, satisfies_disjunction]
    constructor
    · rintro ⟨index, member, holds⟩
      exact ⟨_, List.mem_map.mpr ⟨index, member, rfl⟩, holds⟩
    · rintro ⟨child, member, holds⟩
      obtain ⟨index, sourceMember, rfl⟩ := List.mem_map.mp member
      exact ⟨index, sourceMember, holds⟩

/-- Frozen child truths reduce to the exact truth of the explicit Ferraris
reduct in any tested interpretation. Duplicates and empty ranges are included;
the theorem does not restrict the tested interpretation to the outer candidate. -/
theorem reduct_exact (kind : Connective) (earlier : List (Formula α))
    (outer tested : α → Bool) (arena : List Nat) (span : Span)
    (admitted : Admitted arena earlier.length span) :
    evaluate kind (earlier.map (fun child => formulaValue tested
        (Reduct (interpretation outer) child))) arena span =
      some (formulaValue tested (Reduct (interpretation outer)
        (meaning kind earlier arena span))) := by
  rw [evaluate_map kind earlier .bot _ arena span admitted]
  apply congrArg some
  apply Bool.eq_iff_iff.mpr
  cases kind with
  | conj =>
    simp only [reduce, meaning, formula, List.all_map, List.all_eq_true,
      Function.comp_apply, id_eq, formula_value_true,
      NormalFerraris.reduct_conjunction, List.forall_mem_map]
  | disj =>
    simp only [reduce, meaning, formula, List.any_map, List.any_eq_true,
      Function.comp_apply, id_eq, formula_value_true, reduct_disjunction]
    constructor
    · rintro ⟨index, member, holds⟩
      exact ⟨_, List.mem_map.mpr ⟨index, member, rfl⟩, holds⟩
    · rintro ⟨child, member, holds⟩
      obtain ⟨index, sourceMember, rfl⟩ := List.mem_map.mp member
      exact ⟨index, sourceMember, holds⟩

/-- Refusing a span or backward-reference check cannot become a completed false
answer, even when Boolean reduction could otherwise short circuit. -/
theorem evaluate_rejected (kind : Connective) (earlier : List Bool)
    (arena : List Nat) (span : Span) (rejected : ¬ Admitted arena earlier.length span) :
    evaluate kind earlier arena span = none := by
  have noRow : validate arena earlier.length span = none := by
    cases result : validate arena earlier.length span with
    | none => rfl
    | some operands =>
      exact False.elim (rejected ((validate_exact arena earlier.length span operands).mp result).1)
  simp [evaluate, noRow]

/-- After reading a connective's frozen children, read its own original truth
from the immutable mask. The completed prefix length is the current node index,
as in `IndexedEvaluation`; a missing frozen bit remains an incomplete read. -/
def evaluateFrozen (kind : Connective) (earlier : List Bool)
    (arena : List Nat) (span : Span) (frozen : List Bool) : Option Bool := do
  let value ← evaluate kind earlier arena span
  IndexedEvaluation.applyMask (some frozen) earlier.length value

/-- An admitted operand row does not make an unavailable mask entry false. -/
theorem frozen_missing_mask (kind : Connective) (earlier : List Bool)
    (arena : List Nat) (span : Span) (frozen : List Bool)
    (admitted : Admitted arena earlier.length span)
    (missing : frozen[earlier.length]? = none) :
    evaluateFrozen kind earlier arena span frozen = none := by
  unfold evaluateFrozen
  rw [evaluate_exact kind earlier arena span admitted]
  simp [IndexedEvaluation.applyMask, missing]

/-- The explicit current-node mask read preserves reduct truth when the stored
bit is that node's original truth. The finite-connective reduct law supplies
child composition. If the original connective is false, its entire reduct is
falsum; if true, the extra mask leaves the computed child reduction unchanged.
The mask premise must be established by a separate original table pass. -/
theorem frozen_exact (kind : Connective) (earlier : List (Formula α))
    (outer tested : α → Bool) (arena : List Nat) (span : Span) (frozen : List Bool)
    (admitted : Admitted arena earlier.length span)
    (mask : frozen[earlier.length]? =
      some (formulaValue outer (meaning kind earlier arena span))) :
    evaluateFrozen kind (earlier.map (fun child => formulaValue tested
        (Reduct (interpretation outer) child))) arena span frozen =
      some (formulaValue tested (Reduct (interpretation outer)
        (meaning kind earlier arena span))) := by
  classical
  unfold evaluateFrozen
  rw [reduct_exact kind earlier outer tested arena span admitted]
  simp only [IndexedEvaluation.applyMask, List.length_map,
    mask, Option.map_some]
  apply congrArg some
  by_cases original : Satisfies (interpretation outer) (meaning kind earlier arena span)
  · have frozenTrue : formulaValue outer (meaning kind earlier arena span) = true :=
      (formula_value_true outer _).mpr original
    simp only [frozenTrue, Bool.and_true]
  · have falseReduct : Reduct (interpretation outer) (meaning kind earlier arena span) = .bot :=
      RuleFactorization.false_reduct _ _ original
    simp only [falseReduct, formulaValue, Bool.false_and]

-- Executable boundaries include zero-width identities, repeated indices,
-- a range beyond the arena, and a self reference at the current node.
example : evaluate .conj [] [] ⟨0, 0⟩ = some true := rfl
example : evaluate .disj [] [] ⟨0, 0⟩ = some false := rfl
example : validate [0, 0] 1 ⟨0, 2⟩ = some [0, 0] := rfl
example : read [0] ⟨1, 1⟩ = none := rfl
example : read [0] ⟨2, 0⟩ = none := rfl
example : evaluate .conj [false] [0, 1] ⟨0, 2⟩ = none := rfl
example : evaluateFrozen .disj [true] [0] ⟨0, 1⟩ [true] = none := rfl

end Zetesis.Refinement.OperandArena
