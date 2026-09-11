import Zetesis.Core

/-!
# Propositional formula reducts

This is a separate semantic foundation for a broader language profile. Formulas
are finite syntax trees of atoms, falsum, conjunction, disjunction, and
implication. The reduct replaces every maximal subformula false in the frozen
interpretation by falsum; recursion continues only through true subformulas.

The stable-model definition is subset minimality of this formula reduct, not
equality with a Horn least-consequence operator. The final counterexample shows
why an arbitrary positive/disjunctive reduct cannot inherit S0's least-model
checker. Aggregate-to-formula translation, the full clingo language, and the
Rust implementation are not proved by this module.
-/

namespace Zetesis.Ferraris

universe u

/-- Finite propositional formulas. Default negation is defined using implication
    to falsum; it is not a primitive Boolean complement in the reduct. -/
inductive Formula (α : Type u) where
  | atom : α → Formula α
  | bot : Formula α
  | conj : Formula α → Formula α → Formula α
  | disj : Formula α → Formula α → Formula α
  | imp : Formula α → Formula α → Formula α

variable {α : Type u}

def Satisfies (M : Atoms α) : Formula α → Prop
  | .atom a => M a
  | .bot => False
  | .conj F G => Satisfies M F ∧ Satisfies M G
  | .disj F G => Satisfies M F ∨ Satisfies M G
  | .imp F G => Satisfies M F → Satisfies M G

def Neg (F : Formula α) : Formula α := .imp F .bot

open Classical in
/-- Recurse through an M-true formula and replace an M-false root by falsum.
    The propositional membership predicates need not be computationally decidable;
    this definition is denotational and is therefore noncomputable. -/
noncomputable def Reduct (M : Atoms α) : Formula α → Formula α
  | .atom a => if M a then .atom a else .bot
  | .bot => .bot
  | .conj F G => if Satisfies M (.conj F G)
      then .conj (Reduct M F) (Reduct M G) else .bot
  | .disj F G => if Satisfies M (.disj F G)
      then .disj (Reduct M F) (Reduct M G) else .bot
  | .imp F G => if Satisfies M (.imp F G)
      then .imp (Reduct M F) (Reduct M G) else .bot

abbrev Theory (α : Type u) := List (Formula α)

def Models (M : Atoms α) (T : Theory α) : Prop :=
  ∀ F, F ∈ T → Satisfies M F

noncomputable def ReductTheory (M : Atoms α) (T : Theory α) : Theory α :=
  T.map (Reduct M)

def ProperSub (J M : Atoms α) : Prop := Sub J M ∧ ¬ Sub M J

def MinimalModel (M : Atoms α) (T : Theory α) : Prop :=
  Models M T ∧ ∀ J, Sub J M → Models J T → Sub M J

/-- An interpretation models the original theory and no proper subset models
    its frozen formula reduct. This definition does not assume a least model. -/
def Stable (M : Atoms α) (T : Theory α) : Prop :=
  Models M T ∧ ¬ ∃ J, ProperSub J M ∧ Models J (ReductTheory M T)

theorem reduct_self (M : Atoms α) (F : Formula α) :
    Satisfies M (Reduct M F) ↔ Satisfies M F := by
  classical
  induction F with
  | atom a => by_cases h : M a <;> simp [Reduct, Satisfies, h]
  | bot => rfl
  | conj F G ihF ihG =>
    by_cases hF : Satisfies M F <;> by_cases hG : Satisfies M G
      <;> simp_all [Reduct, Satisfies]
  | disj F G ihF ihG =>
    by_cases hF : Satisfies M F <;> by_cases hG : Satisfies M G
      <;> simp_all [Reduct, Satisfies]
  | imp F G ihF ihG =>
    by_cases hF : Satisfies M F <;> by_cases hG : Satisfies M G
      <;> simp_all [Reduct, Satisfies]

theorem models_reduct_self (M : Atoms α) (T : Theory α) :
    Models M (ReductTheory M T) ↔ Models M T := by
  constructor
  · intro h F hF
    apply (reduct_self M F).mp
    exact h (Reduct M F) (List.mem_map.mpr ⟨F, hF, rfl⟩)
  · intro h F hF
    obtain ⟨G, hG, rfl⟩ := List.mem_map.mp hF
    exact (reduct_self M G).mpr (h G hG)

theorem stable_iff_minimal_reduct (M : Atoms α) (T : Theory α) :
    Stable M T ↔ MinimalModel M (ReductTheory M T) := by
  classical
  constructor
  · intro ⟨hmodel, hminimal⟩
    refine ⟨(models_reduct_self M T).mpr hmodel, ?_⟩
    intro J hsub hj
    by_cases hback : Sub M J
    · exact hback
    · exact False.elim (hminimal ⟨J, ⟨hsub, hback⟩, hj⟩)
  · intro ⟨hmodel, hminimal⟩
    refine ⟨(models_reduct_self M T).mp hmodel, ?_⟩
    rintro ⟨J, ⟨hsub, hproper⟩, hj⟩
    exact hproper (hminimal J hsub hj)

/-- An answer set remains an answer set after adding formulas it satisfies.

The interpretation models both theories. Any proper-subset model of their
combined reduct would also model the original reduct, contradicting stability.
This law assumes one fixed formula translation; a source transformation whose
meaning changes with its surrounding program needs a separate correspondence.
-/
theorem stable_append_of_models (M : Atoms α) (T U : Theory α)
    (original : Stable M T) (additional : Models M U) :
    Stable M (T ++ U) := by
  constructor
  · intro formula member
    rcases List.mem_append.mp member with old | added
    · exact original.1 formula old
    · exact additional formula added
  · rintro ⟨J, proper, combined⟩
    apply original.2
    refine ⟨J, proper, ?_⟩
    intro formula member
    apply combined formula
    change formula ∈ (T ++ U).map (Reduct M)
    rw [List.map_append]
    exact List.mem_append.mpr (Or.inl member)

/-- Adding facts already contained in an answer set preserves that answer set.
Each added atomic formula is true in the interpretation, so the preceding
extension law applies. No new atom is assumed true in a smaller interpretation.
-/
theorem stable_append_facts (M : Atoms α) (T : Theory α) (facts : List α)
    (original : Stable M T) (contained : ∀ atom, atom ∈ facts → M atom) :
    Stable M (T ++ facts.map Formula.atom) := by
  apply stable_append_of_models M T _ original
  intro formula member
  obtain ⟨atom, present, rfl⟩ := List.mem_map.mp member
  exact contained atom present

theorem atom_reduct (M J : Atoms α) (a : α) :
    Satisfies J (Reduct M (.atom a)) ↔ M a ∧ J a := by
  classical
  by_cases h : M a <;> simp [Reduct, Satisfies, h]

/-- A negated atom has a frozen truth value in its formula reduct. -/
theorem neg_atom_reduct (M J : Atoms α) (a : α) :
    Satisfies J (Reduct M (Neg (.atom a))) ↔ ¬ M a := by
  classical
  by_cases h : M a <;> simp [Reduct, Neg, Satisfies, h]

/-- Double default negation becomes a frozen positive candidate condition,
    independent of the interpretation J being tested against the reduct. -/
theorem double_neg_atom_reduct (M J : Atoms α) (a : α) :
    Satisfies J (Reduct M (Neg (Neg (.atom a)))) ↔ M a := by
  classical
  by_cases h : M a <;> simp [Reduct, Neg, Satisfies, h]

/-- The propositional choice formula h ∨ not h reduces to the frozen guard
    M(h) → J(h). An M-false choice permits either J value; an M-true choice
    requires h in J. This is not an aggregate-translation theorem. -/
theorem choice_reduct_guard (M J : Atoms α) (a : α) :
    Satisfies J (Reduct M (.disj (.atom a) (Neg (.atom a)))) ↔ (M a → J a) := by
  classical
  by_cases h : M a <;> simp [Reduct, Neg, Satisfies, h]

def Only (a : Bool) : Atoms Bool := fun b => b = a

def DisjunctionExample : Formula Bool := .disj (.atom false) (.atom true)

def ExampleTheory : Theory Bool := [DisjunctionExample]

theorem only_false_minimal : MinimalModel (Only false) ExampleTheory := by
  constructor
  · intro F hF
    have heq : F = DisjunctionExample := by simpa [ExampleTheory] using hF
    subst F
    exact Or.inl rfl
  · intro J hsub hj a ha
    have hchoice : J false ∨ J true := hj DisjunctionExample (by simp [ExampleTheory])
    have heq : a = false := ha
    subst a
    rcases hchoice with hfalse | htrue
    · exact hfalse
    · have hbad : true = false := hsub true htrue
      cases hbad

theorem only_true_minimal : MinimalModel (Only true) ExampleTheory := by
  constructor
  · intro F hF
    have heq : F = DisjunctionExample := by simpa [ExampleTheory] using hF
    subst F
    exact Or.inr rfl
  · intro J hsub hj a ha
    have hchoice : J false ∨ J true := hj DisjunctionExample (by simp [ExampleTheory])
    have heq : a = true := ha
    subst a
    rcases hchoice with hfalse | htrue
    · have hbad : false = true := hsub false hfalse
      cases hbad
    · exact htrue

theorem incomparable_minimal_models :
    MinimalModel (Only false) ExampleTheory ∧ MinimalModel (Only true) ExampleTheory ∧
    ¬ Sub (Only false) (Only true) ∧ ¬ Sub (Only true) (Only false) := by
  refine ⟨only_false_minimal, only_true_minimal, ?_, ?_⟩
  · intro h
    have hbad : false = true := h false rfl
    cases hbad
  · intro h
    have hbad : true = false := h true rfl
    cases hbad

theorem no_least_model :
    ¬ ∃ L, Models L ExampleTheory ∧ ∀ M, Models M ExampleTheory → Sub L M := by
  rintro ⟨L, hmodel, hleast⟩
  have hchoice : L false ∨ L true := hmodel DisjunctionExample (by simp [ExampleTheory])
  rcases hchoice with hfalse | htrue
  · have hbad : false = true := hleast (Only true) only_true_minimal.1 false hfalse
    cases hbad
  · have hbad : true = false := hleast (Only false) only_false_minimal.1 true htrue
    cases hbad

/-- The counterexample is itself an actual frozen formula reduct: when both
    atoms belong to M, neither disjunct is replaced by falsum. -/
theorem example_is_full_reduct : ReductTheory Full ExampleTheory = ExampleTheory := by
  simp [ReductTheory, ExampleTheory, DisjunctionExample, Reduct, Satisfies, Full]

theorem reduct_has_no_least_model :
    ¬ ∃ L, Models L (ReductTheory Full ExampleTheory) ∧
      ∀ M, Models M (ReductTheory Full ExampleTheory) → Sub L M := by
  rw [example_is_full_reduct]
  exact no_least_model

end Zetesis.Ferraris
