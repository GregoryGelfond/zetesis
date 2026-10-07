import Native.Funs
import Zetesis.TightBodyRecognition
import Zetesis.OperandArena

open Aeneas Aeneas.Std
open ZetesisNativeExtract
open Zetesis

/-!
# Finite native tight-body classes

The native compiler's three tags record structural recognition and whether a
recognized body has an occurrence outside negation. The formula recognizer and
positive-occurrence meaning are the existing TightBodyRecognition laws. This
module supplies only their native tag representation and ordered group fold.

The fold does not rewrite formulas, reorder operands or remove duplicates.
Correspondence with an actual operand loop is separate; no whole-table
classifier, rank certificate or answer-set decision is asserted here.
-/
namespace NativeTightClassFold

/-- An opaque child prevents recognition. Otherwise a positive occurrence in
either child makes the compound positive; two frozen children remain frozen. -/
def combine : tight.compile.Body → tight.compile.Body → tight.compile.Body
  | .Opaque, _ | _, .Opaque => .Opaque
  | .Frozen, .Frozen => .Frozen
  | _, _ => .Positive

/-- The native class of the existing exact formula recognizer. Frozen describes
absence of an unfrozen atom occurrence, independently of formula truth. -/
def formulaClass (formula : Ferraris.Formula Nat) : tight.compile.Body :=
  match TightBodyRecognition.recognize formula with
  | none => .Opaque
  | some body => if TightBodyRecognition.hasPositive body then .Positive else .Frozen

/-- The initial Frozen tag is an identity for accumulation. -/
theorem frozen_identity (classification : tight.compile.Body) :
    combine .Frozen classification = classification := by
  cases classification <;> rfl

/-- Native tag combination groups independently of parenthesization. This is a
property of classification, not a formula reassociation or occurrence erasure. -/
theorem combine_assoc (first second third : tight.compile.Body) :
    combine (combine first second) third = combine first (combine second third) := by
  cases first <;> cases second <;> cases third <;> rfl

/-- Classify every listed occurrence against the supplied initialized prefix.
The total fallback is used only to state the fold; actual-loop correspondence
requires each referenced class to be present. -/
def fold (classes : Slice tight.compile.Body) (initial : tight.compile.Body)
    (operands : List Usize) : tight.compile.Body :=
  operands.foldl (fun accumulated child =>
    combine accumulated (classes.val[child.val]?.getD .Opaque)) initial

/-- An empty remaining row returns the accumulator unchanged. -/
theorem fold_nil (classes : Slice tight.compile.Body) (initial : tight.compile.Body) :
    fold classes initial [] = initial := by
  rfl

/-- The next occurrence is combined before the remaining suffix. -/
theorem fold_cons (classes : Slice tight.compile.Body) (initial : tight.compile.Body)
    (child : Usize) (rest : List Usize) :
    fold classes initial (child :: rest) =
      fold classes (combine initial (classes.val[child.val]?.getD .Opaque)) rest := by
  rfl

/-- Consecutive portions of one ordered row compose through the returned tag. -/
theorem fold_append (classes : Slice tight.compile.Body) (initial : tight.compile.Body)
    (first second : List Usize) :
    fold classes initial (first ++ second) = fold classes (fold classes initial first) second := by
  exact List.foldl_append

/-- A conjunction's native class is precisely the combination of its child
classes. Recognition still requires both original children, including opaque
children after a frozen one. -/
theorem formulaClass_conj (left right : Ferraris.Formula Nat) :
    formulaClass (.conj left right) = combine (formulaClass left) (formulaClass right) := by
  cases first : TightBodyRecognition.recognize left with
  | none => simp [formulaClass, TightBodyRecognition.recognize, first, combine]
  | some leftBody =>
      cases second : TightBodyRecognition.recognize right with
      | none =>
          cases positive : TightBodyRecognition.hasPositive leftBody <;>
            simp [formulaClass, TightBodyRecognition.recognize, first, second,
              positive, combine]
      | some rightBody =>
          cases leftPositive : TightBodyRecognition.hasPositive leftBody <;>
            cases rightPositive : TightBodyRecognition.hasPositive rightBody <;>
            simp [formulaClass, TightBodyRecognition.recognize, first, second,
              TightBodyRecognition.hasPositive, leftPositive, rightPositive, combine]

/-- A disjunction uses the same recognition/positive-occurrence combination.
This classification law does not identify disjunction with conjunction. -/
theorem formulaClass_disj (left right : Ferraris.Formula Nat) :
    formulaClass (.disj left right) = combine (formulaClass left) (formulaClass right) := by
  cases first : TightBodyRecognition.recognize left with
  | none => simp [formulaClass, TightBodyRecognition.recognize, first, combine]
  | some leftBody =>
      cases second : TightBodyRecognition.recognize right with
      | none =>
          cases positive : TightBodyRecognition.hasPositive leftBody <;>
            simp [formulaClass, TightBodyRecognition.recognize, first, second,
              positive, combine]
      | some rightBody =>
          cases leftPositive : TightBodyRecognition.hasPositive leftBody <;>
            cases rightPositive : TightBodyRecognition.hasPositive rightBody <;>
            simp [formulaClass, TightBodyRecognition.recognize, first, second,
              TightBodyRecognition.hasPositive, leftPositive, rightPositive, combine]

/-- An opaque tag is exactly failure of the existing structural recognizer. -/
theorem opaque_iff (formula : Ferraris.Formula Nat) :
    formulaClass formula = .Opaque ↔ TightBodyRecognition.recognize formula = none := by
  cases recognized : TightBodyRecognition.recognize formula with
  | none => simp [formulaClass, recognized]
  | some body =>
      cases positive : TightBodyRecognition.hasPositive body <;>
        simp [formulaClass, recognized, positive]

/-- A frozen class supplies an exact recognized body with no atom occurrence
outside negation. It imposes no condition on truth in an interpretation. -/
theorem frozen_iff (formula : Ferraris.Formula Nat) :
    formulaClass formula = .Frozen ↔
      ∃ body : TightPlans.Body Nat, body.formula = formula ∧ ∀ atom, ¬ body.Positive atom := by
  have recognized : formulaClass formula = .Frozen ↔
      ∃ body, TightBodyRecognition.recognize formula = some body ∧
        TightBodyRecognition.hasPositive body = false := by
    cases found : TightBodyRecognition.recognize formula with
    | none => simp [formulaClass, found]
    | some body =>
        cases positive : TightBodyRecognition.hasPositive body <;>
          simp [formulaClass, found, positive]
  simpa only [TightBodyRecognition.recognize_exact,
    TightBodyRecognition.hasPositive_false_iff] using recognized

/-- A positive class supplies an exact recognized body with an unfrozen atom
occurrence. This is structural occurrence, not candidate satisfaction. -/
theorem positive_iff (formula : Ferraris.Formula Nat) :
    formulaClass formula = .Positive ↔
      ∃ body : TightPlans.Body Nat, body.formula = formula ∧ ∃ atom, body.Positive atom := by
  have recognized : formulaClass formula = .Positive ↔
      ∃ body, TightBodyRecognition.recognize formula = some body ∧
        TightBodyRecognition.hasPositive body = true := by
    cases found : TightBodyRecognition.recognize formula with
    | none => simp [formulaClass, found]
    | some body =>
        cases positive : TightBodyRecognition.hasPositive body <;>
          simp [formulaClass, found, positive]
  simpa only [TightBodyRecognition.recognize_exact,
    TightBodyRecognition.hasPositive_iff] using recognized

/-- Tag classification commutes with the existing left-associated conjunction
fold. Only tag representation is new; formula recognition remains shared. -/
theorem formulaClass_conjunction_fold (rest : List (Ferraris.Formula Nat))
    (initial : Ferraris.Formula Nat) :
    formulaClass (rest.foldl Ferraris.Formula.conj initial) =
      (rest.map formulaClass).foldl combine (formulaClass initial) := by
  induction rest generalizing initial with
  | nil => rfl
  | cons next rest induction =>
      simp only [List.foldl_cons, List.map_cons, induction, formulaClass_conj]

/-- The corresponding disjunction fold retains its original syntax while using
the same class accumulator operation. -/
theorem formulaClass_disjunction_fold (rest : List (Ferraris.Formula Nat))
    (initial : Ferraris.Formula Nat) :
    formulaClass (rest.foldl Ferraris.Formula.disj initial) =
      (rest.map formulaClass).foldl combine (formulaClass initial) := by
  induction rest generalizing initial with
  | nil => rfl
  | cons next rest induction =>
      simp only [List.foldl_cons, List.map_cons, induction, formulaClass_disj]

/-- The existing finite connective formula has precisely the ordered class
fold from Frozen. The mathematical empty and singleton cases are included;
this law alone does not authorize such arities in native stored groups. -/
theorem formulaClass_group (kind : Refinement.OperandArena.Connective)
    (operands : List (Ferraris.Formula Nat)) :
    formulaClass (Refinement.OperandArena.formula kind operands) =
      (operands.map formulaClass).foldl combine .Frozen := by
  cases kind <;> cases operands with
  | nil => rfl
  | cons first rest =>
      simp only [Refinement.OperandArena.formula, NormalFerraris.conjunction,
        Refinement.OperandArena.disjunction, formulaClass_conjunction_fold,
        formulaClass_disjunction_fold, List.map_cons, List.foldl_cons, frozen_identity]

/-- If each referenced prefix class already represents its child's exact
formula, the ordered row fold represents the existing finite connective formula.
This is the local premise later supplied by whole-table prefix induction; it is
not an assumption about the generated operand loop's returned result. -/
theorem fold_classified (classes : Slice tight.compile.Body) (operands : List Usize)
    (formulas : Usize → Ferraris.Formula Nat) (kind : Refinement.OperandArena.Connective)
    (represented : ∀ child ∈ operands,
      classes.val[child.val]?.getD .Opaque = formulaClass (formulas child)) :
    fold classes .Frozen operands =
      formulaClass (Refinement.OperandArena.formula kind (operands.map formulas)) := by
  have mapped : operands.map (fun child => classes.val[child.val]?.getD .Opaque) =
      operands.map (fun child => formulaClass (formulas child)) := by
    exact List.map_congr_left represented
  have classesFold : fold classes .Frozen operands =
      (operands.map (fun child => classes.val[child.val]?.getD .Opaque)).foldl combine .Frozen := by
    simp only [fold, List.foldl_map]
  rw [classesFold, mapped, formulaClass_group, List.map_map]
  rfl

end NativeTightClassFold
