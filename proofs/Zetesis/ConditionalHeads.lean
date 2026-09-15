import Zetesis.ConditionalConsumers
import Zetesis.NegativeHeads

/-!
# Finite conditional disjuncts

A completed local instance H:C contributes (C → H) ∧ ¬¬C. The finite
head is their disjunction. Eligibility is frozen in the original candidate,
but the implication retains C and H in every reduct interpretation. In
particular an empty local family is false, unlike a universal body conditional.

The instance type is shared with universal conditionals; the connective and
quantifier are different. Complete binding coverage, source variable identities,
positive producer coverage, machine arithmetic, work and storage admission, and
the correspondence with the Rust compiler remain explicit external obligations.
-/

namespace Zetesis.ConditionalHeads

open Ferraris UniversalConditionals
universe u v
variable {A : Type u} {V : Type v}

def contribution (row : Instance A) : Formula A :=
  .conj (.imp row.condition row.consequent)
    (Ferraris.Neg (Ferraris.Neg row.condition))

def head (rows : List (Instance A)) : Formula A :=
  RuleFactorization.any (rows.map contribution)

/-- An original conditional instance requires both eligibility and its head. -/
theorem instance_original (M : Atoms A) (row : Instance A) :
    Satisfies M (contribution row) ↔
      Satisfies M row.condition ∧ Satisfies M row.consequent := by
  change ((Satisfies M row.condition → Satisfies M row.consequent) ∧
    Satisfies M (Ferraris.Neg (Ferraris.Neg row.condition))) ↔ _
  rw [double_neg_satisfies]
  exact ⟨fun holds => ⟨holds.2, holds.1 holds.2⟩,
    fun holds => ⟨fun _ => holds.2, holds.1⟩⟩

/-- Original eligibility cannot replace the condition inside the reduct
    implication. No subset premise on J is required. -/
theorem instance_frozen (M J : Atoms A) (row : Instance A) :
    Satisfies J (Reduct M (contribution row)) ↔
      (Satisfies M row.condition ∧ Satisfies M row.consequent) ∧
        (Satisfies J (Reduct M row.condition) →
          Satisfies J (Reduct M row.consequent)) := by
  rw [contribution, RuleFactorization.reduct_conj,
    RuleFactorization.reduct_imp, double_neg_formula_reduct]
  change (((Satisfies M row.condition → Satisfies M row.consequent) ∧ _) ∧ _) ↔ _
  exact ⟨fun holds => ⟨⟨holds.2, holds.1.1 holds.2⟩, holds.1.2⟩,
    fun holds => ⟨⟨fun _ => holds.1.2, holds.2⟩, holds.1.1⟩⟩

/-- A finite head requires at least one eligible, true local instance. -/
theorem head_original (M : Atoms A) (rows : List (Instance A)) :
    Satisfies M (head rows) ↔
      ∃ row ∈ rows, Satisfies M row.condition ∧ Satisfies M row.consequent := by
  rw [head, RuleFactorization.satisfies_any]
  constructor
  · rintro ⟨formula, member, holds⟩
    obtain ⟨row, inside, rfl⟩ := List.mem_map.mp member
    exact ⟨row, inside, (instance_original M row).mp holds⟩
  · rintro ⟨row, member, holds⟩
    exact ⟨contribution row, List.mem_map.mpr ⟨row, member, rfl⟩,
      (instance_original M row).mpr holds⟩

/-- The frozen disjunction retains a complete eligible witness with its
    own reduct implication; it cannot mix two different local bindings. -/
theorem head_frozen (M J : Atoms A) (rows : List (Instance A)) :
    Satisfies J (Reduct M (head rows)) ↔
      ∃ row ∈ rows,
        (Satisfies M row.condition ∧ Satisfies M row.consequent) ∧
          (Satisfies J (Reduct M row.condition) →
            Satisfies J (Reduct M row.consequent)) := by
  rw [head, RuleFactorization.reduct_any]
  constructor
  · rintro ⟨formula, member, holds⟩
    obtain ⟨row, inside, rfl⟩ := List.mem_map.mp member
    exact ⟨row, inside, (instance_frozen M J row).mp holds⟩
  · rintro ⟨row, member, holds⟩
    exact ⟨contribution row, List.mem_map.mpr ⟨row, member, rfl⟩,
      (instance_frozen M J row).mpr holds⟩

/-- Completed empty enumeration supplies no disjunct in either interpretation. -/
theorem empty_is_false (M J : Atoms A) :
    ¬ Satisfies M (head ([] : List (Instance A))) ∧
      ¬ Satisfies J (Reduct M (head ([] : List (Instance A)))) := by
  simp [head_original, head_frozen]

/-- A uniformly true condition can be erased; candidate truth alone is
    insufficient. The second premise includes every frozen interpretation. -/
theorem true_condition_equivalent (row : Instance A)
    (original : ∀ M, Satisfies M row.condition)
    (frozen : ∀ M J, Satisfies J (Reduct M row.condition)) :
    ChoiceIntervals.Equivalent (contribution row) row.consequent := by
  constructor
  · intro M
    rw [instance_original]
    exact ⟨And.right, fun holds => ⟨original M, holds⟩⟩
  · intro M J
    rw [instance_frozen]
    constructor
    · intro holds
      exact holds.2 (frozen M J)
    · intro holds
      have original_head : Satisfies M row.consequent :=
        NegativeHeads.reduct_truth_requires_original M J row.consequent holds
      exact ⟨⟨original M, original_head⟩, fun _ => holds⟩

/-- Reordering or repeating completed instances preserves both interpretations.
    Coverage here names whole condition/head pairs, not separate column sets. -/
theorem complete_family_equivalent (left right : List (Instance A))
    (coverage : ∀ row, row ∈ left ↔ row ∈ right) :
    ChoiceIntervals.Equivalent (head left) (head right) := by
  constructor
  · intro M
    simp only [head_original]
    constructor
    · rintro ⟨row, member, holds⟩
      exact ⟨row, (coverage row).mp member, holds⟩
    · rintro ⟨row, member, holds⟩
      exact ⟨row, (coverage row).mpr member, holds⟩
  · intro M J
    simp only [head_frozen]
    constructor
    · rintro ⟨row, member, holds⟩
      exact ⟨row, (coverage row).mp member, holds⟩
    · rintro ⟨row, member, holds⟩
      exact ⟨row, (coverage row).mpr member, holds⟩

/-- Each aggregate proposal selects its own completed local family while
    retaining the original equality and activation in the whole rule. -/
def assignedRule (equality activation : Formula A)
    (rows : List (Instance A)) : Formula A :=
  .imp (.conj equality activation) (head rows)

/-- A covering finite carrier keeps the actual value paired with its local
    head family. Enumeration success does not establish the equality. -/
theorem covered_assignment (carrier : List V) (keep : V → Bool)
    (rows : V → List (Instance A)) (actual : V)
    (covered : actual ∈ carrier) (accepted : keep actual = true) :
    (actual, rows actual) ∈ AggregateConsumers.substitutions carrier keep rows :=
  AggregateConsumers.covered_consumer carrier keep rows actual covered accepted

/-- An unrealized aggregate proposal cannot activate its conditional head,
    in the original formula or in any frozen interpretation. -/
theorem unrealized_assignment (M J : Atoms A) (equality activation : Formula A)
    (rows : List (Instance A)) (unrealized : ¬ Satisfies M equality) :
    Satisfies M (assignedRule equality activation rows) ∧
      Satisfies J (Reduct M (assignedRule equality activation rows)) := by
  have inactive : ¬ Satisfies M (.conj equality activation) :=
    fun holds => unrealized holds.1
  constructor
  · exact fun holds => False.elim (inactive holds)
  · rw [assignedRule, RuleFactorization.reduct_imp]
    have frozen_inactive : ¬ Satisfies J (Reduct M (.conj equality activation)) := by
      rw [RuleFactorization.false_reduct M _ inactive]
      exact id
    exact ⟨fun holds => False.elim (inactive holds),
      fun holds => False.elim (frozen_inactive holds)⟩

end Zetesis.ConditionalHeads
