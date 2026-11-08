import Zetesis.Algebra

/-!
# Streamed original constraints as elementary subjects

One authenticated original integrity constraint supplies a conditional row for
each of its signed occurrences and a full-body exclusion. The row conditions
retain every other occurrence, including repetitions. A fresh unit update is
exactly one application of that row subject, not the completed streamed closure.

The signed representation below concerns original truth only. Positive and
double-negative occurrences share that truth representation; their frozen
Ferraris reducts are not identified. Soundness is for `ModelsOf T` and can be
transported only to an actually included consequence class by the existing
`Subject.toNarrower` law.

A fixed finite family can be packaged denotationally by its existing `joint`.
This grouping does not change the operational eager/late schedule. Authentication
of ground occurrences, successful scalar evaluation, complete cursor coverage,
retained-witness provenance and typed failure/receipt behavior remain concrete
Rust refinement obligations. No theorem here turns a partial or failed scan
into completion or a first-returning callback into a monotone Subject.
-/

namespace Zetesis.StreamedSubjects

open Ferraris FormulaBounds Algebra

universe u
variable {A : Type u}

/-- The learned literal expressing an occurrence's original truth. -/
def truthLiteral (literal : StreamedConsequences.Literal A) : LearnedRegions.Literal A :=
  match literal.sign with
  | .positive | .doubleNegative => (literal.atom, true)
  | .negative => (literal.atom, false)

/-- Signed original truth is exactly the existing learned-literal denotation.
Double negation is simplified only in the original interpretation. -/
theorem holds_truth_iff (M : Atoms A) (literal : StreamedConsequences.Literal A) :
    LearnedRegions.Holds M (truthLiteral literal) ↔ Satisfies M literal.formula := by
  classical
  cases literal with
  | mk sign atom =>
    cases sign <;>
      simp [truthLiteral, LearnedRegions.Holds, StreamedConsequences.Literal.formula,
        Ferraris.Neg, Satisfies]

/-- The opposite learned literal says exactly that the original occurrence fails. -/
theorem holds_opposite_iff (M : Atoms A) (literal : StreamedConsequences.Literal A) :
    LearnedRegions.Holds M (LearnedRegions.negate (truthLiteral literal)) ↔
      ¬ Satisfies M literal.formula := by
  classical
  cases literal with
  | mk sign atom =>
    cases sign <;>
      simp [truthLiteral, LearnedRegions.Holds, LearnedRegions.negate,
        StreamedConsequences.Literal.formula, Ferraris.Neg, Satisfies]

/-- A region decides the truth literal exactly when the occurrence is sure. -/
theorem decides_truth_iff (r : Region A) (literal : StreamedConsequences.Literal A) :
    LearnedRegions.Decides r (truthLiteral literal) ↔ Sure r literal.formula := by
  cases literal with
  | mk sign atom =>
    cases sign <;>
      simp [truthLiteral, LearnedRegions.Decides, StreamedConsequences.Literal.formula,
        Ferraris.Neg, Sure, FormulaBounds.read]

/-- Mapping signed occurrences preserves the condition test, without deduplication. -/
theorem decides_occurrences_iff (r : Region A)
    (literals : List (StreamedConsequences.Literal A)) :
    (∀ l ∈ literals.map truthLiteral, LearnedRegions.Decides r l) ↔
      ∀ literal ∈ literals, Sure r literal.formula := by
  constructor
  · intro decided literal listed
    exact (decides_truth_iff r literal).mp
      (decided _ (List.mem_map.mpr ⟨literal, listed, rfl⟩))
  · intro sure l listed
    obtain ⟨literal, present, same⟩ := List.mem_map.mp listed
    rw [← same]
    exact (decides_truth_iff r literal).mpr (sure literal present)

/-- Every occurrence of a scalar-passing original constraint supplies a conditional
row: if all other occurrences hold, the pivot must fail. Freshness is not a
premise of row validity; it is required only for a new operational decision.

Proof. Were the pivot also true, the actual prefix/pivot/suffix body would be
true, contradicting the interpretation's authenticated original constraint. -/
theorem occurrence_row (T : Theory A)
    (before after : List (StreamedConsequences.Literal A))
    (pivot : StreamedConsequences.Literal A)
    (original : Ferraris.Neg
      (StreamedConsequences.body true (before ++ pivot :: after)) ∈ T) :
    LearnedRegions.RowOf (ModelsOf T) ((before ++ after).map truthLiteral)
      [LearnedRegions.negate (truthLiteral pivot)] := by
  intro M models conditions output listed
  have same : output = LearnedRegions.negate (truthLiteral pivot) :=
    List.mem_singleton.mp listed
  rw [same]
  apply (holds_opposite_iff M pivot).mpr
  intro pivotTrue
  have otherTrue : ∀ literal ∈ before ++ after, Satisfies M literal.formula := by
    intro literal present
    exact (holds_truth_iff M literal).mp
      (conditions _ (List.mem_map.mpr ⟨literal, present, rfl⟩))
  have bodyTrue : Satisfies M
      (StreamedConsequences.body true (before ++ pivot :: after)) := by
    apply (StreamedConsequences.body_satisfied M _).mpr
    intro literal present
    rcases List.mem_append.mp present with earlier | remaining
    · exact otherTrue literal (List.mem_append_left _ earlier)
    · rcases List.mem_cons.mp remaining with samePivot | later
      · simpa only [samePivot] using pivotTrue
      · exact otherTrue literal (List.mem_append_right _ later)
  exact models _ original bodyTrue

/-- The whole scalar-passing body excludes the original model class. -/
theorem body_excludes (T : Theory A)
    (literals : List (StreamedConsequences.Literal A))
    (original : Ferraris.Neg (StreamedConsequences.body true literals) ∈ T) :
    AlgebraicRows.Excludes (ModelsOf T) (literals.map truthLiteral) := by
  intro M models conditions
  have bodyTrue : Satisfies M (StreamedConsequences.body true literals) := by
    apply (StreamedConsequences.body_satisfied M literals).mpr
    intro literal present
    exact (holds_truth_iff M literal).mp
      (conditions _ (List.mem_map.mpr ⟨literal, present, rfl⟩))
  exact models _ original bodyTrue

/-- The elementary Subject for one occurrence of an authenticated original body.
This uses the existing row operation and its extensiveness, monotonicity and
class-soundness laws; it introduces no alternative narrowing mechanism. -/
def occurrenceSubject (T : Theory A)
    (before after : List (StreamedConsequences.Literal A))
    (pivot : StreamedConsequences.Literal A)
    (original : Ferraris.Neg
      (StreamedConsequences.body true (before ++ pivot :: after)) ∈ T) :
    Subject (ModelsOf T) :=
  Algebra.row ((before ++ after).map truthLiteral)
    [LearnedRegions.negate (truthLiteral pivot)]
    (occurrence_row T before after pivot original)

/-- If the other occurrences are sure, this elementary row is exactly the
existing signed falsification operation. No source cursor or completed-family
claim is used in this equality. -/
theorem occurrence_narrow_eq_falsify (T : Theory A) (r : Region A)
    (before after : List (StreamedConsequences.Literal A))
    (pivot : StreamedConsequences.Literal A)
    (original : Ferraris.Neg
      (StreamedConsequences.body true (before ++ pivot :: after)) ∈ T)
    (sure : ∀ literal ∈ before ++ after, Sure r literal.formula) :
    (occurrenceSubject T before after pivot original).narrow r =
      StreamedConsequences.falsify r pivot := by
  have decided : ∀ l ∈ (before ++ after).map truthLiteral, LearnedRegions.Decides r l :=
    (decides_occurrences_iff r _).mpr sure
  have triggered : (occurrenceSubject T before after pivot original).narrow r =
      (⟨fun a => r.lower a ∨
          LearnedRegions.required [LearnedRegions.negate (truthLiteral pivot)] a,
        fun a => r.upper a ∧
          LearnedRegions.permitted [LearnedRegions.negate (truthLiteral pivot)] a⟩ :
        Region A) := by
    apply inside_antisymm
    · constructor
      · intro a held
        rcases held with old | required
        · exact Or.inl old
        · exact Or.inr ⟨decided, required⟩
      · intro a possible
        exact ⟨possible.1, possible.2 decided⟩
    · constructor
      · intro a held
        rcases held with old | required
        · exact Or.inl old
        · exact Or.inr required.2
      · intro a possible
        exact ⟨possible.1, fun _ => possible.2⟩
  rw [triggered]
  cases pivot with
  | mk sign atom =>
    cases sign <;>
      apply inside_antisymm <;>
      constructor <;>
      intro a bound <;>
      simpa [truthLiteral, LearnedRegions.negate, LearnedRegions.required,
        LearnedRegions.permitted, StreamedConsequences.falsify,
        Region.splitTrue, Region.splitFalse] using bound

/-- A fresh unit occurrence is one elementary-family step, not the completed
joint closure. The UnitBody premise retains occurrence multiplicity and its
fresh-pivot check. -/
theorem fresh_step_reaches (T : Theory A) (r : Region A)
    (before after : List (StreamedConsequences.Literal A))
    (pivot : StreamedConsequences.Literal A)
    (original : Ferraris.Neg
      (StreamedConsequences.body true (before ++ pivot :: after)) ∈ T)
    (unit : StreamedConsequences.UnitBody r before pivot after)
    (family : List (Subject (ModelsOf T)))
    (listed : occurrenceSubject T before after pivot original ∈ family) :
    Reaches family r (StreamedConsequences.falsify r pivot) := by
  have step : Reaches family r
      ((occurrenceSubject T before after pivot original).narrow r) :=
    .step _ listed (.stay _)
  rw [occurrence_narrow_eq_falsify T r before after pivot original unit.2] at step
  exact step

/-- A previously enabled occurrence row remains enabled after eager narrowing.
The retained witness's original row identity and current freshness remain separate
implementation obligations at publication. -/
theorem occurrence_conditions_persist {r smaller : Region A}
    (inside : Inside smaller r) (literals : List (StreamedConsequences.Literal A))
    (sure : ∀ literal ∈ literals, Sure r literal.formula) :
    ∀ literal ∈ literals, Sure smaller literal.formula := by
  intro literal present
  exact (decides_truth_iff smaller literal).mp
    (Algebra.decides_mono inside ((decides_truth_iff r literal).mpr (sure literal present)))

/-! ## Completed fixed-family grouping -/

/-- The completed map of a fixed family is a Subject by the existing joint laws.
This is a mathematical package, not a replacement runtime eager/late schedule. -/
def completed {C : Class A} (family : List (Subject C)) : Subject C where
  narrow := joint family
  extensive := joint_extensive family
  monotone := joint_monotone family
  sound := joint_sound family

/-- Completion fixes exactly the regions already closed under the whole family. -/
theorem completed_fixed_iff {C : Class A} (family : List (Subject C)) (r : Region A) :
    (completed family).narrow r = r ↔ Algebra.Closed family r := by
  constructor
  · intro fixed
    have closed : Algebra.Closed family ((completed family).narrow r) := joint_closed family r
    rw [fixed] at closed
    exact closed
  · intro closed
    exact joint_of_closed closed

/-- Grouping one fixed family into its completed Subject preserves exactly the
closed regions. This does not identify intermediate steps, costs or failures. -/
theorem grouped_closed_iff {C : Class A}
    (eager late : List (Subject C)) (r : Region A) :
    Algebra.Closed (eager ++ [completed late]) r ↔ Algebra.Closed (eager ++ late) r := by
  constructor
  · intro grouped s listed
    rcases List.mem_append.mp listed with early | later
    · exact grouped s (List.mem_append_left _ early)
    · have fixed : (completed late).narrow r = r :=
        grouped _ (List.mem_append_right _ (List.mem_singleton.mpr rfl))
      exact (completed_fixed_iff late r).mp fixed s later
  · intro ungrouped s listed
    rcases List.mem_append.mp listed with early | completion
    · exact ungrouped s (List.mem_append_left _ early)
    · have same : s = completed late := List.mem_singleton.mp completion
      rw [same]
      apply (completed_fixed_iff late r).mpr
      exact fun t later => ungrouped t (List.mem_append_right _ later)

/-- Grouping a fixed late family preserves its completed joint Region, by the
joint's least-closed-region property. No fair schedule or cursor completeness is
inferred, and no class is changed. -/
theorem grouped_joint_eq {C : Class A}
    (eager late : List (Subject C)) (r : Region A) :
    joint (eager ++ [completed late]) r = joint (eager ++ late) r := by
  have groupedInside : Inside (joint (eager ++ [completed late]) r)
      (joint (eager ++ late) r) :=
    joint_least ((grouped_closed_iff eager late _).mp
      (joint_closed (eager ++ [completed late]) r))
      (joint_extensive (eager ++ [completed late]) r)
  have ungroupedInside : Inside (joint (eager ++ late) r)
      (joint (eager ++ [completed late]) r) :=
    joint_least ((grouped_closed_iff eager late _).mpr
      (joint_closed (eager ++ late) r))
      (joint_extensive (eager ++ late) r)
  exact inside_antisymm groupedInside ungroupedInside

/-! ## Completed scans of a fixed authenticated finite family -/

/-- If an occurrence is neither sure nor fresh, its falsifying decision is
already present. The stronger geometric statement needs no consistency premise;
the completed operational theorem below is restricted to a consistent Region. -/
theorem falsify_fixed_of_not_sure_not_fresh (r : Region A)
    (literal : StreamedConsequences.Literal A)
    (notSure : ¬ Sure r literal.formula) (notFresh : ¬ r.Fresh literal.atom) :
    StreamedConsequences.falsify r literal = r := by
  classical
  have cutFixed : ∀ atom, ¬ r.upper atom → r.splitFalse atom = r := by
    intro atom absent
    apply inside_antisymm
    · exact ⟨fun _ held => held, fun _ possible => possible.1⟩
    · constructor
      · exact fun _ held => held
      · intro a possible
        exact ⟨possible, fun same => absent (same ▸ possible)⟩
  have heldFixed : ∀ atom, r.lower atom → r.splitTrue atom = r := by
    intro atom held
    apply inside_antisymm
    · exact ⟨fun _ present => Or.inl present, fun _ possible => possible⟩
    · constructor
      · intro a present
        rcases present with old | same
        · exact old
        · rw [same]
          exact held
      · exact fun _ possible => possible
  cases literal with
  | mk sign atom =>
    cases sign with
    | positive =>
      have notHeld : ¬ r.lower atom := by
        simpa [StreamedConsequences.Literal.formula, Sure, FormulaBounds.read] using notSure
      exact cutFixed atom (fun possible => notFresh ⟨notHeld, possible⟩)
    | negative =>
      have possible : r.upper atom := by
        simpa [StreamedConsequences.Literal.formula, Ferraris.Neg, Sure,
          FormulaBounds.read] using notSure
      have held : r.lower atom := by
        apply Classical.byContradiction
        intro absent
        exact notFresh ⟨absent, possible⟩
      exact heldFixed atom held
    | doubleNegative =>
      have notHeld : ¬ r.lower atom := by
        simpa [StreamedConsequences.Literal.formula, Ferraris.Neg, Sure,
          FormulaBounds.read] using notSure
      exact cutFixed atom (fun possible => notFresh ⟨notHeld, possible⟩)

/-- With no consequence in an authenticated body, every occurrence row fixes the
Region. An inactive row does nothing. An enabled row could change the Region
only at a fresh pivot; a sure pivot would instead give an all-true witness.
Both witnesses are excluded by the completed no-consequence premise. -/
theorem no_consequence_occurrence_fixed (T : Theory A) (r : Region A)
    (before after : List (StreamedConsequences.Literal A))
    (pivot : StreamedConsequences.Literal A)
    (original : Ferraris.Neg
      (StreamedConsequences.body true (before ++ pivot :: after)) ∈ T)
    (none : ¬ StreamedConsequences.HasConsequence r true (before ++ pivot :: after)) :
    (occurrenceSubject T before after pivot original).narrow r = r := by
  classical
  by_cases sure : ∀ literal ∈ before ++ after, Sure r literal.formula
  · have notFresh : ¬ r.Fresh pivot.atom := by
      intro fresh
      exact none ⟨rfl, Or.inr ⟨before, pivot, after, rfl, fresh, sure⟩⟩
    have notSure : ¬ Sure r pivot.formula := by
      intro pivotSure
      apply none
      refine ⟨rfl, Or.inl ?_⟩
      intro literal present
      rcases List.mem_append.mp present with earlier | remaining
      · exact sure literal (List.mem_append_left _ earlier)
      · rcases List.mem_cons.mp remaining with same | later
        · simpa only [same] using pivotSure
        · exact sure literal (List.mem_append_right _ later)
    rw [occurrence_narrow_eq_falsify T r before after pivot original sure]
    exact falsify_fixed_of_not_sure_not_fresh r pivot notSure notFresh
  · have inactive : ¬ ∀ l ∈ (before ++ after).map truthLiteral,
        LearnedRegions.Decides r l :=
      fun decided => sure ((decides_occurrences_iff r _).mp decided)
    apply inside_antisymm ((occurrenceSubject T before after pivot original).extensive r)
    constructor
    · intro a held
      rcases held with old | triggered
      · exact old
      · exact (inactive triggered.1).elim
    · intro a possible
      exact ⟨possible, fun triggered => (inactive triggered).elim⟩

/-- A body with no consequence cannot trigger its full-body exclusion. -/
theorem no_consequence_exclusion_fixed (T : Theory A) (r : Region A)
    (literals : List (StreamedConsequences.Literal A))
    (original : Ferraris.Neg (StreamedConsequences.body true literals) ∈ T)
    (none : ¬ StreamedConsequences.HasConsequence r true literals) :
    (Algebra.exclusion (literals.map truthLiteral) (body_excludes T literals original)).narrow r =
      r := by
  have inactive : ¬ ∀ l ∈ literals.map truthLiteral, LearnedRegions.Decides r l := by
    intro decided
    exact none ⟨rfl, Or.inl ((decides_occurrences_iff r literals).mp decided)⟩
  apply inside_antisymm
    ((Algebra.exclusion (literals.map truthLiteral) (body_excludes T literals original)).extensive r)
  constructor
  · intro a held
    rcases held with old | triggered
    · exact old
    · exact (inactive triggered).elim
  · intro a possible
    exact ⟨possible, inactive⟩

/-- All occurrence rows of a body, in source order. Advancing the prefix removes
exactly one occurrence from the remaining list; repeated atoms are not merged.
The recursion is finite in the remaining list, not a source-cursor algorithm. -/
def occurrenceSubjects (T : Theory A) :
    (before remaining : List (StreamedConsequences.Literal A)) →
    (Ferraris.Neg (StreamedConsequences.body true (before ++ remaining)) ∈ T) →
    List (Subject (ModelsOf T))
  | _, [], _ => []
  | before, pivot :: after, original =>
    occurrenceSubject T before after pivot original ::
      occurrenceSubjects T (before ++ [pivot]) after
        (by simpa [List.append_assoc] using original)

/-- A no-consequence body fixes every occurrence row, including repeated and
oppositely signed occurrences of the same atom. -/
theorem no_consequence_occurrences_closed (T : Theory A) (r : Region A)
    (before remaining : List (StreamedConsequences.Literal A))
    (original : Ferraris.Neg (StreamedConsequences.body true (before ++ remaining)) ∈ T)
    (none : ¬ StreamedConsequences.HasConsequence r true (before ++ remaining)) :
    Algebra.Closed (occurrenceSubjects T before remaining original) r := by
  induction remaining generalizing before with
  | nil =>
    intro subject listed
    simp [occurrenceSubjects] at listed
  | cons pivot after ih =>
    intro subject listed
    have member : subject = occurrenceSubject T before after pivot original ∨
        subject ∈ occurrenceSubjects T (before ++ [pivot]) after
          (by simpa [List.append_assoc] using original) := by
      simpa only [occurrenceSubjects, List.mem_cons] using listed
    rcases member with same | later
    · rw [same]
      exact no_consequence_occurrence_fixed T r before after pivot original none
    · have restClosed : Algebra.Closed
          (occurrenceSubjects T (before ++ [pivot]) after
            (by simpa [List.append_assoc] using original)) r := by
        apply ih
        simpa [List.append_assoc] using none
      exact restClosed subject later

/-- The elementary family of one scalar-passing original constraint: its full
body exclusion and every ordered occurrence row. Even the empty body keeps its
exclusion; absence of literals is not absence of an integrity constraint. -/
def constraintSubjects (T : Theory A) (literals : List (StreamedConsequences.Literal A))
    (original : Ferraris.Neg (StreamedConsequences.body true literals) ∈ T) :
    List (Subject (ModelsOf T)) :=
  Algebra.exclusion (literals.map truthLiteral) (body_excludes T literals original) ::
    occurrenceSubjects T [] literals (by simpa using original)

/-- A completed no-consequence check fixes the complete elementary family of its
constraint. The empty-body case cannot satisfy the no-consequence premise. -/
theorem no_consequence_constraint_closed (T : Theory A) (r : Region A)
    (literals : List (StreamedConsequences.Literal A))
    (original : Ferraris.Neg (StreamedConsequences.body true literals) ∈ T)
    (none : ¬ StreamedConsequences.HasConsequence r true literals) :
    Algebra.Closed (constraintSubjects T literals original) r := by
  intro subject listed
  rcases List.mem_cons.mp listed with same | occurrence
  · rw [same]
    exact no_consequence_exclusion_fixed T r literals original none
  · exact no_consequence_occurrences_closed T r [] literals
      (by simpa using original) (by simpa using none) subject occurrence

/-- One abstract source entry with an explicit scalar result. `none` does not
mean false: it records no successful scalar result and cannot satisfy the
completed-scan theorem's success premise. A true result authenticates the actual
original constraint; false contributes no active elementary subject. Constructing
these data from production source/cursor state remains a refinement obligation. -/
structure Instance (T : Theory A) where
  literals : List (StreamedConsequences.Literal A)
  scalar : Option Bool
  original : scalar = some true →
    Ferraris.Neg (StreamedConsequences.body true literals) ∈ T

/-- The active elementary subjects of one entry. Completion never infers
success merely from this list being empty; it separately requires a scalar result. -/
def Instance.subjects {T : Theory A} (entry : Instance T) : List (Subject (ModelsOf T)) :=
  if passed : entry.scalar = some true then
    constraintSubjects T entry.literals (entry.original passed)
  else []

/-- The fixed finite elementary family for the supplied original-source scope. -/
def family (T : Theory A) (scope : List (Instance T)) : List (Subject (ModelsOf T)) :=
  scope.flatMap Instance.subjects

/-- A complete, successfully evaluated no-consequence scan closes the supplied
fixed finite family on a consistent, non-refuted operational path.

Completeness is the explicit inclusion of every scoped entry in `scanned`;
scalar success is required for every scoped entry. Each successful scanned
entry has no sufficient consequence. Thus every true entry fixes its
exclusion and occurrence rows, while successfully false instances contribute
none. The consistency restriction states the intended operational path; the
local fixed-row laws above are stronger geometric facts.

This does not prove that `scope` covers all actual source instances or that the
Rust cursor produced `scanned`. It cannot be applied to a faulting/unvisited
scalar, a partial scan, or merely an empty pending-consequence queue. -/
theorem complete_no_consequence_closed (T : Theory A) (r : Region A)
    (scope scanned : List (Instance T))
    (_consistent : ∀ atom, r.lower atom → r.upper atom)
    (complete : ∀ entry ∈ scope, entry ∈ scanned)
    (successful : ∀ entry ∈ scope, ∃ value, entry.scalar = some value)
    (none : ∀ entry ∈ scanned, ∀ value, entry.scalar = some value →
      ¬ StreamedConsequences.HasConsequence r value entry.literals) :
    Algebra.Closed (family T scope) r := by
  intro subject listed
  obtain ⟨entry, present, member⟩ := List.mem_flatMap.mp listed
  obtain ⟨value, result⟩ := successful entry present
  cases value with
  | false =>
    simp [Instance.subjects, result] at member
  | true =>
    have fixed : Algebra.Closed
        (constraintSubjects T entry.literals (entry.original result)) r :=
      no_consequence_constraint_closed T r entry.literals (entry.original result)
        (none entry (complete entry present) true result)
    have active : entry.subjects =
        constraintSubjects T entry.literals (entry.original result) := by
      simp [Instance.subjects, result]
    rw [active] at member
    exact fixed subject member

end Zetesis.StreamedSubjects
