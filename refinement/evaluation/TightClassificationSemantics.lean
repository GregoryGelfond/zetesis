import TightDag
import Zetesis.TightBodyRecognition

open Aeneas Aeneas.Std
open ZetesisExtract
open Zetesis

/-!
# Meaning of finite tight-body classification

A finite fold classifies an extracted node table using the original child
indices. On an ordered table its classes agree with exact formula recognition
and the presence of atoms outside default negation. No candidate interpretation
or classical rewriting is used. Out-of-range defaults make the fold total;
the correspondence requires ordered, bounded child references.

These are finite-fold correspondence laws. Agreement of the generated Rust
classifier with this fold is a separate operational proof.
-/
namespace TightClassificationSemantics

/-- An opaque child prevents recognition; otherwise only two frozen children
    make the compound body frozen. -/
def combine : tight.compile.Body → tight.compile.Body → tight.compile.Body
  | .Opaque, _ | _, .Opaque => .Opaque
  | .Frozen, .Frozen => .Frozen
  | _, _ => .Positive

/-- Classify one node using previously computed classes and the immutable
    original table for the exact falsum test at an implication's consequent. -/
def classifyNode (table : List theory.Node) (earlier : List tight.compile.Body) :
    theory.Node → tight.compile.Body
  | .Atom _ => .Positive
  | .False => .Frozen
  | .And left right | .Or left right =>
      combine (earlier.getD left.val .Opaque) (earlier.getD right.val .Opaque)
  | .Implies _ right =>
      match table[right.val]? with
      | some .False => .Frozen
      | _ => .Opaque

/-- Scan a prefix while retaining the full immutable node table for lookups. -/
def scan (table processed : List theory.Node) : List tight.compile.Body :=
  processed.foldl (fun earlier current => earlier ++ [classifyNode table earlier current]) []

/-- Classify every node, in original table order. -/
def classes (table : List theory.Node) : List tight.compile.Body := scan table table

/-- The class determined by exact formula recognition and unfrozen occurrences. -/
def formulaClass (formula : Ferraris.Formula Nat) : tight.compile.Body :=
  match TightBodyRecognition.recognize formula with
  | none => .Opaque
  | some body => if TightBodyRecognition.hasPositive body then .Positive else .Frozen

/-- Every scanned node contributes exactly one class. -/
theorem scan_length (table processed : List theory.Node) :
    (scan table processed).length = processed.length := by
  have lengthFrom (rest : List theory.Node) (earlier : List tight.compile.Body) :
      (rest.foldl (fun entries current =>
        entries ++ [classifyNode table entries current]) earlier).length =
        earlier.length + rest.length := by
    induction rest generalizing earlier with
    | nil => simp
    | cons current rest inductionHypothesis =>
        simp only [List.foldl_cons, inductionHypothesis, List.length_append,
          List.length_cons, List.length_nil]
        omega
  simpa [scan] using lengthFrom processed []

/-- Extending the scanned prefix appends precisely the next local class. -/
theorem scan_snoc (table processed : List theory.Node) (current : theory.Node) :
    scan table (processed ++ [current]) =
      scan table processed ++ [classifyNode table (scan table processed) current] := by
  simp [scan, List.foldl_append]

/-- A stored node unfolds to falsum exactly when its stored constructor is
    falsum. Boundedness excludes the total lookup's default case. -/
theorem meaning_bot_iff (table : List theory.Node)
    (ordered : EvaluationSpecification.Ordered table) (index : Nat)
    (inside : index < table.length) :
    (DagSharing.meanings (table.map EvaluationSemantics.node)).getD index .bot = .bot ↔
      table[index]? = some .False := by
  have decoded := DagSharing.stored_meaning
    (TightDag.ordered_well_formed table ordered) index (by simpa using inside)
  rw [List.getD_eq_getElem (table.map EvaluationSemantics.node) .bot
    (by simpa using inside), List.getElem_map] at decoded
  rw [← decoded, List.getElem?_eq_getElem inside]
  cases table[index] <;> simp [EvaluationSemantics.node, DagSharing.decode]

/-- Exact recognition of conjunction combines the two child classes. -/
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

/-- Exact recognition of disjunction uses the same child-class combination. -/
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

/-- One admitted node has the class of its exact unfolded formula when earlier
    classes already have their formula meanings. The implication case uses
    `meaning_bot_iff`; no classical equivalence is substituted for falsum syntax. -/
theorem node_exact (table : List theory.Node)
    (ordered : EvaluationSpecification.Ordered table) (index : Nat)
    (inside : index < table.length) :
    classifyNode table
        (((DagSharing.meanings (table.map EvaluationSemantics.node)).take index).map
          formulaClass) table[index] =
      formulaClass ((DagSharing.meanings (table.map EvaluationSemantics.node)).getD
        index .bot) := by
  let meanings := DagSharing.meanings (table.map EvaluationSemantics.node)
  have meaningLength : meanings.length = table.length := by
    simp [meanings, DagSharing.meanings_length]
  have lookup (child : Nat) (earlier : child < index) :
      ((meanings.take index).map formulaClass).getD child .Opaque =
        formulaClass (meanings.getD child .bot) := by
    have inMeanings : child < meanings.length := by omega
    have inClasses : child < ((meanings.take index).map formulaClass).length := by
      simp only [List.length_map, List.length_take]
      omega
    rw [List.getD_eq_getElem _ _ inClasses, List.getElem_map, List.getElem_take,
      List.getD_eq_getElem _ _ inMeanings]
  have decoded := DagSharing.stored_meaning
    (TightDag.ordered_well_formed table ordered) index (by simpa using inside)
  rw [List.getD_eq_getElem (table.map EvaluationSemantics.node) .bot
    (by simpa using inside), List.getElem_map] at decoded
  rw [← decoded]
  have before := ordered index inside
  cases entry : table[index] with
  | Atom atom => rfl
  | False => rfl
  | And left right =>
      have children : left.val < index ∧ right.val < index := by
        simpa [entry, EvaluationSpecification.ChildrenBefore] using before
      change combine (((meanings.take index).map formulaClass).getD left.val .Opaque)
        (((meanings.take index).map formulaClass).getD right.val .Opaque) =
          formulaClass (.conj (meanings.getD left.val .bot) (meanings.getD right.val .bot))
      rw [lookup left.val children.1, lookup right.val children.2, formulaClass_conj]
  | Or left right =>
      have children : left.val < index ∧ right.val < index := by
        simpa [entry, EvaluationSpecification.ChildrenBefore] using before
      change combine (((meanings.take index).map formulaClass).getD left.val .Opaque)
        (((meanings.take index).map formulaClass).getD right.val .Opaque) =
          formulaClass (.disj (meanings.getD left.val .bot) (meanings.getD right.val .bot))
      rw [lookup left.val children.1, lookup right.val children.2, formulaClass_disj]
  | Implies left right =>
      have children : left.val < index ∧ right.val < index := by
        simpa [entry, EvaluationSpecification.ChildrenBefore] using before
      have rightInside : right.val < table.length := by omega
      have falsum := meaning_bot_iff table ordered right.val rightInside
      change (match table[right.val]? with
        | some .False => tight.compile.Body.Frozen
        | _ => .Opaque) =
          formulaClass (.imp (meanings.getD left.val .bot) (meanings.getD right.val .bot))
      by_cases rightFalse : table[right.val]? = some .False
      · have formulaFalse : meanings.getD right.val .bot = .bot := falsum.mpr rightFalse
        rw [rightFalse, formulaFalse]
        rfl
      · have formulaNotFalse : meanings.getD right.val .bot ≠ .bot :=
          fun equal => rightFalse (falsum.mp equal)
        cases rightRead : table[right.val]? with
        | none => simp [List.getElem?_eq_getElem rightInside] at rightRead
        | some current =>
            cases current <;> cases consequent : meanings.getD right.val .bot <;>
              simp_all [formulaClass, TightBodyRecognition.recognize]

/-- The scanned prefix agrees, position by position, with the classes of the
    original table's unfolded formulas. Induction appends one admitted node and
    applies `node_exact`; later nodes never alter an earlier formula meaning. -/
theorem scan_take_exact (table : List theory.Node)
    (ordered : EvaluationSpecification.Ordered table) (count : Nat)
    (bounded : count ≤ table.length) :
    scan table (table.take count) =
      ((DagSharing.meanings (table.map EvaluationSemantics.node)).take count).map
        formulaClass := by
  induction count with
  | zero => simp [scan]
  | succ count inductionHypothesis =>
      have inside : count < table.length := by omega
      have earlier := inductionHypothesis (by omega)
      rw [List.take_succ_eq_append_getElem inside, scan_snoc, earlier,
        node_exact table ordered count inside]
      have meaningInside : count <
          (DagSharing.meanings (table.map EvaluationSemantics.node)).length := by
        simpa [DagSharing.meanings_length] using inside
      rw [List.take_succ_eq_append_getElem meaningInside, List.map_append,
        List.map_singleton, List.getD_eq_getElem _ _ meaningInside]

/-- The complete finite classifier fold computes exactly the structural class of
    every unfolded original formula. This discharges the mathematical fold's
    class-recognition premise; generated-call correspondence remains separate. -/
theorem classes_exact (table : List theory.Node)
    (ordered : EvaluationSpecification.Ordered table) :
    classes table =
      (DagSharing.meanings (table.map EvaluationSemantics.node)).map formulaClass := by
  have complete := scan_take_exact table ordered table.length (Nat.le_refl _)
  have meaningLength :
      (DagSharing.meanings (table.map EvaluationSemantics.node)).length = table.length := by
    simp [DagSharing.meanings_length]
  rw [List.take_length, ← meaningLength, List.take_length] at complete
  exact complete

/-- Opaque means that no body in the structural grammar was recognized. -/
theorem formulaClass_opaque (formula : Ferraris.Formula Nat) :
    formulaClass formula = .Opaque ↔ TightBodyRecognition.recognize formula = none := by
  cases recognized : TightBodyRecognition.recognize formula with
  | none => simp [formulaClass, recognized]
  | some body =>
      cases positive : TightBodyRecognition.hasPositive body <;>
        simp [formulaClass, recognized, positive]

/-- Frozen means that the exact recognized body has no unfrozen atom occurrence.
    It does not assert truth or falsity of that body under any interpretation. -/
theorem formulaClass_frozen (formula : Ferraris.Formula Nat) :
    formulaClass formula = .Frozen ↔
      ∃ body : TightPlans.Body Nat, body.formula = formula ∧
        ∀ atom, ¬ body.Positive atom := by
  have characterization : formulaClass formula = .Frozen ↔
      ∃ body, TightBodyRecognition.recognize formula = some body ∧
        TightBodyRecognition.hasPositive body = false := by
    cases recognized : TightBodyRecognition.recognize formula with
    | none => simp [formulaClass, recognized]
    | some body =>
        cases positive : TightBodyRecognition.hasPositive body <;>
          simp [formulaClass, recognized, positive]
  simpa only [TightBodyRecognition.recognize_exact,
    TightBodyRecognition.hasPositive_false_iff] using characterization

/-- Positive means that the exact recognized body contains an atom occurrence
    outside all default-negation scopes. Atom identity needs no equality test. -/
theorem formulaClass_positive (formula : Ferraris.Formula Nat) :
    formulaClass formula = .Positive ↔
      ∃ body : TightPlans.Body Nat, body.formula = formula ∧
        ∃ atom, body.Positive atom := by
  have characterization : formulaClass formula = .Positive ↔
      ∃ body, TightBodyRecognition.recognize formula = some body ∧
        TightBodyRecognition.hasPositive body = true := by
    cases recognized : TightBodyRecognition.recognize formula with
    | none => simp [formulaClass, recognized]
    | some body =>
        cases positive : TightBodyRecognition.hasPositive body <;>
          simp [formulaClass, recognized, positive]
  simpa only [TightBodyRecognition.recognize_exact,
    TightBodyRecognition.hasPositive_iff] using characterization

end TightClassificationSemantics
