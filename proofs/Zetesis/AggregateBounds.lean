import Zetesis.AggregateConsumers
import Zetesis.ChoiceIntervals

/-!
# Choice bounds produced by aggregate proposals

An independent aggregate carrier proposes a value; an instance retains the
original equality and activation formula. Bounds consume that value without
asserting the equality. Active bounds reject candidates outside their interval;
inactive instances impose no frozen obligation. Eligibility remains a formula.

Finite carrier completeness, evaluation of the actual bound, distinct head
identity and source scope safety are premises inherited from the caller. These
laws cover natural cardinality intervals. They do not prove Rust evaluation,
signed comparison lowering, proposal enumeration, partial-domain pruning or
machine/resource bounds. In particular, no new candidate-search pruning is
implemented by admitting more source bound expressions.
-/

namespace Zetesis.AggregateBounds
open Ferraris ChoiceIntervals
universe u
variable {A : Type u}

/-- Every instance retains both its aggregate equality and rule activation. -/
def boundRule (equality activation : Formula A) (lower upper : Nat)
    (heads : List A) (eligible : A → Formula A) : Formula A :=
  group (.conj equality activation) lower upper heads eligible

/-- An active instance enforces the count of selected, eligible head atoms. -/
theorem active_bounds (M : Atoms A) (equality activation : Formula A)
    (lower upper : Nat) (heads : List A) (eligible : A → Formula A)
    (model : Satisfies M (boundRule equality activation lower upper heads eligible))
    (actual : Satisfies M equality) (active : Satisfies M activation) :
    lower ≤ count (fun head => M head ∧ Satisfies M (eligible head)) heads ∧
      count (fun head => M head ∧ Satisfies M (eligible head)) heads ≤ upper := by
  have guarded := (group_classical M (.conj equality activation)
    lower upper heads eligible).mp model
  exact guarded ⟨actual, active⟩

/-- A complete candidate outside an active interval is not a model of the rule. -/
theorem violated_bound_refutes (M : Atoms A) (equality activation : Formula A)
    (lower upper : Nat) (heads : List A) (eligible : A → Formula A)
    (actual : Satisfies M equality) (active : Satisfies M activation)
    (outside : count (fun head => M head ∧ Satisfies M (eligible head)) heads < lower ∨
      upper < count (fun head => M head ∧ Satisfies M (eligible head)) heads) :
    ¬ Satisfies M (boundRule equality activation lower upper heads eligible) := by
  intro model
  have within := active_bounds M equality activation lower upper heads eligible
    model actual active
  rcases outside with below | above
  · exact (Nat.not_lt_of_ge within.1) below
  · exact (Nat.not_lt_of_ge within.2) above

/-- An unrealized proposal or inactive body cannot impose a reduct constraint. -/
theorem inactive_frozen (M J : Atoms A) (equality activation : Formula A)
    (lower upper : Nat) (heads : List A) (eligible : A → Formula A)
    (inactive : ¬ Satisfies M (.conj equality activation)) :
    Satisfies J (Reduct M (boundRule equality activation lower upper heads eligible)) := by
  have body_false := RuleFactorization.false_reduct M
    (.conj equality activation) inactive
  simp only [boundRule, group, RuleFactorization.reduct_imp]
  constructor
  · intro holds
    exact False.elim (inactive holds)
  · rw [body_false]
    exact False.elim

/-- Proposal membership never activates a rule whose equality is false. -/
theorem proposal_does_not_activate :
    1 ∈ [0, 1] ∧ Satisfies (fun _ : Unit => False)
      (boundRule .bot top 1 1 [()] (fun _ => top)) := by
  constructor
  · simp
  · intro impossible
    exact False.elim impossible.1

end Zetesis.AggregateBounds
