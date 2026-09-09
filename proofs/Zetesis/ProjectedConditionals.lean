import Zetesis.ConsequentAlternatives
import Zetesis.OuterNegativeConsumers

/-!
# Signed existential projections in finite conditionals

There are three distinct finite carriers: condition assignments, source
alternatives for each assignment, and anonymous witnesses for each alternative.
Witness formulas first form an existential disjunction. Default negation applies
to that whole projection; the signed source alternatives are then disjoined.
Every condition remains the antecedent of its own implication.

An empty witness family projects to false, so its negation is true and its double
negation false. An empty source-alternative family instead has no signed formula
at all. An empty completed condition carrier is vacuous. These distinctions
cannot be inferred from an unfinished source enumeration.

The complete-carrier law states the bridge to an intended matching predicate
explicitly. The other laws quantify over supplied completed formula families;
they do not establish source matching, safety, bindings, anonymous scope, support
enumeration, resource completion or the Rust compiler's correspondence. A signed
projection tests candidate truth and does not introduce positive atom support.
-/

namespace Zetesis.ProjectedConditionals

universe u v
variable {A : Type u} {W : Type v}
open Ferraris
open RuleFactorization (any)

/-- One condition assignment and the complete witness family of each source
    alternative. Equal witnesses may occur; omission is not duplication. -/
structure Row (A : Type u) where
  condition : Formula A
  alternatives : List (List (Formula A))

/-- Project anonymous witnesses before applying the supplied formula polarity.
    The general constructor does not assume that this polarity freezes truth. -/
def body (polarity : Formula A → Formula A) (rows : List (Row A)) : Formula A :=
  ConsequentAlternatives.body (rows.map (fun row =>
    ⟨row.condition, row.alternatives.map (fun witnesses => polarity (any witnesses))⟩))

/-- Exact finite coverage connects a supplied projection with its intended
    matching domain. Both missing and spurious witnesses are excluded by the
    premise; the domain need not have a global finite enumeration. -/
theorem complete_projection (M : Atoms A) (witnesses : List W)
    (matching : W → Prop) (formula : W → Formula A)
    (coverage : ∀ witness, witness ∈ witnesses ↔ matching witness) :
    Satisfies M (any (witnesses.map formula)) ↔
      ∃ witness, matching witness ∧ Satisfies M (formula witness) := by
  simp only [RuleFactorization.satisfies_any, List.mem_map]
  constructor
  · rintro ⟨_, ⟨witness, member, rfl⟩, truth⟩
    exact ⟨witness, (coverage witness).mp member, truth⟩
  · rintro ⟨witness, matched, truth⟩
    exact ⟨formula witness, ⟨witness, (coverage witness).mpr matched, rfl⟩, truth⟩

/-- Conditions quantify universally, source alternatives existentially, and
    polarity is applied only after each anonymous projection is complete. -/
theorem original_semantics (M : Atoms A) (polarity : Formula A → Formula A)
    (rows : List (Row A)) :
    Satisfies M (body polarity rows) ↔
      ∀ row ∈ rows, Satisfies M row.condition →
        ∃ witnesses ∈ row.alternatives, Satisfies M (polarity (any witnesses)) := by
  rw [body, ConsequentAlternatives.original_semantics]
  constructor
  · intro original row member active
    obtain ⟨alternative, present, truth⟩ :=
      original _ (List.mem_map.mpr ⟨row, member, rfl⟩) active
    obtain ⟨witnesses, inside, rfl⟩ := List.mem_map.mp present
    exact ⟨witnesses, inside, truth⟩
  · intro original row member active
    obtain ⟨source, inside, rfl⟩ := List.mem_map.mp member
    obtain ⟨witnesses, present, truth⟩ := original source inside active
    exact ⟨polarity (any witnesses), List.mem_map.mpr ⟨witnesses, present, rfl⟩, truth⟩

/-- Candidate-only polarity makes the retained conditional candidate-only too.
    The law covers arbitrary M and J and is deliberately conditional on the
    polarity premise; it does not apply to an unsigned projection. -/
theorem frozen_semantics (M J : Atoms A) (polarity : Formula A → Formula A)
    (rows : List (Row A))
    (freezes : ∀ formula, Satisfies J (Reduct M (polarity formula)) ↔
      Satisfies M (polarity formula)) :
    Satisfies J (Reduct M (body polarity rows)) ↔ Satisfies M (body polarity rows) := by
  apply ConsequentAlternatives.candidate_consequents
  intro row member alternative present
  obtain ⟨source, _, rfl⟩ := List.mem_map.mp member
  obtain ⟨witnesses, _, rfl⟩ := List.mem_map.mp present
  exact freezes (any witnesses)

/-- A negative projected consequent needs an alternative with no true witness
    in M. Witnesses absent from J but present in M still block that alternative. -/
theorem negative_frozen (M J : Atoms A) (rows : List (Row A)) :
    Satisfies J (Reduct M (body Neg rows)) ↔
      ∀ row ∈ rows, Satisfies M row.condition →
        ∃ witnesses ∈ row.alternatives, ∀ formula ∈ witnesses, ¬ Satisfies M formula := by
  have freezes (formula : Formula A) :
      Satisfies J (Reduct M (Neg formula)) ↔ Satisfies M (Neg formula) :=
    ChoiceIntervals.negation_frozen M J formula
  rw [frozen_semantics M J Neg rows freezes, original_semantics]
  simp only [← freezes, OuterNegativeConsumers.negative_projection_frozen]

/-- A double-negative consequent needs an original witness in at least one
    source alternative. Its reduct neither requires that witness in J nor gives
    it positive support. -/
theorem double_negative_frozen (M J : Atoms A) (rows : List (Row A)) :
    Satisfies J (Reduct M (body (fun formula => Neg (Neg formula)) rows)) ↔
      ∀ row ∈ rows, Satisfies M row.condition →
        ∃ witnesses ∈ row.alternatives, ∃ formula ∈ witnesses, Satisfies M formula := by
  have freezes (formula : Formula A) :
      Satisfies J (Reduct M (Neg (Neg formula))) ↔ Satisfies M (Neg (Neg formula)) :=
    (double_neg_formula_reduct M J formula).trans (double_neg_satisfies M formula).symm
  rw [frozen_semantics M J _ rows freezes, original_semantics]
  simp only [double_neg_satisfies, RuleFactorization.satisfies_any]

/-- One empty projected alternative is true under negation; no source
    alternative is false. Confusing these empty carriers changes acceptance. -/
theorem empty_carriers_differ (M J : Atoms A) :
    Satisfies J (Reduct M (body Neg [⟨ChoiceIntervals.top, [[]]⟩])) ∧
      ¬ Satisfies J (Reduct M (body Neg [⟨ChoiceIntervals.top, []⟩])) := by
  simp [negative_frozen, ChoiceIntervals.top, Satisfies]

/-- A prefix without the true witness cannot justify negative acceptance.
    Complete coverage, rather than positive-support possibility, is the premise
    needed before interpreting projection absence as logical negation. -/
theorem omitted_witness_changes_negation :
    let M : Atoms Unit := fun _ => True
    Satisfies M (Neg (any [])) ∧ ¬ Satisfies M (Neg (any [.atom ()])) := by
  simp [Ferraris.Neg, any, Satisfies]

end Zetesis.ProjectedConditionals
