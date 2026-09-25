import Zetesis.PositiveTheory
import Zetesis.StrongNegation
import Zetesis.NormalFerraris

/-!
# Terminal positive definitions

An arbitrary base theory uses atoms of type `A`. A finite list of definitions
adds heads of a disjoint type `D`, each with a positive conjunction of base atoms
as its body. The full atom type is `A ⊕ D`, so neither the base theory nor a
definition body can read a derived atom. Repeated body atoms, duplicate
definitions and several producers of one head are permitted.

Every base answer set has exactly one full answer set above it: add precisely
the heads of definitions whose bodies hold. Conversely, every full answer set
arises this way. The proof uses the existing generic atom-renaming laws and the
actual frozen Ferraris reduct, with no positivity restriction on the base.

This is a one-layer propositional extension law. Source partition recognition,
complete ground-instance enumeration, strong-negation coherence coverage,
canonical storage, reconstruction, bounded execution and Rust refinement remain
separate obligations. No recursive definition schedule is established here.
-/

namespace Zetesis.TerminalDefinitions

open Ferraris

universe u v
variable {A : Type u} {D : Type v}

/-- One producer occurrence. Its body reads only base atoms. -/
structure Definition (A : Type u) (D : Type v) where
  head : D
  body : List A

/-- The base part of a full interpretation. -/
def base (N : Atoms (A ⊕ D)) : Atoms A := fun a => N (.inl a)

/-- The derived part of a full interpretation. -/
def derived (N : Atoms (A ⊕ D)) : Atoms D := fun d => N (.inr d)

/-- A head is a consequence when at least one of its listed producers has a
true body. An empty body is true; an unproduced head is not a consequence. -/
def Consequences (rules : List (Definition A D)) (M : Atoms A) : Atoms D :=
  fun d => ∃ r ∈ rules, r.head = d ∧ ∀ a ∈ r.body, M a

/-- Retain the base interpretation and add exactly its definition consequences. -/
def extend (rules : List (Definition A D)) (M : Atoms A) : Atoms (A ⊕ D)
  | .inl a => M a
  | .inr d => Consequences rules M d

/-- The ordinary implication encoding, using truth for an empty conjunction. -/
def Definition.formula (r : Definition A D) : Formula (A ⊕ D) :=
  .imp (NormalFerraris.conjunction (r.body.map (fun a => .atom (.inl a))))
    (.atom (.inr r.head))

def definitions (rules : List (Definition A D)) : Theory (A ⊕ D) :=
  rules.map Definition.formula

/-- Lift the complete base theory and append every definition occurrence. -/
def program (T : Theory A) (rules : List (Definition A D)) : Theory (A ⊕ D) :=
  StrongNegation.renameTheory Sum.inl T ++ definitions rules

/-- A definition holds exactly when its positive base body implies its derived
head. Finite conjunction truth handles empty and repeated body occurrences. -/
theorem satisfies_definition (r : Definition A D) (N : Atoms (A ⊕ D)) :
    Satisfies N r.formula ↔
      ((∀ a ∈ r.body, base N a) → derived N r.head) := by
  simp only [Definition.formula, Satisfies, NormalFerraris.satisfies_conjunction,
    List.forall_mem_map, base, derived]

/-- Classical definition satisfaction requires all consequences, but does not
exclude extra derived atoms. Each producer witnesses one implication. -/
theorem models_definitions_iff (rules : List (Definition A D))
    (N : Atoms (A ⊕ D)) :
    Models N (definitions rules) ↔ Sub (Consequences rules (base N)) (derived N) := by
  constructor
  · intro models d consequence
    obtain ⟨r, member, sameHead, body⟩ := consequence
    have head : derived N r.head :=
      (satisfies_definition r N).mp
        (models r.formula (List.mem_map.mpr ⟨r, member, rfl⟩)) body
    exact sameHead ▸ head
  · intro closed F member
    obtain ⟨r, source, rfl⟩ := List.mem_map.mp member
    apply (satisfies_definition r N).mpr
    intro body
    exact closed r.head ⟨r, source, rfl, body⟩

/-- Original full-theory truth separates into base truth and containment of all
definition consequences. Renaming leaves the arbitrary base formulas intact. -/
theorem original_models_iff (T : Theory A) (rules : List (Definition A D))
    (N : Atoms (A ⊕ D)) :
    Models N (program T rules) ↔
      Models (base N) T ∧ Sub (Consequences rules (base N)) (derived N) := by
  exact (RuleFactorization.models_append N
    (StrongNegation.renameTheory Sum.inl T) (definitions rules)).trans
      (and_congr (StrongNegation.models_rename Sum.inl N T)
        (models_definitions_iff rules N))

/-- Below a candidate satisfying the definitions, the full frozen reduct has
exactly the base-reduct obligation and positive consequence containment.

Rename the actual base reduct. Each definition is an admitted positive rule, so
its reduct agrees with its original implication on subsets of the candidate.
The right-hand containment cannot be strengthened to equality: subset reduct
models may still contain unsupported derived atoms.
-/
theorem reduct_models_iff (T : Theory A) (rules : List (Definition A D))
    (N J : Atoms (A ⊕ D)) (subset : Sub J N)
    (original : Models N (definitions rules)) :
    Models J (ReductTheory N (program T rules)) ↔
      Models (base J) (ReductTheory (base N) T) ∧
      Sub (Consequences rules (base J)) (derived J) := by
  have admitted : ∀ F ∈ definitions rules, PositiveTheory.Admitted F := by
    have positiveFold : ∀ (rest : List (Formula (A ⊕ D))) (first : Formula (A ⊕ D)),
        PositiveTheory.Positive first → (∀ F ∈ rest, PositiveTheory.Positive F) →
        PositiveTheory.Positive (rest.foldl Formula.conj first) := by
      intro rest first initial positive
      induction rest generalizing first with
      | nil => exact initial
      | cons next rest ih =>
        have nextPositive : PositiveTheory.Positive next := positive next (by simp)
        have restPositive : ∀ F ∈ rest, PositiveTheory.Positive F :=
          fun F member => positive F (List.mem_cons_of_mem next member)
        exact ih (.conj first next) (.conj initial nextPositive) restPositive
    have positiveConjunction : ∀ formulas : List (Formula (A ⊕ D)),
        (∀ F ∈ formulas, PositiveTheory.Positive F) →
        PositiveTheory.Positive (NormalFerraris.conjunction formulas) := by
      intro formulas positive
      cases formulas with
      | nil => exact .truth
      | cons first rest =>
        exact positiveFold rest first (positive first (by simp))
          (fun F member => positive F (List.mem_cons_of_mem first member))
    intro F member
    obtain ⟨r, _, rfl⟩ := List.mem_map.mp member
    have definitionAdmitted : PositiveTheory.Admitted r.formula := by
      apply PositiveTheory.Admitted.rule
      apply positiveConjunction
      intro G occurrence
      obtain ⟨a, _, rfl⟩ := List.mem_map.mp occurrence
      exact PositiveTheory.Positive.atom (Sum.inl a : A ⊕ D)
    exact definitionAdmitted
  have definitionReduct : Models J (ReductTheory N (definitions rules)) ↔
      Models J (definitions rules) :=
    PositiveTheory.models_reduct_exact (definitions rules) admitted subset original
  have split : ReductTheory N (program T rules) =
      ReductTheory N (StrongNegation.renameTheory Sum.inl T) ++
        ReductTheory N (definitions rules) := by
    simp only [program, ReductTheory, List.map_append]
  rw [split, RuleFactorization.models_append]
  exact and_congr (StrongNegation.frozen_theory_rename Sum.inl N J T)
    (definitionReduct.trans (models_definitions_iff rules J))

/-- Enlarging the base interpretation preserves every true positive body and
therefore every consequence; the same producer remains its witness. -/
theorem consequences_monotone (rules : List (Definition A D))
    {K M : Atoms A} (subset : Sub K M) :
    Sub (Consequences rules K) (Consequences rules M) := by
  rintro d ⟨r, member, sameHead, body⟩
  exact ⟨r, member, sameHead, fun a present => subset a (body a present)⟩

/-- Extending an interpretation does not change its base part. -/
theorem base_extend (rules : List (Definition A D)) (M : Atoms A) :
    base (extend rules M) = M := rfl

/-- The derived part of an extension is exactly its consequences. -/
theorem derived_extend (rules : List (Definition A D)) (M : Atoms A) :
    derived (extend rules M) = Consequences rules M := rfl

/-- Positive consequence extension preserves containment of base interpretations. -/
theorem extend_monotone (rules : List (Definition A D))
    {K M : Atoms A} (subset : Sub K M) :
    Sub (extend rules K) (extend rules M) := by
  intro atom present
  cases atom with
  | inl a => exact subset a present
  | inr d => exact consequences_monotone rules subset d present

/-- Every classical definition model contains the exact extension of its base.
This establishes containment only; stability will remove any extra atoms. -/
theorem extend_sub_of_models (rules : List (Definition A D))
    (N : Atoms (A ⊕ D)) (original : Models N (definitions rules)) :
    Sub (extend rules (base N)) N := by
  have closed : Sub (Consequences rules (base N)) (derived N) :=
    (models_definitions_iff rules N).mp original
  intro atom present
  cases atom with
  | inl a => exact present
  | inr d => exact closed d present

/-- Full answer sets are exactly base answer sets extended by their consequences.

For a full answer set, its exact extension is a contained reduct model with the
same base. Minimality removes all extra derived atoms. Any smaller base reduct
model would then extend to a contained full reduct model, proving base stability.
Conversely, a contained full reduct model projects to a contained base reduct
model. Base minimality fixes that projection, and the definition obligations
force every consequence back in. Thus the full extension is minimal as well.
-/
theorem stable_iff (T : Theory A) (rules : List (Definition A D))
    (N : Atoms (A ⊕ D)) :
    Stable N (program T rules) ↔
      Stable (base N) T ∧ N = extend rules (base N) := by
  constructor
  · intro stable
    have original : Models (base N) T ∧
        Sub (Consequences rules (base N)) (derived N) :=
      (original_models_iff T rules N).mp stable.1
    have definitionModels : Models N (definitions rules) :=
      (models_definitions_iff rules N).mpr original.2
    have minimal : MinimalModel N (ReductTheory N (program T rules)) :=
      (stable_iff_minimal_reduct N (program T rules)).mp stable
    have extensionSubset : Sub (extend rules (base N)) N :=
      extend_sub_of_models rules N definitionModels
    have extensionReduct :
        Models (extend rules (base N)) (ReductTheory N (program T rules)) :=
      (reduct_models_iff T rules N _ extensionSubset definitionModels).mpr
        ⟨(models_reduct_self (base N) T).mpr original.1, sub_refl _⟩
    have exactExtension : N = extend rules (base N) :=
      sub_antisymm (minimal.2 _ extensionSubset extensionReduct) extensionSubset
    have baseStable : Stable (base N) T := by
      apply (stable_iff_minimal_reduct (base N) T).mpr
      refine ⟨(models_reduct_self (base N) T).mpr original.1, ?_⟩
      intro K subset reduced
      have fullSubset : Sub (extend rules K) N :=
        sub_trans (extend_monotone rules subset) extensionSubset
      have fullReduct : Models (extend rules K) (ReductTheory N (program T rules)) :=
        (reduct_models_iff T rules N _ fullSubset definitionModels).mpr
          ⟨reduced, sub_refl _⟩
      have fullBack : Sub N (extend rules K) := minimal.2 _ fullSubset fullReduct
      exact fun a present => fullBack (.inl a) present
    exact ⟨baseStable, exactExtension⟩
  · intro ⟨baseStable, exactExtension⟩
    have extensionStable : Stable (extend rules (base N)) (program T rules) := by
      have original : Models (extend rules (base N)) (program T rules) :=
        (original_models_iff T rules _).mpr ⟨baseStable.1, sub_refl _⟩
      have definitionModels : Models (extend rules (base N)) (definitions rules) :=
        (models_definitions_iff rules _).mpr (sub_refl _)
      have baseMinimal : MinimalModel (base N) (ReductTheory (base N) T) :=
        (stable_iff_minimal_reduct (base N) T).mp baseStable
      apply (stable_iff_minimal_reduct (extend rules (base N)) (program T rules)).mpr
      refine ⟨(models_reduct_self _ _).mpr original, ?_⟩
      intro J subset reduced
      have parts : Models (base J) (ReductTheory (base N) T) ∧
          Sub (Consequences rules (base J)) (derived J) :=
        (reduct_models_iff T rules _ J subset definitionModels).mp reduced
      have baseSubset : Sub (base J) (base N) :=
        fun a present => subset (.inl a) present
      have baseBack : Sub (base N) (base J) := baseMinimal.2 _ baseSubset parts.1
      intro atom present
      cases atom with
      | inl a => exact baseBack a present
      | inr d => exact parts.2 d (consequences_monotone rules baseBack d present)
    exact exactExtension.symm ▸ extensionStable

/-- Exact consequence extension preserves and reflects base answer-set membership. -/
theorem stable_extend_iff (T : Theory A) (rules : List (Definition A D))
    (M : Atoms A) :
    Stable (extend rules M) (program T rules) ↔ Stable M T := by
  constructor
  · intro stable
    exact ((stable_iff T rules (extend rules M)).mp stable).1
  · intro stable
    exact (stable_iff T rules (extend rules M)).mpr ⟨stable, rfl⟩

/-- Every base answer set has one and only one full answer set above it.
Existence uses exact extension; the characterization fixes any other extension. -/
theorem unique_stable_extension (T : Theory A) (rules : List (Definition A D))
    (M : Atoms A) (stable : Stable M T) :
    ∃ N : Atoms (A ⊕ D), Stable N (program T rules) ∧ base N = M ∧
      ∀ other, Stable other (program T rules) → base other = M → other = N := by
  refine ⟨extend rules M, (stable_extend_iff T rules M).mpr stable, base_extend rules M, ?_⟩
  intro other full projection
  have exactExtension : other = extend rules (base other) :=
    ((stable_iff T rules other).mp full).2
  rw [projection] at exactExtension
  exact exactExtension

end Zetesis.TerminalDefinitions
