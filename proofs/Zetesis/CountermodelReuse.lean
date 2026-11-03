import Zetesis.PositiveTheory
import Zetesis.ReductRegions
import Zetesis.RuleFactorization

/-!
# Reusing a proper-subset witness across candidates

If a model of the reduct at one classical candidate still models the reduct at
a larger classical candidate, the same strict subset can refute the latter.
The fragment proved below allows positive disjunctive heads and default-negated
body atoms. Unrestricted implication does not enjoy this property, even when
falsum never occurs. These are semantic laws. Applying them to a shared
formula DAG would also require a sound fragment recognizer and an authenticated
original theory and witness.
-/

namespace Zetesis.LearnedRegions

open Ferraris
universe u
variable {α : Type u}

/-- Enlarging the frozen classical candidate preserves reduct models below the
smaller candidate. The original-model hypotheses permit ordinary rule roots
whose frozen implication is masked to falsum on a nonmodel. -/
def WitnessPersistent (T : Theory α) : Prop :=
  ∀ {N M M' : Atoms α}, Models M T → Models M' T → Sub N M → Sub M M' →
    Models N (ReductTheory M T) → Models N (ReductTheory M' T)

/-- A proper witness for a classical candidate refutes all its supersets if
the theory preserves witnesses. If a superset were stable it would itself
be a classical model, and the same witness would still be strictly smaller. -/
theorem antimonotone_superset_refutes {T : Theory α} (persistent : WitnessPersistent T)
    {N M M' : Atoms α} (model : Models M T) (witness : ReductRegions.Countermodel M T N)
    (grows : Sub M M') : ¬ Stable M' T := by
  intro stable
  have strict : ProperSub N M' :=
    ⟨sub_trans witness.1.1 grows, fun back => witness.1.2 (sub_trans grows back)⟩
  have survives : Models N (ReductTheory M' T) :=
    persistent model stable.1 witness.1.1 grows witness.2
  exact ReductRegions.leaf_refutes M' N T strict survives stable

/-- A rule body built from positive formulas, default-negated atoms and conjunction.
Positive formulas include atoms, falsum, truth and positive conjunctions and disjunctions. -/
inductive SignedBody : Formula α → Prop where
  | positive {F : Formula α} : PositiveTheory.Positive F → SignedBody F
  | absent (a : α) : SignedBody (Neg (.atom a))
  | conj {F G : Formula α} : SignedBody F → SignedBody G → SignedBody (.conj F G)

/-- On a fixed subset of the original candidate, a body true under the larger
freeze remains true under the smaller one. Positive formulas read that subset,
while absence of an atom in the larger candidate implies its absence in the smaller. -/
theorem signed_body_reduct_antitone {B : Formula α} (body : SignedBody B)
    {N M M' : Atoms α} (below : Sub N M) (grows : Sub M M') :
    Satisfies N (Reduct M' B) → Satisfies N (Reduct M B) := by
  induction body with
  | positive pos =>
    exact fun holds => (PositiveTheory.positive_reduct_exact pos below).mpr
      ((PositiveTheory.positive_reduct_exact pos (sub_trans below grows)).mp holds)
  | absent a =>
    rw [neg_atom_reduct, neg_atom_reduct]
    exact fun absent present => absent (grows a present)
  | conj _ _ left right =>
    rw [RuleFactorization.reduct_conj, RuleFactorization.reduct_conj]
    exact fun holds => ⟨left holds.1, right holds.2⟩

/-- Positive facts and rules with a signed body and positive head. A positive
disjunction is allowed as a head. Falsum as a head gives an integrity constraint. -/
inductive WitnessRule : Formula α → Prop where
  | fact {H : Formula α} : PositiveTheory.Positive H → WitnessRule H
  | rule {B H : Formula α} : SignedBody B → PositiveTheory.Positive H → WitnessRule (.imp B H)

/-- The rule grammar preserves proper-subset witnesses under a larger original
model. A frozen rule only needs its head when its body holds. Transfer that
body back to the smaller freeze, then transfer its positive head forward. -/
theorem witness_rules_persistent {T : Theory α} (rules : ∀ F ∈ T, WitnessRule F) :
    WitnessPersistent T := by
  intro N M M' _ original below grows smaller reduced member
  obtain ⟨F, inTheory, rfl⟩ := List.mem_map.mp member
  have old : Satisfies N (Reduct M F) :=
    smaller _ (List.mem_map.mpr ⟨F, inTheory, rfl⟩)
  have largerSubset : Sub N M' := sub_trans below grows
  cases rules F inTheory with
  | fact head =>
    exact (PositiveTheory.positive_reduct_exact head largerSubset).mpr
      ((PositiveTheory.positive_reduct_exact head below).mp old)
  | rule body head =>
    apply (RuleFactorization.reduct_imp _ _ _ _).mpr
    refine ⟨original _ inTheory, ?_⟩
    intro active
    have activeBefore := signed_body_reduct_antitone body below grows active
    have headBefore := ((RuleFactorization.reduct_imp _ _ _ _).mp old).2 activeBefore
    exact (PositiveTheory.positive_reduct_exact head largerSubset).mpr
      ((PositiveTheory.positive_reduct_exact head below).mp headBefore)

/-- Absence of falsum from the entire syntax tree. In particular no subformula
has the explicit default-negation shape `F → ⊥`. -/
def FalsumFree : Formula α → Prop
  | .bot => False
  | .atom _ => True
  | .conj F G | .disj F G | .imp F G => FalsumFree F ∧ FalsumFree G

/-- Restricting only explicit negation is insufficient. For roots
`b ∨ (b → z)` and `b → a`, the candidate {a} admits the empty witness,
but its superset {a,b} is stable. At {a,b} the first root requires b and
the second then requires a. Neither root contains falsum. -/
theorem falsum_free_countermodel_does_not_refute_superset :
    ∃ (T : Theory (Fin 3)) (N M M' : Atoms (Fin 3)),
      (∀ F ∈ T, FalsumFree F) ∧ Models M T ∧ ReductRegions.Countermodel M T N ∧
        ProperSub M M' ∧ Stable M' T := by
  classical
  let a : Fin 3 := 0
  let b : Fin 3 := 1
  let z : Fin 3 := 2
  let first : Formula (Fin 3) := .disj (.atom b) (.imp (.atom b) (.atom z))
  let second : Formula (Fin 3) := .imp (.atom b) (.atom a)
  let T := [first, second]
  let N : Atoms (Fin 3) := fun _ => False
  let M : Atoms (Fin 3) := fun x => x = a
  let M' : Atoms (Fin 3) := fun x => x = a ∨ x = b
  have ab : a ≠ b := by decide
  have za : z ≠ a := by decide
  have zb : z ≠ b := by decide
  have members : ∀ F ∈ T, FalsumFree F := by simp [T, first, second, FalsumFree]
  have model : Models M T := by simp [Models, T, first, second, M, Satisfies, Ne.symm ab]
  have proper : ProperSub N M := ⟨fun _ impossible => impossible.elim, fun back => back a rfl⟩
  have witness : Models N (ReductTheory M T) := by
    simp [Models, ReductTheory, T, first, second, Reduct, Satisfies, N, M, Ne.symm ab, za]
  have strict : ProperSub M M' :=
    ⟨fun _ present => Or.inl present, fun back => ab (back b (Or.inr rfl)).symm⟩
  have original : Models M' T := by simp [Models, T, first, second, Satisfies, M']
  have stable : Stable M' T := by
    refine ⟨original, ?_⟩
    rintro ⟨J, smaller, satisfies⟩
    have requires : J b ∧ (J b → J a) := by
      simpa [Models, ReductTheory, T, first, second, Reduct, Satisfies, M', za, zb] using satisfies
    apply smaller.2
    intro x present
    rcases present with rfl | rfl
    · exact requires.2 requires.1
    · exact requires.1
  exact ⟨T, N, M, M', members, model, ⟨proper, witness⟩, strict, stable⟩

end Zetesis.LearnedRegions
