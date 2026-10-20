import Zetesis.DisjunctiveSupport

/-!
# Possible foundedness from an upper body relaxation

A set P closed under every possible producer contains every answer set compatible
with its permitted atoms. Ordinary producers contribute all permitted head atoms,
not one selected head; atomic choices contribute their one permitted atom.

Bodies may be arbitrary formulas. The body relaxation must overestimate reduct
truth below P at the candidate under consideration. A concrete relaxation keeps
atoms, conjunctions and disjunctions, treats other implications as possible, and
may remove formulas known false in the original candidate. Complete root coverage
is supplied by DisjunctiveSupport.CoveredWithChoices.

The central argument uses J = M intersect P. Every enabled reduct producer keeps
a head true in M, every chosen choice head survives, and satisfied constraints
have true reducts. Thus J models the whole reduct; minimality forces M into P.
Any closed P suffices. Least closure, finite enumeration, Boolean DAG extraction,
aggregate handling, incidence queues, mask validity and interruption/resource
behavior remain independent implementation obligations.
-/

namespace Zetesis.PossibleFoundedness

universe u
variable {α : Type u}
open Ferraris DisjunctiveSupport

/-- Retain exactly the candidate atoms present in the possible set. -/
def intersection (M P : Atoms α) : Atoms α := fun a => M a ∧ P a

/-- The relaxation cannot reject a body whose frozen reduct is true below P.
This premise is candidate-relative: known-false information must be valid for M.
It makes no claim about an unrelated candidate or an incomplete execution prefix. -/
def UpperSound (M P : Atoms α) (upper : Formula α → Prop) : Prop :=
  ∀ F J, Sub J P → Satisfies J (Reduct M F) → upper F

/-- Facts are always possible; a rule is possible when its relaxed body is true. -/
def producerEnabled (upper : Formula α → Prop) : Producer α → Prop
  | .fact _ => True
  | .rule B _ => upper B

/-- Atomic choices use the same relaxed body in either branch order. -/
def choiceEnabled (upper : Formula α → Prop) : ChoiceProducer α → Prop
  | .fact _ | .reversedFact _ => True
  | .rule B _ | .reversedRule B _ => upper B

/-- Every permitted head occurrence of an enabled ordinary producer is in P.
No competing head or held atom may suppress another permitted head here. -/
def ProducersClosed (P allowed : Atoms α) (upper : Formula α → Prop)
    (rules : List (Producer α)) : Prop :=
  ∀ r ∈ rules, producerEnabled upper r →
    ∀ a, r.head.Contains a → allowed a → P a

/-- Every permitted head of an enabled atomic choice is in P. -/
def ChoicesClosed (P allowed : Atoms α) (upper : Formula α → Prop)
    (choices : List (ChoiceProducer α)) : Prop :=
  ∀ c ∈ choices, choiceEnabled upper c → allowed c.head → P c.head

/-- An original ordinary producer remains true in the reduct after intersection.
If its reduct body is true, upper soundness enables the producer in P; original
satisfaction supplies a true head, and closure ensures that head survives. -/
theorem producer_intersection (M P allowed : Atoms α) (upper : Formula α → Prop)
    (sound : UpperSound M P upper) (compatible : Sub M allowed)
    (r : Producer α) (original : Satisfies M r.formula)
    (closed : producerEnabled upper r →
      ∀ a, r.head.Contains a → allowed a → P a) :
    Satisfies (intersection M P) (Reduct M r.formula) := by
  have below : Sub (intersection M P) P := fun _ present => present.2
  cases r with
  | fact H =>
    obtain ⟨a, occurs, present⟩ := (head_satisfies M H).mp original
    have possible : P a := closed trivial a occurs (compatible a present)
    exact (head_reduct M (intersection M P) H).mpr
      ⟨a, occurs, present, present, possible⟩
  | rule B H =>
    apply (RuleFactorization.reduct_imp M (intersection M P) B H.formula).mpr
    refine ⟨original, ?_⟩
    intro body
    have enabled : upper B := sound B (intersection M P) below body
    have trueHead : Satisfies M H.formula :=
      original (TightPlans.original_of_reduct M (intersection M P) B body)
    obtain ⟨a, occurs, present⟩ := (head_satisfies M H).mp trueHead
    have possible : P a := closed enabled a occurs (compatible a present)
    exact (head_reduct M (intersection M P) H).mpr
      ⟨a, occurs, present, present, possible⟩

/-- A choice survives intersection whenever its reduct body enables it.
An atom selected in M survives by closure. An unselected atom needs no support:
its negative alternative already satisfies the frozen choice. -/
theorem choice_intersection (M P allowed : Atoms α) (upper : Formula α → Prop)
    (sound : UpperSound M P upper) (compatible : Sub M allowed)
    (c : ChoiceProducer α)
    (closed : choiceEnabled upper c → allowed c.head → P c.head) :
    Satisfies (intersection M P) (Reduct M c.formula) := by
  have below : Sub (intersection M P) P := fun _ present => present.2
  cases c with
  | fact a =>
    apply (choice_reduct_guard M (intersection M P) a).mpr
    intro present
    exact ⟨present, closed trivial (compatible a present)⟩
  | reversedFact a =>
    apply (TightPlans.reversed_choice_reduct_guard M (intersection M P) a).mpr
    intro present
    exact ⟨present, closed trivial (compatible a present)⟩
  | rule B a =>
    apply (RuleFactorization.reduct_imp M (intersection M P) B
      (.disj (.atom a) (Ferraris.Neg (.atom a)))).mpr
    refine ⟨choice_satisfied M (.rule B a), ?_⟩
    intro body
    have enabled : upper B := sound B (intersection M P) below body
    apply (choice_reduct_guard M (intersection M P) a).mpr
    intro present
    exact ⟨present, closed enabled (compatible a present)⟩
  | reversedRule B a =>
    apply (RuleFactorization.reduct_imp M (intersection M P) B
      (.disj (Ferraris.Neg (.atom a)) (.atom a))).mpr
    refine ⟨choice_satisfied M (.reversedRule B a), ?_⟩
    intro body
    have enabled : upper B := sound B (intersection M P) below body
    apply (TightPlans.reversed_choice_reduct_guard M (intersection M P) a).mpr
    intro present
    exact ⟨present, closed enabled (compatible a present)⟩

/-- Complete producer and constraint coverage makes the intersection a model
of the entire frozen reduct. Ordinary producers and choices follow the two
preceding laws; satisfied original constraints have true reducts everywhere. -/
theorem intersection_models_reduct (M P allowed : Atoms α) (upper : Formula α → Prop)
    (T : Theory α) (rules : List (Producer α)) (choices : List (ChoiceProducer α))
    (covered : CoveredWithChoices T rules choices)
    (original : Models M T) (sound : UpperSound M P upper)
    (compatible : Sub M allowed)
    (producers : ProducersClosed P allowed upper rules)
    (choiceHeads : ChoicesClosed P allowed upper choices) :
    Models (intersection M P) (ReductTheory M T) := by
  intro reduced member
  obtain ⟨F, asserted, rfl⟩ := List.mem_map.mp member
  rcases covered F asserted with
    ⟨r, inRules, rfl⟩ | ⟨c, inChoices, rfl⟩ | ⟨G, rfl⟩ | rfl
  · exact producer_intersection M P allowed upper sound compatible r
      (original r.formula asserted) (producers r inRules)
  · exact choice_intersection M P allowed upper sound compatible c
      (choiceHeads c inChoices)
  · exact (TightPlans.negation_frozen M (intersection M P) G).mpr
      (original (Ferraris.Neg G) asserted)
  · exact False.elim (original .bot asserted)

/-- Every compatible answer set is contained in a closed possible set P.

The intersection J = M intersect P lies below M and, by the preceding law,
models every frozen reduct root. Reduct minimality therefore gives M below J,
and hence below P. Neither leastness of P nor a dependency ranking is required.
This proves a necessary candidate restriction, not that P is an answer set.
-/
theorem stable_sub_possible (M P allowed : Atoms α) (upper : Formula α → Prop)
    (T : Theory α) (rules : List (Producer α)) (choices : List (ChoiceProducer α))
    (covered : CoveredWithChoices T rules choices)
    (sound : UpperSound M P upper) (compatible : Sub M allowed)
    (producers : ProducersClosed P allowed upper rules)
    (choiceHeads : ChoicesClosed P allowed upper choices)
    (answer : Stable M T) : Sub M P := by
  have reduced : Models (intersection M P) (ReductTheory M T) :=
    intersection_models_reduct M P allowed upper T rules choices covered
      answer.1 sound compatible producers choiceHeads
  have below : Sub (intersection M P) M := fun _ present => present.1
  have reverse : Sub M (intersection M P) :=
    ((stable_iff_minimal_reduct M T).mp answer).2 (intersection M P) below reduced
  exact fun a present => (reverse a present).2

/-- Recomputing support for selected heads preserves every compatible answer set
when all other permitted atoms remain in the possible set.

Every producer entering the selected heads must be closed, including producers
whose bodies did not change. A head outside that selection is already present
by the boundary premise. These two cases establish whole-program closure, so
`stable_sub_possible` applies.

The selection may be conservative: this law proves safe exclusion, not equality
with a full least-closure computation. Dependency selection, inherited completion,
exact producer coverage, concrete body evaluation and interrupted execution
remain separate implementation obligations.
-/
theorem stable_sub_locally_closed (M P allowed selected : Atoms α)
    (upper : Formula α → Prop)
    (T : Theory α) (rules : List (Producer α)) (choices : List (ChoiceProducer α))
    (covered : CoveredWithChoices T rules choices)
    (sound : UpperSound M P upper) (compatible : Sub M allowed)
    (boundary : ∀ a, allowed a → ¬ selected a → P a)
    (producers : ∀ r ∈ rules, producerEnabled upper r →
      ∀ a, r.head.Contains a → allowed a → selected a → P a)
    (choiceHeads : ∀ c ∈ choices, choiceEnabled upper c →
      allowed c.head → selected c.head → P c.head)
    (answer : Stable M T) : Sub M P := by
  have ordinaryClosed : ProducersClosed P allowed upper rules := by
    intro r member enabled a occurs permitted
    by_cases affected : selected a
    · exact producers r member enabled a occurs permitted affected
    · exact boundary a permitted affected
  have choicesClosed : ChoicesClosed P allowed upper choices := by
    intro c member enabled permitted
    by_cases affected : selected c.head
    · exact choiceHeads c member enabled permitted affected
    · exact boundary c.head permitted affected
  exact stable_sub_possible M P allowed upper T rules choices covered sound
    compatible ordinaryClosed choicesClosed answer

/-- A condition that blocks every producer entering U excludes every atom of U
from each matching answer set.

The condition describes interpretations, not an ancestor region or a frozen
query. Its upper-soundness premise must hold separately for every matching
interpretation. Every ordinary producer with any head in U is blocked, even
when another head lies outside U, and every atomic choice into U is blocked.

Take P to be the complement of U and permit all atoms. Producers whose heads
lie outside U are already closed in P; the premises disable every other
producer. `stable_sub_possible` then places each matching answer set inside P.
This semantic law does not certify an executable explanation collector, signed
atom coordinates, matching, ownership, resource bounds or interruption handling.
-/
theorem conditional_unsupported_absent
    (T : Theory α) (rules : List (Producer α)) (choices : List (ChoiceProducer α))
    (covered : CoveredWithChoices T rules choices)
    (condition : Atoms α → Prop) (unsupported : Atoms α)
    (upper : Formula α → Prop)
    (sound : ∀ M, condition M → UpperSound M (fun a => ¬ unsupported a) upper)
    (ordinaryBlocked : ∀ r ∈ rules,
      (∃ a, r.head.Contains a ∧ unsupported a) → ¬ producerEnabled upper r)
    (choicesBlocked : ∀ c ∈ choices, unsupported c.head → ¬ choiceEnabled upper c)
    (M : Atoms α) (matching : condition M) (answer : Stable M T) :
    ∀ a, unsupported a → ¬ M a := by
  have ordinaryClosed :
      ProducersClosed (fun a => ¬ unsupported a) (fun _ => True) upper rules := by
    intro r member enabled a head _ inside
    exact ordinaryBlocked r member ⟨a, head, inside⟩ enabled
  have choiceClosed :
      ChoicesClosed (fun a => ¬ unsupported a) (fun _ => True) upper choices := by
    intro c member enabled _ inside
    exact choicesBlocked c member inside enabled
  have possible : Sub M (fun a => ¬ unsupported a) :=
    stable_sub_possible M (fun a => ¬ unsupported a) (fun _ => True) upper
      T rules choices covered (sound M matching) (fun _ _ => trivial)
      ordinaryClosed choiceClosed answer
  intro a inside present
  exact possible a present inside

/-- A present unsupported atom forces the remaining certificate condition to fail.

Suppose the condition C together with a remaining condition excludes every atom
of U by `conditional_unsupported_absent`. An answer set satisfying C and containing
an atom of U cannot satisfy the remaining condition: otherwise the same answer
set would both contain and omit that atom.

Both conditions are predicates on interpretations; no signed-literal encoding or
partial-region state is assumed. The Rust rule for exactly one unknown condition
must separately establish that all other conditions hold in every compatible
answer set and that an output atom is held. This law does not verify that
classification, event propagation, ownership or resource handling.
-/
theorem conditional_unsupported_forces_complement
    (T : Theory α) (rules : List (Producer α)) (choices : List (ChoiceProducer α))
    (covered : CoveredWithChoices T rules choices)
    (condition remaining : Atoms α → Prop) (unsupported : Atoms α)
    (upper : Formula α → Prop)
    (sound : ∀ M, condition M ∧ remaining M →
      UpperSound M (fun a => ¬ unsupported a) upper)
    (ordinaryBlocked : ∀ r ∈ rules,
      (∃ a, r.head.Contains a ∧ unsupported a) → ¬ producerEnabled upper r)
    (choicesBlocked : ∀ c ∈ choices, unsupported c.head → ¬ choiceEnabled upper c)
    (M : Atoms α) (matching : condition M)
    (present : ∃ a, unsupported a ∧ M a) (answer : Stable M T) :
    ¬ remaining M := by
  intro holds
  have absent : ∀ a, unsupported a → ¬ M a :=
    conditional_unsupported_absent T rules choices covered
      (fun N => condition N ∧ remaining N) unsupported upper sound
      ordinaryBlocked choicesBlocked M ⟨matching, holds⟩ answer
  obtain ⟨a, inside, held⟩ := present
  exact absent a inside held

/-- Keep positive Boolean structure; an implication is possible unless known
false. The known-false predicate is tested at each retained syntax node.
Aggregates are outside Formula and require their own upper-soundness proof. -/
def bodyUpper (P : Atoms α) (knownFalse : Formula α → Prop) : Formula α → Prop
  | .atom a => ¬ knownFalse (.atom a) ∧ P a
  | .bot => False
  | .conj F G => ¬ knownFalse (.conj F G) ∧ bodyUpper P knownFalse F ∧ bodyUpper P knownFalse G
  | .disj F G => ¬ knownFalse (.disj F G) ∧ (bodyUpper P knownFalse F ∨ bodyUpper P knownFalse G)
  | .imp F G => ¬ knownFalse (.imp F G)

/-- The positive Boolean relaxation is upper-sound when each known-false mark
is valid in M. Reduct truth implies original truth, so no true reduct node is
removed by the mask; atoms transfer through J below P and Boolean cases recurse.
No held atom is used as a seed and implications supply no negative dependency. -/
theorem body_upper_sound (M P : Atoms α) (knownFalse : Formula α → Prop)
    (valid : ∀ F, knownFalse F → ¬ Satisfies M F) :
    UpperSound M P (bodyUpper P knownFalse) := by
  intro F
  induction F with
  | atom a =>
    intro J below reduced
    obtain ⟨original, present⟩ := (atom_reduct M J a).mp reduced
    exact ⟨fun absent => valid (.atom a) absent original, below a present⟩
  | bot =>
    intro J _ reduced
    exact False.elim reduced
  | conj F G ihF ihG =>
    intro J below reduced
    have unmasked : ¬ knownFalse (.conj F G) := fun absent =>
      valid (.conj F G) absent (TightPlans.original_of_reduct M J (.conj F G) reduced)
    have children := (RuleFactorization.reduct_conj M J F G).mp reduced
    exact ⟨unmasked, ihF J below children.1, ihG J below children.2⟩
  | disj F G ihF ihG =>
    intro J below reduced
    have unmasked : ¬ knownFalse (.disj F G) := fun absent =>
      valid (.disj F G) absent (TightPlans.original_of_reduct M J (.disj F G) reduced)
    have children := (RuleFactorization.reduct_disj M J F G).mp reduced
    refine ⟨unmasked, ?_⟩
    rcases children with left | right
    · exact Or.inl (ihF J below left)
    · exact Or.inr (ihG J below right)
  | imp F G _ _ =>
    intro J _ reduced
    exact fun absent =>
      valid (.imp F G) absent (TightPlans.original_of_reduct M J (.imp F G) reduced)

end Zetesis.PossibleFoundedness
