import Zetesis.TightPlans

/-!
# Necessary support for disjunctive and atomic-choice producers

An answer-set atom needs ordinary sole-head support or enabled atomic-choice
support. Otherwise deleting that atom leaves a
proper-subset model of the frozen reduct. This is a necessary candidate
restriction, not a sufficient membership test or a shift of the original theory.

Bodies may be arbitrary formulas. Asserted heads are positive disjunctions or
atomic choices; asserted default negations are frozen constraints. An enabled
choice supplies a support witness for its chosen atom. Repeated ordinary head
occurrences do not denote different atoms. The laws do not establish Rust DAG
extraction, Boolean encoding, allocation or resource accounting.
-/
namespace Zetesis.DisjunctiveSupport

universe u
variable {α : Type u}
open Ferraris

/-- Positive disjunctive heads, including falsum and repeated occurrences. -/
inductive Head (α : Type u) where
  | atom : α → Head α
  | bot : Head α
  | disj : Head α → Head α → Head α

/-- The original head formula; no exclusive-choice rewriting occurs. -/
def Head.formula : Head α → Formula α
  | .atom a => .atom a
  | .bot => .bot
  | .disj H K => .disj H.formula K.formula

/-- Semantic atom occurrence ignores multiplicity. -/
def Head.Contains : Head α → α → Prop
  | .atom b, a => a = b
  | .bot, _ => False
  | .disj H K, a => H.Contains a ∨ K.Contains a

/-- A producer retains whether its original root had an explicit body. -/
inductive Producer (α : Type u) where
  | fact : Head α → Producer α
  | rule : Formula α → Head α → Producer α

def Producer.head : Producer α → Head α
  | .fact H | .rule _ H => H

def Producer.Enabled (M : Atoms α) : Producer α → Prop
  | .fact _ => True
  | .rule B _ => Satisfies M B

def Producer.formula : Producer α → Formula α
  | .fact H => H.formula
  | .rule B H => .imp B H.formula

/-- This applicable rule has no other true distinct head in the candidate. -/
def Producer.Supports (M : Atoms α) (r : Producer α) (a : α) : Prop :=
  r.Enabled M ∧ r.head.Contains a ∧
    ∀ b, r.head.Contains b → M b → b = a

/-- Every original root is covered. A partial producer inventory is insufficient. -/
def Covered (T : Theory α) (rules : List (Producer α)) : Prop :=
  ∀ F ∈ T, (∃ r ∈ rules, r.formula = F) ∨
    (∃ G, F = Ferraris.Neg G) ∨ F = .bot

/-- A positive head is true precisely when one of its occurring atoms is true. -/
theorem head_satisfies (M : Atoms α) (H : Head α) :
    Satisfies M H.formula ↔ ∃ a, H.Contains a ∧ M a := by
  induction H with
  | atom a => simp [Head.formula, Head.Contains, Satisfies]
  | bot => simp [Head.formula, Head.Contains, Satisfies]
  | disj H K ihH ihK =>
    simp only [Head.formula, Satisfies, ihH, ihK, Head.Contains]
    constructor
    · rintro (⟨a, ha, present⟩ | ⟨a, ha, present⟩)
      · exact ⟨a, Or.inl ha, present⟩
      · exact ⟨a, Or.inr ha, present⟩
    · rintro ⟨a, ha | ha, present⟩
      · exact Or.inl ⟨a, ha, present⟩
      · exact Or.inr ⟨a, ha, present⟩

/-- The frozen positive head needs an occurring atom true in both worlds. -/
theorem head_reduct (M J : Atoms α) (H : Head α) :
    Satisfies J (Reduct M H.formula) ↔
      ∃ a, H.Contains a ∧ M a ∧ J a := by
  induction H with
  | atom a => simpa [Head.formula, Head.Contains] using atom_reduct M J a
  | bot => simp [Head.formula, Head.Contains, Reduct, Satisfies]
  | disj H K ihH ihK =>
    rw [Head.formula, RuleFactorization.reduct_disj, ihH, ihK]
    constructor
    · rintro (⟨a, ha, present⟩ | ⟨a, ha, present⟩)
      · exact ⟨a, Or.inl ha, present⟩
      · exact ⟨a, Or.inr ha, present⟩
    · rintro ⟨a, ha | ha, present⟩
      · exact Or.inl ⟨a, ha, present⟩
      · exact Or.inr ⟨a, ha, present⟩

/-- Delete one semantic atom; all other memberships remain unchanged. -/
def remove (M : Atoms α) (a : α) : Atoms α := fun b => M b ∧ b ≠ a

/-- Without support for `a`, an applicable producer retains another true head.
If every true head were `a`, original satisfaction would instead supply the
missing support witness. -/
theorem other_head (M : Atoms α) (r : Producer α) (a : α)
    (model : Satisfies M r.formula) (enabled : r.Enabled M)
    (unsupported : ¬ r.Supports M a) :
    ∃ b, r.head.Contains b ∧ M b ∧ b ≠ a := by
  classical
  have trueHead : Satisfies M r.head.formula := by
    cases r with
    | fact H => exact model
    | rule B H => exact model enabled
  obtain ⟨b, inHead, present⟩ := (head_satisfies M r.head).mp trueHead
  by_cases same : b = a
  · by_cases other : ∃ c, r.head.Contains c ∧ M c ∧ c ≠ a
    · exact other
    · have support : r.Supports M a := by
        refine ⟨enabled, same ▸ inHead, ?_⟩
        intro c occurs member
        exact Classical.byContradiction (fun different => other ⟨c, occurs, member, different⟩)
      exact False.elim (unsupported support)
  · exact ⟨b, inHead, present, same⟩

/-- Deleting an unsupported atom preserves each producer's frozen reduct.
For a disabled rule the frozen body is falsum. Otherwise another true head
survives, making the reduct implication true independently of its body. -/
theorem producer_removal (M : Atoms α) (r : Producer α) (a : α)
    (model : Satisfies M r.formula) (unsupported : ¬ r.Supports M a) :
    Satisfies (remove M a) (Reduct M r.formula) := by
  classical
  cases r with
  | fact H =>
    obtain ⟨b, occurs, present, different⟩ := other_head M (.fact H) a model trivial unsupported
    exact (head_reduct M (remove M a) H).mpr ⟨b, occurs, present, present, different⟩
  | rule B H =>
    have rootTrue : Satisfies M (.imp B H.formula) := model
    change Satisfies (remove M a) (Reduct M (.imp B H.formula))
    rw [Reduct, if_pos rootTrue]
    intro body
    have enabled : Satisfies M B := TightPlans.original_of_reduct M (remove M a) B body
    obtain ⟨b, occurs, present, different⟩ := other_head M (.rule B H) a model enabled unsupported
    exact (head_reduct M (remove M a) H).mpr ⟨b, occurs, present, present, different⟩

/-- Every answer-set atom has an applicable sole-head support witness.

Assume no witness exists. Removing the atom preserves every producer reduct by
the preceding law and every asserted negation because it is frozen. Coverage
then gives a proper-subset model of the complete reduct, contradicting stability.
-/
theorem answer_set_supported (M : Atoms α) (T : Theory α)
    (rules : List (Producer α)) (covered : Covered T rules)
    (original : ∀ r ∈ rules, r.formula ∈ T) (answer : Stable M T) :
    ∀ a, M a → ∃ r ∈ rules, r.Supports M a := by
  classical
  intro a present
  apply Classical.byContradiction
  intro unsupported
  have reductModel : Models (remove M a) (ReductTheory M T) := by
    intro reduced member
    obtain ⟨F, asserted, rfl⟩ := List.mem_map.mp member
    rcases covered F asserted with ⟨r, inRules, rfl⟩ | ⟨G, rfl⟩ | rfl
    · apply producer_removal M r a (answer.1 r.formula (original r inRules))
      exact fun support => unsupported ⟨r, inRules, support⟩
    · apply (TightPlans.negation_frozen M (remove M a) G).mpr
      exact answer.1 (Ferraris.Neg G) asserted
    · exact False.elim (answer.1 .bot asserted)
  have proper : ProperSub (remove M a) M := by
    refine ⟨fun _ member => member.1, ?_⟩
    intro reverse
    exact (reverse a present).2 rfl
  exact answer.2 ⟨remove M a, proper, reductModel⟩

/-- Atomic choices retain their original body and the order of their two
branches. Unlike ranked producers, their bodies may be arbitrary formulas. -/
inductive ChoiceProducer (α : Type u) where
  | fact : α → ChoiceProducer α
  | rule : Formula α → α → ChoiceProducer α
  | reversedFact : α → ChoiceProducer α
  | reversedRule : Formula α → α → ChoiceProducer α

def ChoiceProducer.head : ChoiceProducer α → α
  | .fact a | .rule _ a | .reversedFact a | .reversedRule _ a => a

def ChoiceProducer.Enabled (M : Atoms α) : ChoiceProducer α → Prop
  | .fact _ | .reversedFact _ => True
  | .rule B _ | .reversedRule B _ => Satisfies M B

def ChoiceProducer.formula : ChoiceProducer α → Formula α
  | .fact a => .disj (.atom a) (Ferraris.Neg (.atom a))
  | .rule B a => .imp B (.disj (.atom a) (Ferraris.Neg (.atom a)))
  | .reversedFact a => .disj (Ferraris.Neg (.atom a)) (.atom a)
  | .reversedRule B a => .imp B (.disj (Ferraris.Neg (.atom a)) (.atom a))

/-- The original body permits this single atom. No dependency rank is required. -/
def ChoiceProducer.Supports (M : Atoms α) (r : ChoiceProducer α) (a : α) : Prop :=
  r.Enabled M ∧ r.head = a

/-- An atomic-choice root is classically true, with or without a body.
Its frozen reduct can still require the chosen atom. -/
theorem choice_satisfied (M : Atoms α) (r : ChoiceProducer α) :
    Satisfies M r.formula := by
  cases r with
  | fact a => exact Classical.em (M a)
  | rule _ a => exact fun _ => Classical.em (M a)
  | reversedFact a => exact Or.symm (Classical.em (M a))
  | reversedRule _ a => exact fun _ => Or.symm (Classical.em (M a))

/-- Preserving each enabled, chosen head satisfies the choice's frozen reduct.
An inactive original body cannot become true in its reduct. Otherwise the
atomic-choice guard requires only the original chosen head. -/
theorem choice_reduct_of_heads (M J : Atoms α) (r : ChoiceProducer α)
    (heads : r.Enabled M → M r.head → J r.head) :
    Satisfies J (Reduct M r.formula) := by
  cases r with
  | fact a => exact (choice_reduct_guard M J a).mpr (heads trivial)
  | reversedFact a =>
    exact (TightPlans.reversed_choice_reduct_guard M J a).mpr (heads trivial)
  | rule B a =>
    apply (RuleFactorization.reduct_imp M J B
      (.disj (.atom a) (Ferraris.Neg (.atom a)))).mpr
    refine ⟨choice_satisfied M (.rule B a), ?_⟩
    intro body
    exact (choice_reduct_guard M J a).mpr
      (heads (TightPlans.original_of_reduct M J B body))
  | reversedRule B a =>
    apply (RuleFactorization.reduct_imp M J B
      (.disj (Ferraris.Neg (.atom a)) (.atom a))).mpr
    refine ⟨choice_satisfied M (.reversedRule B a), ?_⟩
    intro body
    exact (TightPlans.reversed_choice_reduct_guard M J a).mpr
      (heads (TightPlans.original_of_reduct M J B body))

/-- Deleting an atom unsupported by a choice preserves that choice's reduct.
An enabled chosen head must be a different atom and therefore survives deletion. -/
theorem choice_removal (M : Atoms α) (r : ChoiceProducer α) (a : α)
    (unsupported : ¬ r.Supports M a) :
    Satisfies (remove M a) (Reduct M r.formula) := by
  apply choice_reduct_of_heads M (remove M a) r
  intro enabled present
  refine ⟨present, ?_⟩
  intro same
  exact unsupported ⟨enabled, same⟩

/-- Both producer families and frozen constraints cover every original root.
No unrecognized asserted formula may be omitted from this premise. -/
def CoveredWithChoices (T : Theory α) (rules : List (Producer α))
    (choices : List (ChoiceProducer α)) : Prop :=
  ∀ F ∈ T, (∃ r ∈ rules, r.formula = F) ∨
    (∃ c ∈ choices, c.formula = F) ∨ (∃ G, F = Ferraris.Neg G) ∨ F = .bot

/-- Every answer-set atom has ordinary sole-head support or enabled choice support.

If neither support exists, deleting the atom preserves every ordinary producer
and every choice reduct by the two removal laws. Frozen constraints survive as
before. Complete root coverage then contradicts reduct minimality. This is only
a necessary restriction: unranked support does not establish membership.
-/
theorem answer_set_supported_with_choices (M : Atoms α) (T : Theory α)
    (rules : List (Producer α)) (choices : List (ChoiceProducer α))
    (covered : CoveredWithChoices T rules choices)
    (answer : Stable M T) :
    ∀ a, M a → (∃ r ∈ rules, r.Supports M a) ∨
      (∃ c ∈ choices, c.Supports M a) := by
  classical
  intro a present
  apply Classical.byContradiction
  intro unsupported
  have reductModel : Models (remove M a) (ReductTheory M T) := by
    intro reduced member
    obtain ⟨F, asserted, rfl⟩ := List.mem_map.mp member
    rcases covered F asserted with
      ⟨r, inRules, rfl⟩ | ⟨c, inChoices, rfl⟩ | ⟨G, rfl⟩ | rfl
    · apply producer_removal M r a (answer.1 r.formula asserted)
      exact fun support => unsupported (Or.inl ⟨r, inRules, support⟩)
    · apply choice_removal M c a
      exact fun support => unsupported (Or.inr ⟨c, inChoices, support⟩)
    · apply (TightPlans.negation_frozen M (remove M a) G).mpr
      exact answer.1 (Ferraris.Neg G) asserted
    · exact False.elim (answer.1 .bot asserted)
  have proper : ProperSub (remove M a) M := by
    refine ⟨fun _ member => member.1, ?_⟩
    intro reverse
    exact (reverse a present).2 rfl
  exact answer.2 ⟨remove M a, proper, reductModel⟩

end Zetesis.DisjunctiveSupport
