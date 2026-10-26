import Zetesis.Search
import Zetesis.PossibleFoundedness

/-!
# Learned rows of a consequence class

A learned row pairs a condition B with outputs O. Both are finite lists of signed
atom literals: an atom required present or required absent. A row belongs to a
consequence class C, a predicate on interpretations, when every member of C that
satisfies every condition satisfies every output. The class names what the row is
a consequence of. For rows learned from the original theory alone, C is the
theory's answer sets; a narrower family, such as the answer sets that also satisfy
a streamed core, inherits every row of the wider class.

Rows say nothing about the region in which they were found. A narrowing applies
them to any region: a row whose conditions the region decides refutes the region
when the region decides an output's negation, and otherwise narrows it to the
outputs; a row with exactly one condition left undecided and an output's negation
decided forces that condition to fail. Each conclusion holds for every member of
C in the region, which is the premise `Cube.narrow_preserves_valid` and the
search's `narrowed` constructor require, with C as the valid predicate.

These laws do not certify a concrete row encoding, signed-mask matching, an
occurrence index, retention, eviction, delivery between workers, ownership or
resource handling. Each source of rows discharges `RowOf` for its own rows: the
possible-foundedness collector by `conditional_unsupported_absent`, below.
-/

namespace Zetesis.LearnedRegions

universe u
variable {α : Type u}

/-- A signed atom literal: the atom, and `true` when it is required present. -/
abbrev Literal (α : Type u) := α × Bool

/-- An interpretation satisfies a literal. -/
def Holds (M : Atoms α) : Literal α → Prop
  | (a, true) => M a
  | (a, false) => ¬ M a

/-- The literal on the same atom with the opposite sign. -/
def negate : Literal α → Literal α
  | (a, b) => (a, !b)

/-- A cube decides a literal when every interpretation it contains satisfies the
literal: a required presence lies in its lower bound, a required absence outside
its upper bound. -/
def Decides (c : Cube α) : Literal α → Prop
  | (a, true) => c.lower a
  | (a, false) => ¬ c.upper a

/-- A row of class C: every member of C satisfying every condition satisfies
every output. The lists' order and repetition are immaterial. -/
def RowOf (C : Atoms α → Prop) (B O : List (Literal α)) : Prop :=
  ∀ M, C M → (∀ l ∈ B, Holds M l) → ∀ o ∈ O, Holds M o

/-- The atoms the outputs require present. -/
def required (O : List (Literal α)) : Atoms α := fun a => (a, true) ∈ O

/-- The atoms the outputs leave possible: every atom not required absent. -/
def permitted (O : List (Literal α)) : Atoms α := fun a => (a, false) ∉ O

/-- A literal the cube decides holds in every interpretation it contains. -/
theorem decides_holds {c : Cube α} {z : Atoms α} (inside : c.Contains z) :
    ∀ {l : Literal α}, Decides c l → Holds z l
  | (a, true), lower => inside.1 a lower
  | (a, false), outside => fun present => outside (inside.2 a present)

/-- No interpretation satisfies both a literal and its negation. -/
theorem holds_negate_excludes {M : Atoms α} :
    ∀ {l : Literal α}, Holds M (negate l) → ¬ Holds M l
  | (_, true), absent, present => absent present
  | (_, false), present, absent => absent present

/-- Every interpretation satisfies a literal or its negation. -/
theorem holds_or_negate (M : Atoms α) : ∀ l : Literal α, Holds M l ∨ Holds M (negate l)
  | (a, true) => Classical.em (M a)
  | (a, false) => (Classical.em (M a)).symm

/-- A row whose conditions the cube decides refutes the cube when the cube also
decides an output's negation: the cube contains no member of the class.

Proof. A member of C in the cube satisfies every condition, since the cube decides
each one, so the row gives it every output. It also satisfies the decided negation
of one output, a contradiction. -/
theorem consequence_row_refutes (C : Atoms α → Prop) (B O : List (Literal α))
    (row : RowOf C B O) (c : Cube α)
    (conditions : ∀ l ∈ B, Decides c l)
    (contradicted : ∃ o ∈ O, Decides c (negate o)) :
    ∀ z, c.Contains z → ¬ C z := by
  intro z inside member
  have satisfied : ∀ l ∈ B, Holds z l := by
    intro l listed
    exact decides_holds inside (conditions l listed)
  obtain ⟨o, output, negated⟩ := contradicted
  have holds : Holds z o := row z member satisfied o output
  have opposite : Holds z (negate o) := decides_holds inside negated
  exact holds_negate_excludes opposite holds

/-- A row whose conditions the cube decides gives every member of the class in
the cube every output. -/
theorem consequence_row_forces (C : Atoms α → Prop) (B O : List (Literal α))
    (row : RowOf C B O) (c : Cube α)
    (conditions : ∀ l ∈ B, Decides c l) :
    ∀ z, c.Contains z → C z → ∀ o ∈ O, Holds z o := by
  intro z inside member
  have satisfied : ∀ l ∈ B, Holds z l := by
    intro l listed
    exact decides_holds inside (conditions l listed)
  exact row z member satisfied

/-- The forced outputs as a narrowing: every member of the class in the cube lies
in the cube narrowed to the outputs' required atoms below and their permitted
atoms above. This is `Cube.narrow_preserves_valid`'s premise with the class as
the valid predicate.

Proof. By `consequence_row_forces`, a member satisfies every output: each atom an
output requires present is in it, and each atom an output requires absent is not,
which places it inside both new bounds. -/
theorem consequence_row_narrows (C : Atoms α → Prop) (B O : List (Literal α))
    (row : RowOf C B O) (c : Cube α)
    (conditions : ∀ l ∈ B, Decides c l) :
    ∀ z, c.Contains z → C z → (c.narrow (required O) (permitted O)).Contains z := by
  have forced := consequence_row_forces C B O row c conditions
  have forcedLower : ∀ z, c.Contains z → C z → Sub (required O) z := by
    intro z inside member a requiredPresent
    exact forced z inside member (a, true) requiredPresent
  have forcedUpper : ∀ z, c.Contains z → C z → Sub z (permitted O) := by
    intro z inside member a present requiredAbsent
    exact forced z inside member (a, false) requiredAbsent present
  intro z inside member
  exact Cube.narrow_preserves_valid C c (required O) (permitted O)
    forcedLower forcedUpper inside member

/-- With every condition but one decided and an output's negation decided, the
remaining condition fails in every member of the class in the cube.

Proof. Suppose a member satisfied the remaining condition. It satisfies the others,
which the cube decides, so it satisfies every condition and the row gives it every
output, contradicting the output's decided negation. So it satisfies the remaining
condition's negation. -/
theorem consequence_row_forces_complement (C : Atoms α → Prop) (B O : List (Literal α))
    (row : RowOf C B O) (c : Cube α) (remaining : Literal α)
    (others : ∀ l ∈ B, l ≠ remaining → Decides c l)
    (contradicted : ∃ o ∈ O, Decides c (negate o)) :
    ∀ z, c.Contains z → C z → Holds z (negate remaining) := by
  intro z inside member
  obtain ⟨o, output, negated⟩ := contradicted
  have opposite : Holds z (negate o) := decides_holds inside negated
  cases holds_or_negate z remaining with
  | inr fails => exact fails
  | inl holdsRemaining =>
    have satisfied : ∀ l ∈ B, Holds z l := by
      intro l listed
      by_cases same : l = remaining
      · rw [same]
        exact holdsRemaining
      · exact decides_holds inside (others l listed same)
    have holds : Holds z o := row z member satisfied o output
    exact absurd holds (holds_negate_excludes opposite)

/-- A row of a class is a row of every narrower class. Rows learned from the
original theory alone hold, for example, for the answer sets that also satisfy a
streamed core. -/
theorem row_of_narrower_class (C D : Atoms α → Prop)
    (inclusion : ∀ M, D M → C M) (B O : List (Literal α)) (row : RowOf C B O) :
    RowOf D B O := by
  intro M member satisfied
  exact row M (inclusion M member) satisfied

/-- Two rows of a class with the same conditions give a row with both rows'
outputs. Retention may merge such rows. -/
theorem row_union (C : Atoms α → Prop) (B O₁ O₂ : List (Literal α))
    (left : RowOf C B O₁) (right : RowOf C B O₂) :
    RowOf C B (O₁ ++ O₂) := by
  intro M member satisfied o output
  cases List.mem_append.mp output with
  | inl inLeft => exact left M member satisfied o inLeft
  | inr inRight => exact right M member satisfied o inRight

/-- A row gives every row with more conditions and fewer outputs. Retention may
therefore omit a row another row subsumes, or retire a row a new row dominates,
without losing any consequence. Deleting a row loses pruning only. -/
theorem row_dominance (C : Atoms α → Prop) (B O B' O' : List (Literal α))
    (row : RowOf C B O)
    (conditions : ∀ l ∈ B, l ∈ B') (outputs : ∀ o ∈ O', o ∈ O) :
    RowOf C B' O' := by
  intro M member satisfied o output
  have weaker : ∀ l ∈ B, Holds M l := by
    intro l listed
    exact satisfied l (conditions l listed)
  exact row M member weaker o (outputs o output)

open Ferraris DisjunctiveSupport PossibleFoundedness in
/-- The possible-foundedness collector's rows are rows of the theory's answer
sets: under a condition that blocks every producer entering the unsupported
atoms, each of them is absent from every matching answer set.

This restates `conditional_unsupported_absent` for a condition given as signed
literals and outputs given as absences. It is the founded source's discharge of
`RowOf`; the collector's construction of the blocking witnesses remains an
implementation obligation. -/
theorem founded_row (T : Theory α) (rules : List (Producer α))
    (choices : List (ChoiceProducer α))
    (covered : CoveredWithChoices T rules choices)
    (B : List (Literal α)) (unsupported : List α) (upper : Formula α → Prop)
    (sound : ∀ M, (∀ l ∈ B, Holds M l) →
      UpperSound M (fun a => ¬ a ∈ unsupported) upper)
    (ordinaryBlocked : ∀ r ∈ rules,
      (∃ a, r.head.Contains a ∧ a ∈ unsupported) → ¬ producerEnabled upper r)
    (choicesBlocked : ∀ c ∈ choices, c.head ∈ unsupported → ¬ choiceEnabled upper c) :
    RowOf (fun M => Stable M T) B (unsupported.map fun a => (a, false)) := by
  intro M answer satisfied o output
  obtain ⟨a, listed, named⟩ := List.mem_map.mp output
  have absent : ∀ b, b ∈ unsupported → ¬ M b :=
    conditional_unsupported_absent T rules choices covered
      (fun N => ∀ l ∈ B, Holds N l) (fun b => b ∈ unsupported) upper sound
      ordinaryBlocked choicesBlocked M satisfied answer
  rw [← named]
  exact absent a listed

end Zetesis.LearnedRegions
