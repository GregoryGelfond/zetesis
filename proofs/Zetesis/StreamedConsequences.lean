import Zetesis.StreamedRegions

/-!
# Original streamed constraints narrow candidate regions

A successfully evaluated scalar guard and signed atom occurrences denote one
original constraint body. If every occurrence but one is sure, any model of
that constraint must falsify the remaining occurrence. A default-negated pivot
therefore holds its atom; a positive or double-negated pivot cuts its atom.
The signs retain their different Ferraris reducts. These are original-truth
consequences, not new supporting facts or rewrites of a frozen reduct.

Occurrence identity is retained by the prefix/pivot/suffix decomposition. No
atom-distinctness premise is assumed. A second open occurrence of the pivot's
atom prevents the conservative unique-open test, including opposite signs.
Finite open-atom measures bound successful narrowing steps; a stopped scan
still cannot certify exhaustion. Final original satisfaction and reduct
minimality remain mandatory.

Scalar evaluation success, membership in the original theory, exact grounded
occurrences, immutable region reads, query-mode coverage and checked resource
accounting are premises or Rust correspondence obligations. In particular,
`false` below means a successfully evaluated false scalar guard, never a fault.
These laws neither implement a source cursor nor certify its ownership, shared
whole-closure allowance, cancellation, statistics or worker scheduling.

A finite batch may reuse consequences of one immutable snapshot at any narrower
region. Exact lower/upper agreement on a template's complete read dependency
preserves its sufficient witnesses, so a completed negative scan can be reused.
Neither law grants permission to omit fallible scalar evaluation or to reorder
faults. Dependency coverage and completed scans are explicit premises.
-/

namespace Zetesis.StreamedConsequences

open Ferraris FormulaBounds

universe u v
variable {A : Type u}

/-- Original body signs; positive and double-negative truth agree, not reducts. -/
inductive Sign where
  | positive
  | negative
  | doubleNegative

/-- A grounded signed literal; its body-list position distinguishes occurrences. -/
structure Literal (A : Type u) where
  sign : Sign
  atom : A

/-- The signed occurrence's original Ferraris formula. -/
def Literal.formula (literal : Literal A) : Formula A :=
  match literal.sign with
  | .positive => .atom literal.atom
  | .negative => Ferraris.Neg (.atom literal.atom)
  | .doubleNegative => Ferraris.Neg (Ferraris.Neg (.atom literal.atom))

/-- Scalar guards are already evaluated successfully before this denotation. -/
def body (scalarPassed : Bool) (literals : List (Literal A)) : Formula A :=
  if scalarPassed then NormalFerraris.conjunction (literals.map Literal.formula) else .bot

/-- A body whose scalar guard passed is true exactly when all occurrences are. -/
theorem body_satisfied (M : Atoms A) (literals : List (Literal A)) :
    Satisfies M (body true literals) ↔
      ∀ literal ∈ literals, Satisfies M literal.formula := by
  simp [body, NormalFerraris.satisfies_conjunction]

/-- Every model of a subregion also belongs to its enclosing region. -/
theorem contains_of_inside {smaller larger : Cube A}
    (inside : Inside smaller larger) {M : Atoms A} (member : smaller.Contains M) :
    larger.Contains M :=
  ⟨fun a held => member.1 a (inside.1 a held),
    fun a present => inside.2 a (member.2 a present)⟩

/-- A fresh atom cannot make any of its three signed occurrences sure. -/
theorem fresh_not_sure (c : Cube A) (literal : Literal A)
    (fresh : c.Fresh literal.atom) : ¬ Sure c literal.formula := by
  cases literal with
  | mk sign atom =>
    cases sign <;>
      simp_all [Literal.formula, Sure, FormulaBounds.read, Ferraris.Neg, Cube.Fresh]

/-- Exactly one occurrence may remain open; every other occurrence must be sure.
This is deliberately conservative when several occurrences share an open atom. -/
def UnitBody (c : Cube A) (before : List (Literal A)) (pivot : Literal A)
    (after : List (Literal A)) : Prop :=
  c.Fresh pivot.atom ∧ ∀ literal ∈ before ++ after, Sure c literal.formula

/-- A second occurrence of the same open atom blocks this occurrence-based test,
regardless of its sign. No alias coalescing or pivot-query completeness is assumed. -/
theorem aliased_open_occurrence_blocks (c : Cube A) (before after : List (Literal A))
    (pivot other : Literal A) (present : other ∈ before ++ after)
    (sameAtom : other.atom = pivot.atom) : ¬ UnitBody c before pivot after := by
  intro unit
  have fresh : c.Fresh other.atom := sameAtom ▸ unit.1
  exact fresh_not_sure c other fresh (unit.2 other present)

/-- One authenticated all-true body refutes every classical model of its region.
Only this occurrence is needed; the remaining source need not be exhausted. -/
theorem all_true_refutes (T : Theory A) (c : Cube A) (literals : List (Literal A))
    (scalarPassed : Bool) (passed : scalarPassed = true)
    (original : Ferraris.Neg (body scalarPassed literals) ∈ T)
    (sure : ∀ literal ∈ literals, Sure c literal.formula) :
    ∀ M, c.Contains M → ¬ Models M T := by
  intro M member models
  have satisfied : Satisfies M (body scalarPassed literals) := by
    rw [passed, body_satisfied]
    exact fun literal present => sure_sound c member (sure literal present)
  exact models _ original satisfied

/-- A scalar-passing original constraint forces its sole open occurrence false.

Proof outline: each other occurrence is true in any interpretation in the cube.
If the pivot were also true, the entire original body would hold, contradicting
that interpretation's satisfaction of its authenticated integrity constraint. -/
theorem unit_literal_false (T : Theory A) (c : Cube A)
    (before after : List (Literal A)) (pivot : Literal A)
    (scalarPassed : Bool) (passed : scalarPassed = true)
    (original : Ferraris.Neg (body scalarPassed (before ++ pivot :: after)) ∈ T)
    (unit : UnitBody c before pivot after)
    {M : Atoms A} (member : c.Contains M) (models : Models M T) :
    ¬ Satisfies M pivot.formula := by
  intro pivotTrue
  have satisfied : Satisfies M (body scalarPassed (before ++ pivot :: after)) := by
    rw [passed, body_satisfied]
    intro literal present
    rcases List.mem_append.mp present with earlier | remaining
    · exact sure_sound c member (unit.2 literal (List.mem_append_left _ earlier))
    · rcases List.mem_cons.mp remaining with same | later
      · simpa only [same] using pivotTrue
      · exact sure_sound c member (unit.2 literal (List.mem_append_right _ later))
  exact models _ original satisfied

/-- Falsifying the open default-negated occurrence holds its atom. -/
theorem unit_negative_holds (T : Theory A) (c : Cube A)
    (before after : List (Literal A)) (atom : A)
    (original : Ferraris.Neg (body true (before ++ ⟨.negative, atom⟩ :: after)) ∈ T)
    (unit : UnitBody c before ⟨.negative, atom⟩ after)
    {M : Atoms A} (member : c.Contains M) (models : Models M T) : M atom := by
  have forced := unit_literal_false T c before after ⟨.negative, atom⟩ true rfl
    original unit member models
  simpa [Literal.formula, Ferraris.Neg, Satisfies] using forced

/-- Falsifying the open positive occurrence cuts its atom. -/
theorem unit_positive_cuts (T : Theory A) (c : Cube A)
    (before after : List (Literal A)) (atom : A)
    (original : Ferraris.Neg (body true (before ++ ⟨.positive, atom⟩ :: after)) ∈ T)
    (unit : UnitBody c before ⟨.positive, atom⟩ after)
    {M : Atoms A} (member : c.Contains M) (models : Models M T) : ¬ M atom :=
  unit_literal_false T c before after ⟨.positive, atom⟩ true rfl original unit member models

/-- Double negation also cuts its atom, by original truth rather than reduct rewriting. -/
theorem unit_double_negative_cuts (T : Theory A) (c : Cube A)
    (before after : List (Literal A)) (atom : A)
    (original : Ferraris.Neg (body true (before ++ ⟨.doubleNegative, atom⟩ :: after)) ∈ T)
    (unit : UnitBody c before ⟨.doubleNegative, atom⟩ after)
    {M : Atoms A} (member : c.Contains M) (models : Models M T) : ¬ M atom := by
  have forced := unit_literal_false T c before after ⟨.doubleNegative, atom⟩ true rfl
    original unit member models
  simpa [Literal.formula, Ferraris.Neg, Satisfies] using forced

/-- Restrict candidate interpretations to those falsifying this occurrence. -/
def falsify (c : Cube A) (literal : Literal A) : Cube A :=
  match literal.sign with
  | .negative => c.splitTrue literal.atom
  | .positive | .doubleNegative => c.splitFalse literal.atom

/-- The signed decision only strengthens bounds; it cannot reopen an atom. -/
theorem falsify_inside (c : Cube A) (literal : Literal A) : Inside (falsify c literal) c := by
  cases literal with
  | mk sign atom =>
    cases sign with
    | positive => exact ⟨fun _ held => held, fun _ possible => possible.1⟩
    | negative => exact ⟨fun _ held => Or.inl held, fun _ possible => possible⟩
    | doubleNegative => exact ⟨fun _ held => held, fun _ possible => possible.1⟩

/-- A signed decision retains precisely interpretations falsifying its occurrence. -/
theorem contains_falsify (c : Cube A) (literal : Literal A) (M : Atoms A) :
    (falsify c literal).Contains M ↔ c.Contains M ∧ ¬ Satisfies M literal.formula := by
  cases literal with
  | mk sign atom =>
    cases sign <;> simp [falsify, Cube.contains_splitFalse, Cube.contains_splitTrue,
      Literal.formula, Ferraris.Neg, Satisfies]

/-- Unit narrowing preserves exactly the original classical models in the cube.
The model predicate, including every original constraint, is unchanged. -/
theorem unit_preserves_models (T : Theory A) (c : Cube A)
    (before after : List (Literal A)) (pivot : Literal A)
    (scalarPassed : Bool) (passed : scalarPassed = true)
    (original : Ferraris.Neg (body scalarPassed (before ++ pivot :: after)) ∈ T)
    (unit : UnitBody c before pivot after) (M : Atoms A) :
    (falsify c pivot).Contains M ∧ Models M T ↔ c.Contains M ∧ Models M T := by
  constructor
  · intro accepted
    exact ⟨(contains_falsify c pivot M).mp accepted.1 |>.1, accepted.2⟩
  · intro accepted
    have forced := unit_literal_false T c before after pivot scalarPassed passed
      original unit accepted.1 accepted.2
    exact ⟨(contains_falsify c pivot M).mpr ⟨accepted.1, forced⟩, accepted.2⟩

/-- The same restriction preserves exactly all answer sets. It does not establish
that any retained interpretation is stable; that still requires its frozen reduct. -/
theorem unit_preserves_answers (T : Theory A) (c : Cube A)
    (before after : List (Literal A)) (pivot : Literal A)
    (scalarPassed : Bool) (passed : scalarPassed = true)
    (original : Ferraris.Neg (body scalarPassed (before ++ pivot :: after)) ∈ T)
    (unit : UnitBody c before pivot after) (M : Atoms A) :
    (falsify c pivot).Contains M ∧ Stable M T ↔ c.Contains M ∧ Stable M T := by
  constructor
  · intro accepted
    exact ⟨(contains_falsify c pivot M).mp accepted.1 |>.1, accepted.2⟩
  · intro accepted
    have forced := unit_literal_false T c before after pivot scalarPassed passed
      original unit accepted.1 accepted.2.1
    exact ⟨(contains_falsify c pivot M).mpr ⟨accepted.1, forced⟩, accepted.2⟩

/-- A sound unit consequence remains true after any further bound strengthening. -/
theorem unit_survives_narrowing (T : Theory A) (c smaller : Cube A)
    (inside : Inside smaller c) (before after : List (Literal A)) (pivot : Literal A)
    (original : Ferraris.Neg (body true (before ++ pivot :: after)) ∈ T)
    (unit : UnitBody c before pivot after)
    {M : Atoms A} (member : smaller.Contains M) (models : Models M T) :
    ¬ Satisfies M pivot.formula :=
  unit_literal_false T c before after pivot true rfl original unit
    (contains_of_inside inside member) models

/-- Falsifying this occurrence is necessary for every valid interpretation in
one immutable snapshot. Validity may be original modelhood or answer-set membership;
neither current openness nor a successful source lookup is implicit. -/
def SnapshotConsequence (valid : Atoms A → Prop) (snapshot : Cube A)
    (literal : Literal A) : Prop :=
  ∀ M, snapshot.Contains M → valid M → ¬ Satisfies M literal.formula

/-- The existing authenticated unit rule supplies a snapshot consequence. -/
theorem unit_snapshot_consequence (T : Theory A) (snapshot : Cube A)
    (before after : List (Literal A)) (pivot : Literal A)
    (original : Ferraris.Neg (body true (before ++ pivot :: after)) ∈ T)
    (unit : UnitBody snapshot before pivot after) :
    SnapshotConsequence (fun M => Models M T) snapshot pivot := by
  intro M member models
  exact unit_literal_false T snapshot before after pivot true rfl original unit member models

/-- A proved snapshot consequence remains valid after any bound strengthening.
The old pivot need not still be open; this is semantic validity, not progress. -/
theorem snapshot_consequence_survives (valid : Atoms A → Prop)
    (snapshot smaller : Cube A) (inside : Inside smaller snapshot) (literal : Literal A)
    (sound : SnapshotConsequence valid snapshot literal) :
    SnapshotConsequence valid smaller literal := by
  intro M member accepted
  exact sound M (contains_of_inside inside member) accepted

/-- Apply a finite batch in list order. Repetitions and conflicting signed
decisions are allowed; consistency and successful atom updates are not assumed. -/
def falsifyBatch (c : Cube A) : List (Literal A) → Cube A
  | [] => c
  | literal :: rest => falsifyBatch (falsify c literal) rest

/-- A batch retains exactly the old interpretations falsifying each listed
occurrence. Induction separates the first decision from the remaining batch. -/
theorem contains_falsify_batch (c : Cube A) (pending : List (Literal A)) (M : Atoms A) :
    (falsifyBatch c pending).Contains M ↔
      c.Contains M ∧ ∀ literal ∈ pending, ¬ Satisfies M literal.formula := by
  induction pending generalizing c with
  | nil => simp [falsifyBatch]
  | cons first rest ih =>
    rw [falsifyBatch, ih, contains_falsify]
    constructor
    · rintro ⟨⟨member, firstFalse⟩, restFalse⟩
      refine ⟨member, ?_⟩
      intro literal present
      rcases List.mem_cons.mp present with same | later
      · simpa only [same] using firstFalse
      · exact restFalse literal later
    · rintro ⟨member, allFalse⟩
      exact ⟨⟨member, allFalse first (List.mem_cons_self)⟩,
        fun literal present => allFalse literal (List.mem_cons_of_mem first present)⟩

/-- Applying snapshot-valid decisions to a narrower region preserves exactly
its valid interpretations. Every finite prefix is covered by choosing that
prefix as `pending`; no completed scan or exhausted search follows.

Proof outline: batch membership is old membership plus the listed falsities.
Every valid old member also belongs to the snapshot, where each falsity was
proved. Conversely, restriction never adds an interpretation. -/
theorem batch_preserves_valid (valid : Atoms A → Prop) (snapshot current : Cube A)
    (inside : Inside current snapshot) (pending : List (Literal A))
    (sound : ∀ literal ∈ pending, SnapshotConsequence valid snapshot literal)
    (M : Atoms A) :
    (falsifyBatch current pending).Contains M ∧ valid M ↔
      current.Contains M ∧ valid M := by
  rw [contains_falsify_batch]
  constructor
  · intro retained
    exact ⟨retained.1.1, retained.2⟩
  · intro accepted
    have snapshotMember : snapshot.Contains M := contains_of_inside inside accepted.1
    have allFalse : ∀ literal ∈ pending, ¬ Satisfies M literal.formula := by
      intro literal present
      exact sound literal present M snapshotMember accepted.2
    exact ⟨⟨accepted.1, allFalse⟩, accepted.2⟩

/-- Applying only a prefix of a valid batch preserves the same interpretations.
The unconsumed suffix is not claimed checked, applied or exhausted. -/
theorem batch_prefix_preserves_valid (valid : Atoms A → Prop) (snapshot current : Cube A)
    (inside : Inside current snapshot) (applied pending : List (Literal A))
    (sound : ∀ literal ∈ applied ++ pending, SnapshotConsequence valid snapshot literal)
    (M : Atoms A) :
    (falsifyBatch current applied).Contains M ∧ valid M ↔
      current.Contains M ∧ valid M := by
  have prefixSound : ∀ literal ∈ applied, SnapshotConsequence valid snapshot literal := by
    intro literal present
    exact sound literal (List.mem_append_left pending present)
  exact batch_preserves_valid valid snapshot current inside applied prefixSound M

/-- Batching original-model consequences preserves answer sets too, without
replacing original satisfaction or frozen-reduct minimality by those decisions. -/
theorem batch_preserves_answers (T : Theory A) (snapshot current : Cube A)
    (inside : Inside current snapshot) (pending : List (Literal A))
    (sound : ∀ literal ∈ pending, SnapshotConsequence (fun M => Models M T) snapshot literal)
    (M : Atoms A) :
    (falsifyBatch current pending).Contains M ∧ Stable M T ↔
      current.Contains M ∧ Stable M T := by
  have stableSound : ∀ literal ∈ pending,
      SnapshotConsequence (fun M => Stable M T) snapshot literal := by
    intro literal present interpretation member stable
    exact sound literal present interpretation member stable.1
  exact batch_preserves_valid (fun M => Stable M T) snapshot current inside pending stableSound M

/-- Exact agreement on both bounds of every atom in an explicit dependency.
Equal decided unions are insufficient: they can hide a held/cut polarity swap. -/
def SameBoundsOn (left right : Cube A) (dependency : Atoms A) : Prop :=
  ∀ atom, dependency atom →
    (left.lower atom ↔ right.lower atom) ∧ (left.upper atom ↔ right.upper atom)

/-- A signed literal's sure reading is unchanged when its atom's bounds agree. -/
theorem literal_sure_unchanged (left right : Cube A) (dependency : Atoms A)
    (same : SameBoundsOn left right dependency) (literal : Literal A)
    (covered : dependency literal.atom) :
    Sure left literal.formula ↔ Sure right literal.formula := by
  have bounds := same literal.atom covered
  cases literal with
  | mk sign atom =>
    cases sign <;> simp_all [Literal.formula, Sure, FormulaBounds.read, Ferraris.Neg]

/-- The occurrence-based unit test is unchanged when every body atom's bounds
agree. Aliases retain their occurrence positions and the conservative old test. -/
theorem unit_body_unchanged (left right : Cube A) (dependency : Atoms A)
    (same : SameBoundsOn left right dependency)
    (before after : List (Literal A)) (pivot : Literal A)
    (covered : ∀ literal ∈ before ++ pivot :: after, dependency literal.atom) :
    UnitBody left before pivot after ↔ UnitBody right before pivot after := by
  have pivotCovered : dependency pivot.atom := covered pivot (by simp)
  have pivotBounds := same pivot.atom pivotCovered
  have fresh : left.Fresh pivot.atom ↔ right.Fresh pivot.atom := by
    exact and_congr (not_congr pivotBounds.1) pivotBounds.2
  have sure : ∀ literal ∈ before ++ after,
      Sure left literal.formula ↔ Sure right literal.formula := by
    intro literal present
    have inBody : literal ∈ before ++ pivot :: after := by
      rcases List.mem_append.mp present with earlier | later
      · exact List.mem_append_left _ earlier
      · exact List.mem_append_right _ (List.mem_cons_of_mem pivot later)
    exact literal_sure_unchanged left right dependency same literal (covered literal inBody)
  constructor
  · intro unit
    exact ⟨fresh.mp unit.1, fun literal present => (sure literal present).mp (unit.2 literal present)⟩
  · intro unit
    exact ⟨fresh.mpr unit.1, fun literal present => (sure literal present).mpr (unit.2 literal present)⟩

/-- The sufficient witnesses sought for one successfully evaluated instance:
either its whole body is sure, or its occurrence-based unit test succeeds. -/
def HasConsequence (c : Cube A) (scalarPassed : Bool) (literals : List (Literal A)) : Prop :=
  scalarPassed = true ∧ ((∀ literal ∈ literals, Sure c literal.formula) ∨
    ∃ before pivot after, literals = before ++ pivot :: after ∧ UnitBody c before pivot after)

/-- A completed instance has the same sufficient witnesses if its complete atom
dependency is unchanged. The scalar result and original occurrences are identical
on both sides; faulting or newly enabled scalar evaluation is outside this law. -/
theorem consequence_unchanged (left right : Cube A) (dependency : Atoms A)
    (same : SameBoundsOn left right dependency) (scalarPassed : Bool)
    (literals : List (Literal A))
    (covered : ∀ literal ∈ literals, dependency literal.atom) :
    HasConsequence left scalarPassed literals ↔ HasConsequence right scalarPassed literals := by
  have allSure : (∀ literal ∈ literals, Sure left literal.formula) ↔
      ∀ literal ∈ literals, Sure right literal.formula := by
    constructor
    · intro sure literal present
      exact (literal_sure_unchanged left right dependency same literal (covered literal present)).mp
        (sure literal present)
    · intro sure literal present
      exact (literal_sure_unchanged left right dependency same literal (covered literal present)).mpr
        (sure literal present)
  have units : (∃ before pivot after, literals = before ++ pivot :: after ∧
      UnitBody left before pivot after) ↔
      ∃ before pivot after, literals = before ++ pivot :: after ∧
        UnitBody right before pivot after := by
    constructor
    · rintro ⟨before, pivot, after, shape, unit⟩
      have bodyCovered : ∀ literal ∈ before ++ pivot :: after, dependency literal.atom := by
        simpa only [shape] using covered
      exact ⟨before, pivot, after, shape,
        (unit_body_unchanged left right dependency same before after pivot bodyCovered).mp unit⟩
    · rintro ⟨before, pivot, after, shape, unit⟩
      have bodyCovered : ∀ literal ∈ before ++ pivot :: after, dependency literal.atom := by
        simpa only [shape] using covered
      exact ⟨before, pivot, after, shape,
        (unit_body_unchanged left right dependency same before after pivot bodyCovered).mpr unit⟩
  exact and_congr Iff.rfl (or_congr allSure units)

/-- A completely scanned template needs no new witness search if every original
instance reads unchanged bounds. `instances` must cover the whole template, with
the same successfully evaluated scalar results and grounded occurrences. A stopped
scan cannot supply `complete`; no runtime cursor or dirty-set coverage is proved. -/
theorem completed_template_unchanged {I : Type v} (instances : List I)
    (scalarPassed : I → Bool) (literals : I → List (Literal A))
    (left right : Cube A) (dependency : Atoms A)
    (same : SameBoundsOn left right dependency)
    (covered : ∀ entry ∈ instances, ∀ literal ∈ literals entry, dependency literal.atom)
    (complete : ∀ entry ∈ instances,
      ¬ HasConsequence left (scalarPassed entry) (literals entry)) :
    ∀ entry ∈ instances, ¬ HasConsequence right (scalarPassed entry) (literals entry) := by
  intro entry present witness
  have oldWitness : HasConsequence left (scalarPassed entry) (literals entry) :=
    (consequence_unchanged left right dependency same (scalarPassed entry)
      (literals entry) (covered entry present)).mpr witness
  exact complete entry present oldWitness

/-- A newly available consequence must read an atom whose bounds changed.
The scalar result and grounded occurrences are fixed, and the earlier instance
has been completely checked without a consequence.

Proof outline: if every occurrence read unchanged bounds, the existing
consequence would also have existed before, contradicting the completed check.
The law does not establish which source rows enumerate those occurrences. -/
theorem consequence_reads_change (left right : Cube A) (changed : Atoms A)
    (same : SameBoundsOn left right (fun atom => ¬ changed atom))
    (scalarPassed : Bool) (literals : List (Literal A))
    (absent : ¬ HasConsequence left scalarPassed literals)
    (present : HasConsequence right scalarPassed literals) :
    ∃ literal ∈ literals, changed literal.atom := by
  classical
  apply Classical.byContradiction
  intro missing
  have covered : ∀ literal ∈ literals, ¬ changed literal.atom := by
    intro literal member altered
    exact missing ⟨literal, member, altered⟩
  have oldWitness : HasConsequence left scalarPassed literals :=
    (consequence_unchanged left right (fun atom => ¬ changed atom) same
      scalarPassed literals covered).mpr present
  exact absent oldWitness

/-- After a completed negative template scan, a change read only by positive
occurrences covers every newly available consequence through such an occurrence.
Repeated occurrences and aliases remain in the list; any one may be the witness.

The changed set can contain one atom or several. A concrete cursor must still
cover every matching positive occurrence and retain the same scalar evaluation,
bindings and occurrence-based unit test. Negative or double-negative readers
do not satisfy the positive-read premise. -/
theorem positive_changes_cover_consequences {I : Type v} (instances : List I)
    (scalarPassed : I → Bool) (literals : I → List (Literal A))
    (left right : Cube A) (changed : Atoms A)
    (same : SameBoundsOn left right (fun atom => ¬ changed atom))
    (positive : ∀ entry ∈ instances, ∀ literal ∈ literals entry,
      changed literal.atom → literal.sign = .positive)
    (complete : ∀ entry ∈ instances,
      ¬ HasConsequence left (scalarPassed entry) (literals entry)) :
    ∀ entry ∈ instances, HasConsequence right (scalarPassed entry) (literals entry) →
      ∃ literal ∈ literals entry, literal.sign = .positive ∧ changed literal.atom := by
  intro entry member consequence
  obtain ⟨literal, occurrence, altered⟩ :=
    consequence_reads_change left right changed same (scalarPassed entry)
      (literals entry) (complete entry member) consequence
  exact ⟨literal, occurrence, positive entry member literal occurrence altered, altered⟩

/-- Strengthening both bounds cannot turn a decided atom back into an open one. -/
theorem fresh_of_inside {smaller larger : Cube A} (inside : Inside smaller larger)
    {atom : A} (fresh : smaller.Fresh atom) : larger.Fresh atom :=
  ⟨fun held => fresh.1 (inside.1 atom held), inside.2 atom fresh.2⟩

/-- A signed decision leaves its own atom decided. -/
theorem falsify_not_fresh (c : Cube A) (literal : Literal A) :
    ¬ (falsify c literal).Fresh literal.atom := by
  cases literal with
  | mk sign atom =>
    cases sign <;> simp [falsify, Cube.Fresh, Cube.splitFalse, Cube.splitTrue]

open Classical in
/-- The number of open atoms in a finite supplied carrier. A distinct carrier
counts atoms once; duplicates can only make this conservative bound larger. -/
noncomputable def openCount (carrier : List A) (c : Cube A) : Nat :=
  carrier.countP (fun atom => decide (c.Fresh atom))

/-- Any sound or unsound strengthening decreases this geometric measure weakly;
semantic preservation is a separate obligation, never inferred from the measure. -/
theorem open_count_mono (carrier : List A) {smaller larger : Cube A}
    (inside : Inside smaller larger) : openCount carrier smaller ≤ openCount carrier larger := by
  classical
  apply List.countP_mono_left
  intro atom _ fresh
  exact decide_eq_true (fresh_of_inside inside (of_decide_eq_true fresh))

/-- Deciding a fresh carrier atom strictly decreases the finite open-atom count.
The finite carrier need only contain this pivot; every later pivot has the same
explicit membership obligation. The result bounds decisions, not source work. -/
theorem falsify_decreases_open (carrier : List A) (c : Cube A) (pivot : Literal A)
    (member : pivot.atom ∈ carrier) (fresh : c.Fresh pivot.atom) :
    openCount carrier (falsify c pivot) < openCount carrier c := by
  classical
  have decided : ¬ (falsify c pivot).Fresh pivot.atom := falsify_not_fresh c pivot
  induction carrier with
  | nil => simp at member
  | cons atom rest ih =>
    have headMono : (falsify c pivot).Fresh atom → c.Fresh atom :=
      fresh_of_inside (falsify_inside c pivot)
    rcases List.mem_cons.mp member with same | remaining
    · subst atom
      have tailBound := open_count_mono rest (falsify_inside c pivot)
      simp only [openCount, List.countP_cons, decide_eq_true_eq] at *
      simp only [if_neg decided, if_pos fresh]
      omega
    · have tailStrict := ih remaining
      simp only [openCount, List.countP_cons, decide_eq_true_eq] at *
      by_cases newOpen : (falsify c pivot).Fresh atom
      · simp only [if_pos newOpen, if_pos (headMono newOpen)]
        omega
      · by_cases oldOpen : c.Fresh atom
        · simp only [if_neg newOpen, if_pos oldOpen]
          omega
        · simp only [if_neg newOpen, if_neg oldOpen]
          exact tailStrict

/-- A finite sequence of sound source decisions, with any preserving monotone
formula closure before each decision. The selected atom must still be open.
The constructor requires the actual semantic consequence, not an attempted or
interrupted lookup. It is not a concrete runtime or a completion certificate. -/
inductive ConsequenceSteps (carrier : List A) (valid : Atoms A → Prop) :
    Cube A → Cube A → Nat → Prop where
  | done (c : Cube A) : ConsequenceSteps carrier valid c c 0
  | next {c closed final : Cube A} {count : Nat}
      (inside : Inside closed c)
      (closurePreserves : ∀ M, c.Contains M → valid M → closed.Contains M)
      (pivot : Literal A) (member : pivot.atom ∈ carrier) (fresh : closed.Fresh pivot.atom)
      (forced : ∀ M, closed.Contains M → valid M → ¬ Satisfies M pivot.formula)
      (rest : ConsequenceSteps carrier valid (falsify closed pivot) final count) :
      ConsequenceSteps carrier valid c final (count + 1)

/-- Completed source decisions and preserving formula closures retain exactly the
valid interpretations of the initial region. An interrupted prefix can use this
preservation law but cannot claim that its final region has been searched. -/
theorem steps_preserve (carrier : List A) (valid : Atoms A → Prop)
    {first last : Cube A} {count : Nat}
    (steps : ConsequenceSteps carrier valid first last count) (M : Atoms A) :
    last.Contains M ∧ valid M ↔ first.Contains M ∧ valid M := by
  induction steps with
  | done c => rfl
  | @next c closed final count inside closurePreserves pivot member fresh forced rest ih =>
    rw [ih, contains_falsify]
    constructor
    · intro retained
      exact ⟨contains_of_inside inside retained.1.1, retained.2⟩
    · intro initial
      have closedMember : closed.Contains M := closurePreserves M initial.1 initial.2
      exact ⟨⟨closedMember, forced M closedMember initial.2⟩, initial.2⟩

/-- Every successful step spends a distinct remaining open-atom opportunity.
Thus a run has at most the initial open count many completed decisions. This
measure does not count source calls, resource work or interruptions, and grants
no renewed allowance. -/
theorem steps_bounded (carrier : List A) (valid : Atoms A → Prop)
    {first last : Cube A} {count : Nat}
    (steps : ConsequenceSteps carrier valid first last count) :
    count + openCount carrier last ≤ openCount carrier first := by
  induction steps with
  | done c => simp
  | @next c closed final count inside closurePreserves pivot member fresh forced rest ih =>
    have monotone := open_count_mono carrier inside
    have strict := falsify_decreases_open carrier closed pivot member fresh
    omega

/-- There cannot be successful decision prefixes of every finite length from
one finite region. The operation can still stop earlier for an incomplete cause;
this theorem does not manufacture a completed query or an exhaustion verdict. -/
theorem no_unbounded_steps (carrier : List A) (valid : Atoms A → Prop) (first : Cube A) :
    ¬ ∀ count, ∃ last, ConsequenceSteps carrier valid first last count := by
  intro unbounded
  obtain ⟨last, steps⟩ := unbounded (openCount carrier first + 1)
  have bound := steps_bounded carrier valid steps
  omega

/-- Two open atoms can hide a violating interpretation without a unit witness.
Even a complete search for these sufficient consequences cannot replace the
final original-satisfaction check or establish an answer set. -/
theorem no_unit_can_hide_violation :
    let c : Cube Bool := ⟨Empty, Full⟩
    let p : Literal Bool := ⟨.positive, false⟩
    let q : Literal Bool := ⟨.positive, true⟩
    ¬ UnitBody c [] p [q] ∧ ¬ UnitBody c [p] q [] ∧ c.Contains Full ∧
      ¬ Models Full [Ferraris.Neg (body true [p, q])] := by
  simp [UnitBody, Sure, FormulaBounds.read, Literal.formula, Cube.Fresh,
    Cube.Contains, Sub, Empty, Full, Models, body, NormalFerraris.conjunction,
    Ferraris.Neg, Satisfies]

/-- A stopped scan after a sound decision can still retain an original answer.
Even useful narrowing cannot turn interruption into refutation or exhaustion. -/
theorem pending_after_decision_retains_answer :
    let c : Cube Bool := ⟨Empty, Full⟩
    StreamedConstraints.scan (fun _ : Bool => false) 0 [true] = .pending [true] ∧
      (falsify c ⟨.positive, false⟩).Contains Empty ∧
      Stable Empty [Ferraris.Neg (.atom false), Ferraris.Neg (.atom true)] := by
  simp [StreamedConstraints.scan, falsify, Cube.Contains, Cube.splitFalse, Sub,
    Empty, Full, Stable, Models, Ferraris.Neg, Satisfies, ProperSub]

end Zetesis.StreamedConsequences
