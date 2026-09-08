import Zetesis.AggregateBounds

/-!
# Completed value consumers in a choice-rule body

A completed aggregate proposal supplies scalar values to a static body filter
and to the bounds of one whole choice group. The original aggregate equality,
ordinary activation and head eligibility remain formulas. The bounds constrain
the group as a whole; evaluating a proposal never proves its equality.

These laws compose finite-value coverage with the existing choice semantics.
They assume a complete proposal carrier, total scalar evaluation and natural
cardinality bounds. The Rust source scheduler, captured-argument correspondence,
signed arithmetic, local scope safety, tuple/atom correspondence for count heads,
support completeness and resource/error behavior remain unproved implementation
obligations. No Rust, WGSL or hardware refinement is claimed by this module.
-/

namespace Zetesis.ChoiceConsumers
open Ferraris ChoiceIntervals AggregateBounds
universe u v
variable {A : Type u} {V : Type v}

/-- A static filter is part of activation of the entire original group. -/
def filteredRule (equality activation : Formula A) (accepted : Bool)
    (lower upper : Nat) (heads : List A) (eligible : A → Formula A) : Formula A :=
  boundRule equality (.conj activation (if accepted then top else .bot))
    lower upper heads eligible

/-- Classical bounds are required exactly when the original body and its
    evaluated scalar filter are true. In particular, bounds supply no equality. -/
theorem original_bounds (M : Atoms A) (equality activation : Formula A)
    (accepted : Bool) (lower upper : Nat) (heads : List A)
    (eligible : A → Formula A) :
    Satisfies M (filteredRule equality activation accepted lower upper heads eligible) ↔
      (Satisfies M equality ∧ Satisfies M activation ∧ accepted = true →
        lower ≤ count (fun head => M head ∧ Satisfies M (eligible head)) heads ∧
        count (fun head => M head ∧ Satisfies M (eligible head)) heads ≤ upper) := by
  have group_bounds := group_classical M
    (.conj equality (.conj activation (if accepted then top else .bot)))
    lower upper heads eligible
  cases accepted <;>
    simpa [filteredRule, boundRule, Satisfies, top] using group_bounds

/-- A rejected completed row imposes no obligation in any frozen world,
    even when its proposed lower bound is positive and the group is empty. -/
theorem rejected_frozen (M J : Atoms A) (equality activation : Formula A)
    (lower upper : Nat) (heads : List A) (eligible : A → Formula A) :
    Satisfies J (Reduct M
      (filteredRule equality activation false lower upper heads eligible)) := by
  have inactive : ¬ Satisfies M (.conj equality (.conj activation .bot)) := by
    intro holds
    exact holds.2.2
  exact inactive_frozen M J equality (.conj activation .bot)
    lower upper heads eligible inactive

/-- Even an accepted proposal does not activate a group when its original
    aggregate equality is false in the frozen candidate. -/
theorem unrealized_frozen (M J : Atoms A) (equality activation : Formula A)
    (accepted : Bool) (lower upper : Nat) (heads : List A)
    (eligible : A → Formula A) (unrealized : ¬ Satisfies M equality) :
    Satisfies J (Reduct M
      (filteredRule equality activation accepted lower upper heads eligible)) := by
  have inactive : ¬ Satisfies M
      (.conj equality (.conj activation (if accepted then top else .bot))) := by
    intro holds
    exact unrealized holds.1
  exact inactive_frozen M J equality
    (.conj activation (if accepted then top else .bot)) lower upper heads eligible inactive

/-- Inserting a successful static filter preserves original and frozen group
    semantics, including every eligibility formula and both cardinality bounds. -/
theorem accepted_equivalent (equality activation : Formula A)
    (lower upper : Nat) (heads : List A) (eligible : A → Formula A) :
    Equivalent (filteredRule equality activation true lower upper heads eligible)
      (boundRule equality activation lower upper heads eligible) := by
  have activation_same : Equivalent (.conj activation top) activation := by
    constructor
    · intro M
      simp only [Satisfies, top, false_implies, and_true]
    · intro M J
      rw [RuleFactorization.reduct_conj]
      have top_frozen : Satisfies J (Reduct M (top : Formula A)) := by
        simp only [top, Reduct, Satisfies, false_implies, ↓reduceIte]
      exact and_iff_left top_frozen
  have body_same := equivalent_conj equality (.conj activation top) equality activation
    (equivalent_refl equality) activation_same
  exact equivalent_imp _ _ _ _ body_same (equivalent_refl _)

/-- A complete finite carrier preserves the actual lower/upper pair computed
    from an accepted aggregate value. Coverage and evaluation are premises. -/
theorem covered_bounds (carrier : List V) (keep : V → Bool)
    (bounds : V → Nat × Nat) (actual : V)
    (covered : actual ∈ carrier) (accepted : keep actual = true) :
    (actual, bounds actual) ∈ AggregateConsumers.substitutions carrier keep bounds :=
  AggregateConsumers.covered_consumer carrier keep bounds actual covered accepted

/-- Substituting equal evaluated bounds preserves the whole group in an
    arbitrary frozen formula context; all activation formulas stay unchanged. -/
theorem equal_bounds_frozen (M J : Atoms A) (equality activation : Formula A)
    (accepted : Bool) (left right : Nat × Nat) (same : left = right)
    (heads : List A) (eligible : A → Formula A) (context : Formula A → Formula A) :
    Satisfies J (Reduct M (context
      (filteredRule equality activation accepted left.1 left.2 heads eligible))) ↔
      Satisfies J (Reduct M (context
        (filteredRule equality activation accepted right.1 right.2 heads eligible))) := by
  subst right
  rfl

end Zetesis.ChoiceConsumers
