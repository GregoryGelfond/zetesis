import Zetesis.DisjunctiveSupport

/-!
# Ranked shifted support for tight head disjunctions

A ranked certificate over normal, atomic-choice and positive head-disjunction
producers lets original satisfaction and *shifted* support discharge reduct
minimality. An atom is shifted-supported by a producer whose body is true and
whose head contains it as its only present atom. The original disjunctive root
is retained; no rule of the theory is rewritten, and the shift appears only in
the support condition. The rank requires every unnegated body atom to lie
strictly below every atom of the producer's head, which makes the program tight
and therefore head-cycle-free.

`DisjunctiveSupport.answer_set_supported` already states the necessary
direction for disjunctive producers. This file adds sufficiency: with a checked
rank, original satisfaction plus shifted support establishes stability. As in
`TightPlans`, the laws do not prove that Rust DAG extraction, flattening of
nested head disjunctions, rank derivation or the optimized check refine these
definitions.
-/

namespace Zetesis.ShiftedSupport

universe u
variable {α : Type u}
open Ferraris
open Zetesis.DisjunctiveSupport (Head head_reduct)

/-- A tight producer, or a positive head disjunction with an optional ranked body. -/
inductive Producer (α : Type u) where
  | tight : TightPlans.Producer α → Producer α
  | disjFact : Head α → Producer α
  | disjRule : TightPlans.Body α → Head α → Producer α

def Producer.body : Producer α → TightPlans.Body α
  | .tight r => r.body
  | .disjFact _ => TightPlans.truthBody
  | .disjRule B _ => B

def Producer.formula : Producer α → Formula α
  | .tight r => r.formula
  | .disjFact H => H.formula
  | .disjRule B H => .imp B.formula H.formula

/-- Head occurrence: the single head of a tight producer, or any atom of a disjunction. -/
def Producer.Contains : Producer α → α → Prop
  | .tight r, a => a = r.head
  | .disjFact H, a => H.Contains a
  | .disjRule _ H, a => H.Contains a

/-- Shifted support: a true body, and `a` is the only present atom of the head. -/
def Producer.Supports (M : Atoms α) (r : Producer α) (a : α) : Prop :=
  Satisfies M r.body.formula ∧ r.Contains a ∧ ∀ b, r.Contains b → M b → b = a

def Supported (M : Atoms α) (rules : List (Producer α)) : Prop :=
  ∀ a, M a → ∃ r ∈ rules, r.Supports M a

/-- Every unnegated body atom is ranked below every head atom of its producer. -/
def Ranked (rank : α → Nat) (rules : List (Producer α)) : Prop :=
  ∀ r ∈ rules, ∀ a, r.Contains a → ∀ b, r.body.Positive b → rank b < rank a

def OriginalProducers (T : Theory α) (rules : List (Producer α)) : Prop :=
  ∀ r ∈ rules, r.formula ∈ T

/-- A frozen positive head whose only present atom is `a` forces `a` in a reduct model. -/
theorem sole_head_forced (M J : Atoms α) (H : Head α) (a : α)
    (sole : ∀ b, H.Contains b → M b → b = a)
    (reduced : Satisfies J (Reduct M H.formula)) : J a := by
  obtain ⟨b, occurs, present, inJ⟩ := (head_reduct M J H).mp reduced
  have same : b = a := sole b occurs present
  exact same ▸ inJ

/-- A shifted-supporting producer forces its head in every model of the reduct
    in which its reduced body holds. -/
theorem producer_forces_head (M J : Atoms α) (r : Producer α) (a : α)
    (present : M a) (supports : r.Supports M a)
    (root : Satisfies J (Reduct M r.formula))
    (body : Satisfies J (Reduct M r.body.formula)) : J a := by
  obtain ⟨_, occurs, sole⟩ := supports
  cases r with
  | tight r =>
    have head : a = r.head := occurs
    subst head
    exact TightPlans.producer_forces_head M J r present root body
  | disjFact H => exact sole_head_forced M J H a sole root
  | disjRule B H =>
    have reducedHead := ((RuleFactorization.reduct_imp M J B.formula H.formula).mp root).2 body
    exact sole_head_forced M J H a sole reducedHead

/-- Induction over the checked rank makes every reduct model contain M. -/
theorem ranked_support_contained (M J : Atoms α) (T : Theory α)
    (rules : List (Producer α)) (rank : α → Nat)
    (original : OriginalProducers T rules) (ranked : Ranked rank rules)
    (supported : Supported M rules) (reduced : Models J (ReductTheory M T)) :
    Sub M J := by
  have byRank : ∀ n, ∀ a, rank a = n → M a → J a := by
    intro n
    induction n using Nat.strongRecOn with
    | ind n ih =>
      intro a ha present
      obtain ⟨r, hr, supports⟩ := supported a present
      have lower : ∀ b, r.body.Positive b → M b → J b := by
        intro b hb presentB
        have lt : rank b < n := ha ▸ ranked r hr a supports.2.1 b hb
        exact ih (rank b) lt b rfl presentB
      have bodyJ := TightPlans.body_reduct_of_positive M J r.body lower supports.1
      have rootJ : Satisfies J (Reduct M r.formula) :=
        reduced (Reduct M r.formula) (List.mem_map.mpr ⟨r.formula, original r hr, rfl⟩)
      exact producer_forces_head M J r a present supports rootJ bodyJ
  intro a present
  exact byRank (rank a) a rfl present

/-- Original satisfaction plus ranked shifted support establishes stability. -/
theorem ranked_support_stable (M : Atoms α) (T : Theory α)
    (rules : List (Producer α)) (rank : α → Nat)
    (original : OriginalProducers T rules) (ranked : Ranked rank rules)
    (model : Models M T) (supported : Supported M rules) : Stable M T := by
  refine ⟨model, ?_⟩
  rintro ⟨J, ⟨_, proper⟩, reduced⟩
  exact proper (ranked_support_contained M J T rules rank original ranked supported reduced)

end Zetesis.ShiftedSupport
