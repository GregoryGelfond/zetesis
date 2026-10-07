import Zetesis.NormalFerraris

/-!
# Rules blocked by retained facts

A finite rule body containing `not p` is false whenever the interpretation
contains `p`. If the surrounding theory retains the fact `p`, the rule can be
omitted without changing original satisfaction or any frozen-reduct model.
The frozen law quantifies over arbitrary outer and tested interpretations:
neither subset containment nor candidate modelhood is assumed.

Bodies use the existing finite conjunction. Their other conjuncts and the head
may be arbitrary Ferraris formulas, and the remaining theory may contain
recursion or disjunction. No least-consequence argument is used. A retained
unconditional fact suffices; a complete fact-only predicate classifier is a
stronger implementation precondition, not a premise of this semantic law.

These laws do not establish source classification, tuple lookup, source/IR
correspondence, diagnostics for skipped arithmetic or constructors, atom-carrier
mapping, resource accounting or Rust refinement. In particular, pruning possible
support can hide a downstream source error unless its admission is separately
preserved. A fact hit is not a closed-world assertion about a missing tuple.
-/

namespace Zetesis.FactRejection

universe u
variable {α : Type u}
open Ferraris NormalFerraris

/-- A rule with a default-negative fact in its body holds in every interpretation
containing that fact, regardless of its other conjuncts and its head. -/
theorem blocked_original (M : Atoms α) (atom : α)
    (body : List (Formula α)) (head : Formula α)
    (negative : Neg (.atom atom) ∈ body) (fact : M atom) :
    Satisfies M (.imp (conjunction body) head) := by
  intro bodyHolds
  have negationHolds : Satisfies M (Neg (.atom atom)) := by
    exact (satisfies_conjunction M body).mp bodyHolds _ negative
  have contradiction : False := by
    exact negationHolds fact
  exact False.elim contradiction

/-- The blocked rule's frozen reduct holds in every tested interpretation.
The outer interpretation contains the fact; the tested interpretation is
otherwise arbitrary and need not be contained in the outer interpretation. -/
theorem blocked_reduct (M J : Atoms α) (atom : α)
    (body : List (Formula α)) (head : Formula α)
    (negative : Neg (.atom atom) ∈ body) (fact : M atom) :
    Satisfies J (Reduct M (.imp (conjunction body) head)) := by
  have original : Satisfies M (.imp (conjunction body) head) := by
    exact blocked_original M atom body head negative fact
  have implication : Satisfies J (Reduct M (conjunction body)) →
      Satisfies J (Reduct M head) := by
    intro bodyHolds
    have negationHolds : Satisfies J (Reduct M (Neg (.atom atom))) := by
      exact (reduct_conjunction M J body).mp bodyHolds _ negative
    have factAbsent : ¬ M atom := by
      exact (neg_atom_reduct M J atom).mp negationHolds
    exact False.elim (factAbsent fact)
  exact (RuleFactorization.reduct_imp M J (conjunction body) head).mpr
    ⟨original, implication⟩

/-- Keeping the fact in the surrounding theory makes omission of its blocked
rule preserve and reflect original satisfaction, for every interpretation. -/
theorem original_omit (M : Atoms α) (theory : Theory α) (atom : α)
    (body : List (Formula α)) (head : Formula α)
    (retained : Formula.atom atom ∈ theory)
    (negative : Neg (.atom atom) ∈ body) :
    Models M (.imp (conjunction body) head :: theory) ↔ Models M theory := by
  rw [models_cons]
  constructor
  · exact And.right
  · intro remaining
    have fact : M atom := by
      exact remaining (.atom atom) retained
    have blocked : Satisfies M (.imp (conjunction body) head) := by
      exact blocked_original M atom body head negative fact
    exact ⟨blocked, remaining⟩

/-- Omission also preserves and reflects models of the entire frozen reduct
for arbitrary M and J. A model of the retained fact's reduct itself establishes
that M contains the fact; the theorem assumes no candidate modelhood. -/
theorem reduct_omit (M J : Atoms α) (theory : Theory α) (atom : α)
    (body : List (Formula α)) (head : Formula α)
    (retained : Formula.atom atom ∈ theory)
    (negative : Neg (.atom atom) ∈ body) :
    Models J (ReductTheory M (.imp (conjunction body) head :: theory)) ↔
      Models J (ReductTheory M theory) := by
  change Models J (Reduct M (.imp (conjunction body) head) ::
    ReductTheory M theory) ↔ Models J (ReductTheory M theory)
  rw [models_cons]
  constructor
  · exact And.right
  · intro remaining
    have frozenFact : Satisfies J (Reduct M (.atom atom)) := by
      exact remaining _ (List.mem_map.mpr ⟨.atom atom, retained, rfl⟩)
    have fact : M atom := by
      exact ((atom_reduct M J atom).mp frozenFact).1
    have blocked : Satisfies J (Reduct M (.imp (conjunction body) head)) := by
      exact blocked_reduct M J atom body head negative fact
    exact ⟨blocked, remaining⟩

/-- The two satisfaction correspondences give exactly the same answer sets.
The surrounding theory may be arbitrary; no positive-normal class is assumed. -/
theorem stable_omit (M : Atoms α) (theory : Theory α) (atom : α)
    (body : List (Formula α)) (head : Formula α)
    (retained : Formula.atom atom ∈ theory)
    (negative : Neg (.atom atom) ∈ body) :
    Stable M (.imp (conjunction body) head :: theory) ↔ Stable M theory := by
  have original : Models M (.imp (conjunction body) head :: theory) ↔
      Models M theory := by
    exact original_omit M theory atom body head retained negative
  have countermodels :
      (∃ J, ProperSub J M ∧
        Models J (ReductTheory M (.imp (conjunction body) head :: theory))) ↔
      (∃ J, ProperSub J M ∧ Models J (ReductTheory M theory)) := by
    constructor
    · rintro ⟨J, proper, model⟩
      have remaining : Models J (ReductTheory M theory) := by
        exact (reduct_omit M J theory atom body head retained negative).mp model
      exact ⟨J, proper, remaining⟩
    · rintro ⟨J, proper, model⟩
      have extended :
          Models J (ReductTheory M (.imp (conjunction body) head :: theory)) := by
        exact (reduct_omit M J theory atom body head retained negative).mpr model
      exact ⟨J, proper, extended⟩
  exact and_congr original (not_congr countermodels)

end Zetesis.FactRejection
