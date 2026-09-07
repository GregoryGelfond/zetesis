import Zetesis.Lifted
import Zetesis.Semantics

/-!
# Lifted-to-ground semantic bridge

`ConceptualGrounding` identifies a finite semantic program with all instances of
registered headed and constraint templates. It is a semantic source-coverage
premise, not a demand to enumerate these instances at runtime. Binding types
remain arbitrary; programs admitted by the finite source profile must establish
that this finite conceptual program exists.

Theorems here connect the lifted composition and materialization contract to the
independent reduct/minimal-model semantics in `Semantics`. They do not establish
that themelios parsing, source normalization, Rust joins, or a device implement
this contract.
-/

namespace Zetesis.LiftedBridge

open Lifted

universe u v₁ v₂ v₃ w₁ w₂

variable {α : Type u} {ι : Type w₁} {κ : Type w₂}
  {β : ι → Type v₁} {δ : κ → Type v₂} {ε : Type v₃}

def InstantiateHeaded (t : Template α ε) (b : ε) : Semantics.Rule α where
  head := some (t.head b)
  positive := t.positive b
  gateTrue := t.gateTrue b
  gateFalse := t.gateFalse b
  filter := t.filter b

/-- Constraint templates use the same body pipeline; their head is ignored. -/
def InstantiateConstraint (t : Template α ε) (b : ε) : Semantics.Rule α where
  head := none
  positive := t.positive b
  gateTrue := t.gateTrue b
  gateFalse := t.gateFalse b
  filter := t.filter b

def ConceptualGrounding (P : Semantics.Program α)
    (headed : (i : ι) → Template α (β i))
    (constraints : (j : κ) → Template α (δ j)) : Prop :=
  ∀ r, r ∈ P ↔
    (∃ i b, InstantiateHeaded (headed i) b = r) ∨
    (∃ j b, InstantiateConstraint (constraints j) b = r)

theorem family_consequence_ground (P : Semantics.Program α)
    (headed : (i : ι) → Template α (β i))
    (constraints : (j : κ) → Template α (δ j))
    (hground : ConceptualGrounding P headed constraints) (z : Atoms α) :
    TEquivalent (fun X => FamilyConsequence headed z X) (Semantics.Consequence P z) := by
  intro X a
  constructor
  · rintro ⟨i, b, hf, hg, hb, hh⟩
    refine ⟨InstantiateHeaded (headed i) b,
      (hground _).mpr (Or.inl ⟨i, b, rfl⟩), ?_, hf, hg, hb⟩
    exact congrArg some hh
  · rintro ⟨r, hr, hh, hf, hg, hb⟩
    rcases (hground r).mp hr with ⟨i, b, rfl⟩ | ⟨j, b, rfl⟩
    · exact ⟨i, b, hf, hg, hb, Option.some.inj hh⟩
    · simp [InstantiateConstraint] at hh

theorem family_constraints_ground (P : Semantics.Program α)
    (headed : (i : ι) → Template α (β i))
    (constraints : (j : κ) → Template α (δ j))
    (hground : ConceptualGrounding P headed constraints) (z X : Atoms α) :
    (∀ j, ¬ ConstraintTriggered (constraints j) z X) ↔
      Semantics.ConstraintsOK P z X := by
  constructor
  · intro h r hr hh hf hg hb
    rcases (hground r).mp hr with ⟨i, b, rfl⟩ | ⟨j, b, rfl⟩
    · simp [InstantiateHeaded] at hh
    · exact h j ⟨b, hf, hg, hb⟩
  · intro h j hj
    rcases hj with ⟨b, hf, hg, hb⟩
    exact h (InstantiateConstraint (constraints j) b)
      ((hground _).mpr (Or.inr ⟨j, b, rfl⟩)) rfl hf hg hb

theorem lifted_gamma_ground (P : Semantics.Program α)
    (headed : (i : ι) → Template α (β i))
    (constraints : (j : κ) → Template α (δ j))
    (hground : ConceptualGrounding P headed constraints) (z : Atoms α) :
    Least (fun X => FamilyConsequence headed z X) = Semantics.Gamma P z :=
  least_congr (family_consequence_ground P headed constraints hground z)

/-- The denotational least closure of the snapshot-dependent materializer.
    Under `SourceCoverage` at every input it is exactly the ordinary reduct
    closure. This definition by itself is not an executable lazy algorithm. -/
def LazyGamma (headed : (i : ι) → Template α (β i)) (z : Atoms α)
    (selected : Atoms α → (i : ι) → Bindings (β i)) : Atoms α :=
  Least (fun X => MaterializedFamily headed z X (selected X))

theorem lazy_gamma_ground (P : Semantics.Program α)
    (headed : (i : ι) → Template α (β i))
    (constraints : (j : κ) → Template α (δ j))
    (hground : ConceptualGrounding P headed constraints) (z : Atoms α)
    (selected : Atoms α → (i : ι) → Bindings (β i))
    (hcover : ∀ X i, SourceCoverage (headed i) z X (selected X i)) :
    LazyGamma headed z selected = Semantics.Gamma P z := by
  apply least_congr
  intro X a
  exact (family_exact headed z X (selected X) (hcover X) a).trans
    (family_consequence_ground P headed constraints hground z X a)

/-- Every finite stage consists only of justified consequences, even when
    earlier materialization rounds are incomplete. No coverage is assumed. -/
theorem lazy_stage_sound (P : Semantics.Program α)
    (headed : (i : ι) → Template α (β i))
    (constraints : (j : κ) → Template α (δ j))
    (hground : ConceptualGrounding P headed constraints) (z : Atoms α)
    (selected : Nat → Atoms α → (i : ι) → Bindings (β i)) :
    ∀ n, Sub (LazyStage headed z selected n) (Semantics.Gamma P z) := by
  intro n
  induction n with
  | zero =>
    intro a ha
    exact False.elim ha
  | succ n ih =>
    intro a ha
    change LazyStage headed z selected n a ∨
      MaterializedFamily headed z (LazyStage headed z selected n)
        (selected n (LazyStage headed z selected n)) a at ha
    rcases ha with hold | hnew
    · exact ih a hold
    · rcases hnew with ⟨i, hi⟩
      have hsource : FamilyConsequence headed z (LazyStage headed z selected n) a :=
        ⟨i, materialized_sound (headed i) z _ _ a hi⟩
      have hgroundConsequence :=
        (family_consequence_ground P headed constraints hground z _ a).mp hsource
      exact Semantics.gamma_closed P z a
        (Semantics.consequence_mono P z ih a hgroundConsequence)

/-- Operational lazy completion needs coverage only at the final snapshot.
    All earlier rounds may be incomplete. Sound construction from the empty
    relation establishes leastness; covered final closedness establishes that
    no source consequence is missing. Neither queue emptiness nor cyclic
    support alone supplies these premises. -/
theorem completed_lazy_stage_exact (P : Semantics.Program α)
    (headed : (i : ι) → Template α (β i))
    (constraints : (j : κ) → Template α (δ j))
    (hground : ConceptualGrounding P headed constraints) (z : Atoms α)
    (selected : Nat → Atoms α → (i : ι) → Bindings (β i)) (n : Nat)
    (hcover : ∀ i, SourceCoverage (headed i) z (LazyStage headed z selected n)
      (selected n (LazyStage headed z selected n) i))
    (hclosed : Sub (MaterializedFamily headed z (LazyStage headed z selected n)
      (selected n (LazyStage headed z selected n))) (LazyStage headed z selected n)) :
    LazyStage headed z selected n = Semantics.Gamma P z := by
  apply exact_of_sound_and_closed
  · exact lazy_stage_sound P headed constraints hground z selected n
  · intro a ha
    have hfamily :=
      (family_consequence_ground P headed constraints hground z _ a).mpr ha
    exact hclosed a
      ((family_exact headed z _ _ hcover a).mpr hfamily)

/-- A completed operational lazy run can be accepted using final-snapshot
    headed and constraint coverage. No coverage obligation is imposed on
    unvisited states or incomplete earlier stages. -/
theorem completed_lazy_stage_accept_sound (P : Semantics.Program α)
    (headed : (i : ι) → Template α (β i))
    (constraints : (j : κ) → Template α (δ j))
    (hground : ConceptualGrounding P headed constraints) (S z : Atoms α)
    (hcarrier : Semantics.GateCarrier P S)
    (selected : Nat → Atoms α → (i : ι) → Bindings (β i)) (n : Nat)
    (hcover : ∀ i, SourceCoverage (headed i) z (LazyStage headed z selected n)
      (selected n (LazyStage headed z selected n) i))
    (hclosed : Sub (MaterializedFamily headed z (LazyStage headed z selected n)
      (selected n (LazyStage headed z selected n))) (LazyStage headed z selected n))
    (constraintSelection : (j : κ) → Bindings (δ j))
    (hconstraintCover : ∀ j, SourceCoverage (constraints j) z
      (LazyStage headed z selected n) (constraintSelection j))
    (hseed : Inter (LazyStage headed z selected n) S = z)
    (hconstraints : ∀ j, ¬ MaterializedConstraint (constraints j) z
      (LazyStage headed z selected n) (constraintSelection j)) :
    Semantics.Stable P (LazyStage headed z selected n) := by
  have hgamma := completed_lazy_stage_exact P headed constraints hground z
    selected n hcover hclosed
  have haccept : Semantics.Accept P S z := by
    constructor
    · rw [← hgamma]
      exact hseed
    · rw [← hgamma]
      apply (family_constraints_ground P headed constraints hground z _).mp
      intro j hj
      exact hconstraints j
        ((materialized_constraint_exact _ _ _ _ (hconstraintCover j)).mpr hj)
  rw [hgamma]
  exact Semantics.accept_sound hcarrier haccept

/-- An exact lazy transformer oracle, including covered constraint checks,
    reconstructs a stable model of the conceptual ground program. The theorem
    assumes the global semantic materializer contract; finite execution and
    concrete joins require their own refinement proofs. -/
theorem lazy_accept_sound (P : Semantics.Program α)
    (headed : (i : ι) → Template α (β i))
    (constraints : (j : κ) → Template α (δ j))
    (hground : ConceptualGrounding P headed constraints) (S z : Atoms α)
    (hcarrier : Semantics.GateCarrier P S)
    (selected : Atoms α → (i : ι) → Bindings (β i))
    (hcover : ∀ X i, SourceCoverage (headed i) z X (selected X i))
    (constraintSelection : (j : κ) → Bindings (δ j))
    (hconstraintCover : ∀ j, SourceCoverage (constraints j) z
      (LazyGamma headed z selected) (constraintSelection j))
    (hseed : Inter (LazyGamma headed z selected) S = z)
    (hconstraints : ∀ j, ¬ MaterializedConstraint (constraints j) z
      (LazyGamma headed z selected) (constraintSelection j)) :
    Semantics.Stable P (LazyGamma headed z selected) := by
  have hgamma := lazy_gamma_ground P headed constraints hground z selected hcover
  have haccept : Semantics.Accept P S z := by
    constructor
    · rw [← hgamma]
      exact hseed
    · rw [← hgamma]
      apply (family_constraints_ground P headed constraints hground z _).mp
      intro j hj
      exact hconstraints j
        ((materialized_constraint_exact _ _ _ _ (hconstraintCover j)).mpr hj)
  rw [hgamma]
  exact Semantics.accept_sound hcarrier haccept

end Zetesis.LiftedBridge
