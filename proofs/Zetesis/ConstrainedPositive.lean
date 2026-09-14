import Zetesis.PositiveTheory

/-!
# Positive producers with arbitrary constraints

An original constraint is F → falsum, with no restriction on F. If a candidate
satisfies it, F is false there, so the entire constraint reduct is true in every
tested interpretation. Constraints can filter answer sets but supply no support.

The general append law applies to any producer theory. Its positive specialization
uses PositiveTheory's existing least-consequence characterization. A failed
nonmonotone constraint at least consequences rules out answer sets, but need not
rule out larger classical models. Rust root partitioning, original evaluation,
queue execution and resource accounting remain separate obligations.
-/

namespace Zetesis.ConstrainedPositive

open Ferraris

universe u
variable {A : Type u}

/-- A formula false in the frozen candidate has falsum as its entire reduct.
Each syntax case follows the outer test in the reduct definition. -/
theorem reduct_of_false (M : Atoms A) (F : Formula A)
    (absent : ¬ Satisfies M F) : Reduct M F = .bot := by
  classical
  cases F with
  | atom a =>
    change ¬ M a at absent
    simp only [Reduct, if_neg absent]
  | bot => rfl
  | conj F G => simp only [Reduct, if_neg absent]
  | disj F G => simp only [Reduct, if_neg absent]
  | imp F G => simp only [Reduct, if_neg absent]

/-- A satisfied original constraint has a true reduct in every interpretation.
No subset premise or monotonicity premise is needed: its antecedent was false
in the candidate and therefore freezes to falsum. -/
theorem constraint_frozen (M N : Atoms A) (F : Formula A)
    (original : Satisfies M (Ferraris.Neg F)) : Satisfies N (Reduct M (Ferraris.Neg F)) := by
  classical
  have absent : ¬ Satisfies M F := original
  change Satisfies M (.imp F .bot) at original
  change Satisfies N (Reduct M (.imp F .bot))
  rw [Reduct, if_pos original, reduct_of_false M F absent]
  exact fun impossible => impossible

/-- Every original constraint satisfied by M is harmless to reduct satisfaction
in N, including when N is not a subset of M. The list retains every occurrence. -/
theorem constraints_frozen (M N : Atoms A) (bodies : List (Formula A))
    (original : Models M (bodies.map Ferraris.Neg)) :
    Models N (ReductTheory M (bodies.map Ferraris.Neg)) := by
  intro reduced member
  obtain ⟨constraint, present, rfl⟩ := List.mem_map.mp member
  obtain ⟨body, source, rfl⟩ := List.mem_map.mp present
  exact constraint_frozen M N body
    (original (Ferraris.Neg body) (List.mem_map.mpr ⟨body, source, rfl⟩))

/-- Adding arbitrary constraints retains exactly the original answer sets that
satisfy them. For the forward direction, every proper-subset reduct model of
the original theory also satisfies the constraint reducts, contradicting full
minimality. The reverse direction is the existing satisfied-extension law. -/
theorem stable_append_constraints (P : Theory A) (bodies : List (Formula A))
    (M : Atoms A) :
    Stable M (P ++ bodies.map Ferraris.Neg) ↔ Stable M P ∧ Models M (bodies.map Ferraris.Neg) := by
  constructor
  · intro full
    have original : Models M P :=
      fun F member => full.1 F (List.mem_append_left _ member)
    have constraints : Models M (bodies.map Ferraris.Neg) :=
      fun F member => full.1 F (List.mem_append_right _ member)
    have stable : Stable M P := by
      refine ⟨original, ?_⟩
      rintro ⟨N, proper, reduced⟩
      have constraintReduct : Models N (ReductTheory M (bodies.map Ferraris.Neg)) :=
        constraints_frozen M N bodies constraints
      have combined : Models N (ReductTheory M (P ++ bodies.map Ferraris.Neg)) := by
        intro formula member
        obtain ⟨root, source, rfl⟩ := List.mem_map.mp member
        rcases List.mem_append.mp source with producer | constraint
        · exact reduced (Reduct M root) (List.mem_map.mpr ⟨root, producer, rfl⟩)
        · exact constraintReduct (Reduct M root)
            (List.mem_map.mpr ⟨root, constraint, rfl⟩)
      exact full.2 ⟨N, proper, combined⟩
    exact ⟨stable, constraints⟩
  · rintro ⟨stable, constraints⟩
    exact stable_append_of_models M P (bodies.map Ferraris.Neg) stable constraints

/-- A positive atomic-head theory plus arbitrary constraints has an answer set
exactly when its least consequences satisfy both its positive constraints and
the additional arbitrary constraints. The answer is exactly that least set.
When P contains only facts and atomic-head producers, its positive-constraint
condition is vacuous; no arbitrary constraint participates in least closure. -/
theorem positive_stable_iff (P : Theory A) (bodies : List (Formula A))
    (admitted : ∀ F ∈ P, PositiveTheory.Admitted F) (M : Atoms A) :
    Stable M (P ++ bodies.map Ferraris.Neg) ↔
      M = PositiveTheory.Least P ∧
      PositiveTheory.Constraints (PositiveTheory.Least P) P ∧
      Models (PositiveTheory.Least P) (bodies.map Ferraris.Neg) := by
  rw [stable_append_constraints, PositiveTheory.stable_iff_least_constraints P admitted M]
  constructor
  · rintro ⟨⟨equality, positiveConstraints⟩, constraints⟩
    subst M
    exact ⟨rfl, positiveConstraints, constraints⟩
  · rintro ⟨equality, positiveConstraints, constraints⟩
    subst M
    exact ⟨⟨rfl, positiveConstraints⟩, constraints⟩

/-- A failed arbitrary constraint at least consequences rules out every answer
set. Unlike PositiveTheory.failed_constraint_no_model, this law makes no claim
that the original theory lacks larger classical models. -/
theorem failed_constraints_no_stable (P : Theory A) (bodies : List (Formula A))
    (admitted : ∀ F ∈ P, PositiveTheory.Admitted F)
    (failed : ¬ Models (PositiveTheory.Least P) (bodies.map Ferraris.Neg)) :
    ¬ ∃ M, Stable M (P ++ bodies.map Ferraris.Neg) := by
  rintro ⟨M, stable⟩
  exact failed ((positive_stable_iff P bodies admitted M).mp stable).2.2

end Zetesis.ConstrainedPositive
