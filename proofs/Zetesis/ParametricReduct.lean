import Zetesis.Ferraris

/-!
# Candidate-parametric reduct evaluation

A fixed connective network can evaluate every frozen reduct. Its atom inputs
come from the prospective smaller interpretation. Only implication nodes need
an explicit parameter recording their truth in the original candidate:
conjunction and disjunction already collapse when their required children do.

The laws below relate that network to the existing Ferraris reduct under two
premises: the smaller interpretation is a subset of the candidate, and the
parameters record actual original truth. They do not justify using the original
formula's classical equivalences as reduct equivalences. They also do not prove
Rust DAG/CNF encoding, parameter authentication, SAT search, allocation or work
accounting. Those are separate representation and execution obligations.
-/

namespace Zetesis.ParametricReduct

universe u

variable {α : Type u}

/-- Truth of a connective network with original-truth parameters. Implication
    keeps its original truth as a guard; atoms read the smaller interpretation.
    The parameter function is abstract data until an agreement premise is given. -/
def Satisfies (original : Ferraris.Formula α → Prop) (N : Atoms α) :
    Ferraris.Formula α → Prop
  | .atom a => N a
  | .bot => False
  | .conj F G => Satisfies original N F ∧ Satisfies original N G
  | .disj F G => Satisfies original N F ∨ Satisfies original N G
  | .imp F G => original (.imp F G) ∧
      (Satisfies original N F → Satisfies original N G)

/-- A true parametric value is true in the original candidate. The atom case
    uses subset containment; conjunction/disjunction use their children; an
    implication's explicit original-truth guard supplies the final case.
    Consequently every formula false in the candidate is false in this network. -/
theorem satisfies_original (original : Ferraris.Formula α → Prop)
    (M N : Atoms α) (subset : Sub N M)
    (agree : ∀ F, original F ↔ Ferraris.Satisfies M F)
    (F : Ferraris.Formula α) :
    Satisfies original N F → Ferraris.Satisfies M F := by
  induction F with
  | atom a => exact subset a
  | bot => exact fun impossible => impossible
  | conj F G left right =>
    intro truth
    exact ⟨left truth.1, right truth.2⟩
  | disj F G left right =>
    intro truth
    rcases truth with first | second
    · exact Or.inl (left first)
    · exact Or.inr (right second)
  | imp F G _left _right =>
    intro truth
    exact (agree (.imp F G)).mp truth.1

/-- The parametric network agrees with the frozen reduct on every subset.

The proof follows the original syntax. Atoms use subset containment. For a
candidate-true conjunction/disjunction, the induction hypotheses supply the
same children. When the original compound is false, `satisfies_original`
refutes its parametric value as well. Implication explicitly retains the
original-truth guard, including the candidate-false case.
-/
theorem satisfies_reduct (original : Ferraris.Formula α → Prop)
    (M N : Atoms α) (subset : Sub N M)
    (agree : ∀ F, original F ↔ Ferraris.Satisfies M F)
    (F : Ferraris.Formula α) :
    Satisfies original N F ↔ Ferraris.Satisfies N (Ferraris.Reduct M F) := by
  classical
  induction F with
  | atom a =>
    by_cases member : M a
    · simp only [Satisfies, Ferraris.Reduct, if_pos member, Ferraris.Satisfies]
    · have absent : ¬ N a := fun present => member (subset a present)
      simp only [Satisfies, Ferraris.Reduct, if_neg member, Ferraris.Satisfies]
      exact iff_false_intro absent
  | bot => rfl
  | conj F G left right =>
    rw [Ferraris.Reduct]
    by_cases truth : Ferraris.Satisfies M (.conj F G)
    · rw [if_pos truth]
      exact and_congr left right
    · rw [if_neg truth]
      have absent : ¬ Satisfies original N (.conj F G) := by
        intro present
        exact truth (satisfies_original original M N subset agree (.conj F G) present)
      exact iff_false_intro absent
  | disj F G left right =>
    rw [Ferraris.Reduct]
    by_cases truth : Ferraris.Satisfies M (.disj F G)
    · rw [if_pos truth]
      exact or_congr left right
    · rw [if_neg truth]
      have absent : ¬ Satisfies original N (.disj F G) := by
        intro present
        exact truth (satisfies_original original M N subset agree (.disj F G) present)
      exact iff_false_intro absent
  | imp F G left right =>
    rw [Ferraris.Reduct]
    by_cases truth : Ferraris.Satisfies M (.imp F G)
    · rw [if_pos truth]
      have parameter : original (.imp F G) := (agree (.imp F G)).mpr truth
      constructor
      · intro network antecedent
        exact right.mp (network.2 (left.mpr antecedent))
      · intro reduced
        exact ⟨parameter, fun antecedent => right.mpr (reduced (left.mp antecedent))⟩
    · rw [if_neg truth]
      have absent : ¬ Satisfies original N (.imp F G) := by
        intro present
        exact truth (satisfies_original original M N subset agree (.imp F G) present)
      exact iff_false_intro absent

/-- Under authenticated original truth, existence of a proper-subset model of
    all parametric roots is exactly existence of a proper-subset reduct model.
    Original candidate satisfaction remains a separate membership obligation. -/
theorem countermodel_iff (original : Ferraris.Formula α → Prop)
    (M : Atoms α) (T : Ferraris.Theory α)
    (agree : ∀ F, original F ↔ Ferraris.Satisfies M F) :
    (∃ N, Ferraris.ProperSub N M ∧ ∀ F, F ∈ T → Satisfies original N F) ↔
    (∃ N, Ferraris.ProperSub N M ∧ Ferraris.Models N (Ferraris.ReductTheory M T)) := by
  constructor
  · rintro ⟨N, proper, roots⟩
    have reduct : Ferraris.Models N (Ferraris.ReductTheory M T) := by
      intro reduced member
      obtain ⟨F, source, rfl⟩ := List.mem_map.mp member
      exact (satisfies_reduct original M N proper.1 agree F).mp (roots F source)
    exact ⟨N, proper, reduct⟩
  · rintro ⟨N, proper, reduct⟩
    have roots : ∀ F, F ∈ T → Satisfies original N F := by
      intro F member
      have reduced : Ferraris.Satisfies N (Ferraris.Reduct M F) :=
        reduct (Ferraris.Reduct M F) (List.mem_map.mpr ⟨F, member, rfl⟩)
      exact (satisfies_reduct original M N proper.1 agree F).mpr reduced
    exact ⟨N, proper, roots⟩

end Zetesis.ParametricReduct
