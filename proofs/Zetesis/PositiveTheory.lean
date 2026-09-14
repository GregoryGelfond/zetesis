import Zetesis.Ferraris

/-!
# Least consequences for positive atomic-head theories

Bodies use atoms, falsum, conjunction, disjunction and canonical truth. Original
roots are atomic facts, body-to-atom producers, body-to-falsum constraints, or
falsum. Positive cycles are allowed. Choices and other implication bodies are
outside this grammar.

The least consequences are defined as the intersection of all producer-closed
interpretations. Monotonicity makes that intersection closed. A completed theory
has exactly that answer set when its constraints hold there; otherwise it has no
model. No finite iteration schedule, DAG/CSR representation, Rust allocation or
source-to-theory correspondence is proved here. The guide separates those
implementation obligations from these semantic laws.
-/

namespace Zetesis.PositiveTheory

open Ferraris

universe u
variable {A : Type u}

/-- Exactly the monotone body grammar; truth has its original formula spelling. -/
inductive Positive : Formula A → Prop where
  | atom (a : A) : Positive (.atom a)
  | bot : Positive .bot
  | truth : Positive (.imp .bot .bot)
  | conj {F G : Formula A} : Positive F → Positive G → Positive (.conj F G)
  | disj {F G : Formula A} : Positive F → Positive G → Positive (.disj F G)

/-- The complete original-root grammar. Atomic heads distinguish this class
from arbitrary positive theories with several incomparable minimal models. -/
inductive Admitted : Formula A → Prop where
  | fact (a : A) : Admitted (.atom a)
  | falsum : Admitted .bot
  | rule {F : Formula A} (a : A) : Positive F → Admitted (.imp F (.atom a))
  | constraint {F : Formula A} : Positive F → Admitted (.imp F .bot)

/-- Positive body truth is preserved when true atoms are added. Induct on the
body: atoms use containment, binary cases use their child claims, constants do
not depend on the interpretation. -/
theorem positive_monotone {F : Formula A} (positive : Positive F)
    {J M : Atoms A} (subset : Sub J M) : Satisfies J F → Satisfies M F := by
  induction positive with
  | atom a => exact subset a
  | bot => exact fun impossible => impossible
  | truth => exact fun _ impossible => impossible
  | conj left right leftInduction rightInduction =>
    exact fun holds => ⟨leftInduction holds.1, rightInduction holds.2⟩
  | disj left right leftInduction rightInduction =>
    intro holds
    rcases holds with leftHolds | rightHolds
    · exact Or.inl (leftInduction leftHolds)
    · exact Or.inr (rightInduction rightHolds)

/-- Below the frozen candidate, a positive body's reduct has its original truth.
If a compound was false in the candidate, monotonicity makes it false in every
subset. Otherwise apply the two child equivalences. -/
theorem positive_reduct_exact {F : Formula A} (positive : Positive F)
    {J M : Atoms A} (subset : Sub J M) :
    Satisfies J (Reduct M F) ↔ Satisfies J F := by
  classical
  induction positive with
  | atom a =>
    rw [atom_reduct]
    exact ⟨And.right, fun present => ⟨subset a present, present⟩⟩
  | bot => rfl
  | truth => simp [Reduct, Satisfies]
  | @conj F G left right leftInduction rightInduction =>
    by_cases original : Satisfies M (.conj F G)
    · rw [Reduct, if_pos original]
      exact and_congr leftInduction rightInduction
    · have absent : ¬ Satisfies J (.conj F G) :=
        fun holds => original (positive_monotone (.conj left right) subset holds)
      rw [Reduct, if_neg original]
      exact ⟨False.elim, fun holds => False.elim (absent holds)⟩
  | @disj F G left right leftInduction rightInduction =>
    by_cases original : Satisfies M (.disj F G)
    · rw [Reduct, if_pos original]
      exact or_congr leftInduction rightInduction
    · have absent : ¬ Satisfies J (.disj F G) :=
        fun holds => original (positive_monotone (.disj left right) subset holds)
      rw [Reduct, if_neg original]
      exact ⟨False.elim, fun holds => False.elim (absent holds)⟩

/-- Producer obligations only; constraints deliberately add no consequences. -/
def Produces (M : Atoms A) : Formula A → Prop
  | .atom a => M a
  | .imp F (.atom a) => Satisfies M F → M a
  | _ => True

/-- Constraint obligations only, separate from positive-head closure. -/
def Constraint (M : Atoms A) : Formula A → Prop
  | .bot => False
  | .imp F .bot => ¬ Satisfies M F
  | _ => True

def Closed (M : Atoms A) (T : Theory A) : Prop :=
  ∀ F ∈ T, Produces M F

def Constraints (M : Atoms A) (T : Theory A) : Prop :=
  ∀ F ∈ T, Constraint M F

/-- Intersection of all producer-closed interpretations, ignoring constraints. -/
def Least (T : Theory A) : Atoms A :=
  fun a => ∀ M, Closed M T → M a

/-- By definition, least consequences lie inside every producer-closed set. -/
theorem least_sub_closed {T : Theory A} {M : Atoms A} (closed : Closed M T) :
    Sub (Least T) M := by
  intro atom present
  exact present M closed

/-- For the admitted root grammar, modeling separates into producer closure and
constraint satisfaction. This is a whole-root equivalence, not a projection. -/
theorem models_iff_closed_constraints (T : Theory A) (M : Atoms A)
    (admitted : ∀ F ∈ T, Admitted F) :
    Models M T ↔ Closed M T ∧ Constraints M T := by
  have root_exact : ∀ F ∈ T,
      Satisfies M F ↔ Produces M F ∧ Constraint M F := by
    intro F member
    cases admitted F member <;> simp [Satisfies, Produces, Constraint]
  constructor
  · intro models
    exact ⟨fun F member => ((root_exact F member).mp (models F member)).1,
      fun F member => ((root_exact F member).mp (models F member)).2⟩
  · rintro ⟨closed, constraints⟩ F member
    exact (root_exact F member).mpr ⟨closed F member, constraints F member⟩

/-- Least consequences are themselves producer-closed. A true body in the
intersection is true in every closed interpretation by monotonicity, so its
atomic head belongs to their intersection. No acyclic rank is assumed. -/
theorem least_closed (T : Theory A) (admitted : ∀ F ∈ T, Admitted F) :
    Closed (Least T) T := by
  intro F member
  cases admitted F member with
  | fact a =>
    intro M closed
    exact closed (.atom a) member
  | falsum => trivial
  | @rule body a positive =>
    intro holds M closed
    exact closed (.imp body (.atom a)) member
      (positive_monotone positive (least_sub_closed closed) holds)
  | constraint positive => trivial

/-- Positive constraints satisfied by a model remain satisfied in its subsets.
A violating smaller body would stay true in the larger set by monotonicity. -/
theorem constraints_downward (T : Theory A) (admitted : ∀ F ∈ T, Admitted F)
    {J M : Atoms A} (subset : Sub J M) (constraints : Constraints M T) :
    Constraints J T := by
  intro F member
  cases admitted F member with
  | fact a => trivial
  | falsum => exact constraints .bot member
  | rule a positive => trivial
  | @constraint body positive =>
    intro holds
    exact constraints (.imp body .bot) member (positive_monotone positive subset holds)

/-- An admitted original root true in M agrees with its reduct below M. The
outer implication is retained; positive body and atomic-head reducts agree by
the positive-body law. -/
theorem root_reduct_exact {F : Formula A} (admitted : Admitted F)
    {J M : Atoms A} (subset : Sub J M) (original : Satisfies M F) :
    Satisfies J (Reduct M F) ↔ Satisfies J F := by
  classical
  cases admitted with
  | fact a => exact positive_reduct_exact (.atom a) subset
  | falsum => exact False.elim original
  | @rule body a positive =>
    rw [Reduct, if_pos original]
    change (Satisfies J (Reduct M body) → Satisfies J (Reduct M (.atom a))) ↔
      (Satisfies J body → J a)
    rw [positive_reduct_exact positive subset,
      positive_reduct_exact (Positive.atom a) subset]
    rfl
  | @constraint body positive =>
    rw [Reduct, if_pos original]
    change (Satisfies J (Reduct M body) → False) ↔ (Satisfies J body → False)
    rw [positive_reduct_exact positive subset]

/-- Whole admitted-theory satisfaction agrees with the frozen reduct below a
model of the original theory. Every original root is covered explicitly. -/
theorem models_reduct_exact (T : Theory A) (admitted : ∀ F ∈ T, Admitted F)
    {J M : Atoms A} (subset : Sub J M) (original : Models M T) :
    Models J (ReductTheory M T) ↔ Models J T := by
  constructor
  · intro reduced F member
    exact (root_reduct_exact (admitted F member) subset (original F member)).mp
      (reduced (Reduct M F) (List.mem_map.mpr ⟨F, member, rfl⟩))
  · intro models F member
    obtain ⟨root, present, rfl⟩ := List.mem_map.mp member
    exact (root_reduct_exact (admitted root present) subset (original root present)).mpr
      (models root present)

/-- The theory has exactly its least-consequence answer set when that set
satisfies its constraints. Any stable model contains the least set, which is a
model of its reduct; minimality forces equality. Conversely, no proper subset of
the least closed set can model its reduct, because it would be producer-closed. -/
theorem stable_iff_least_constraints (T : Theory A)
    (admitted : ∀ F ∈ T, Admitted F) (M : Atoms A) :
    Stable M T ↔ M = Least T ∧ Constraints (Least T) T := by
  classical
  constructor
  · intro stable
    have parts : Closed M T ∧ Constraints M T :=
      (models_iff_closed_constraints T M admitted).mp stable.1
    have subset : Sub (Least T) M := least_sub_closed parts.1
    have constraints : Constraints (Least T) T :=
      constraints_downward T admitted subset parts.2
    have leastModels : Models (Least T) T :=
      (models_iff_closed_constraints T (Least T) admitted).mpr
        ⟨least_closed T admitted, constraints⟩
    have reverse : Sub M (Least T) := by
      apply Classical.byContradiction
      intro absent
      exact stable.2 ⟨Least T, ⟨subset, absent⟩,
        (models_reduct_exact T admitted subset stable.1).mpr leastModels⟩
    have equality : M = Least T := by
      apply atoms_ext
      intro atom
      exact ⟨reverse atom, subset atom⟩
    exact ⟨equality, constraints⟩
  · rintro ⟨rfl, constraints⟩
    have original : Models (Least T) T :=
      (models_iff_closed_constraints T (Least T) admitted).mpr
        ⟨least_closed T admitted, constraints⟩
    refine ⟨original, ?_⟩
    rintro ⟨J, ⟨subset, proper⟩, reduced⟩
    have models : Models J T := (models_reduct_exact T admitted subset original).mp reduced
    exact proper (least_sub_closed ((models_iff_closed_constraints T J admitted).mp models).1)

/-- Failure of a positive constraint at least consequences rules out every
classical model, hence every answer set. Every model contains the least set and
would pass its constraints downward, contradicting the failure. -/
theorem failed_constraint_no_model (T : Theory A)
    (admitted : ∀ F ∈ T, Admitted F) (failed : ¬ Constraints (Least T) T) :
    ¬ ∃ M, Models M T := by
  rintro ⟨M, models⟩
  have parts : Closed M T ∧ Constraints M T :=
    (models_iff_closed_constraints T M admitted).mp models
  exact failed (constraints_downward T admitted (least_sub_closed parts.1) parts.2)

end Zetesis.PositiveTheory
