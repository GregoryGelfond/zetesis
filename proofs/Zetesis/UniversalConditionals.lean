import Zetesis.ChoiceIntervals

/-!
# Finite universal body conditionals

An already supplied finite row list denotes the conjunction of its
condition-to-consequent implications. Empty completed lists are vacuously true.
Original and frozen truth retain both parts of every implication; no candidate
truth test may erase a retained antecedent from its reduct test.

An antecedent false at the fixed original candidate reduces to bottom at every
tested interpretation. Its row may therefore be omitted for that candidate.
Reusing the reduced list across candidates requires uniform falsity on the
admitted candidate domain. This supports exact per-candidate specialization
without confusing it with static source compilation.

The list is assumed complete for the intended finite interpretation. The source
join, possible-positive support, safety, substitutions, resource exhaustion,
dependency analysis, support guards and Rust compiler are not refined here.
-/

namespace Zetesis.UniversalConditionals

universe u
variable {α : Type u}
open Ferraris
open ChoiceIntervals (all top)

structure Instance (α : Type u) where
  condition : Formula α
  consequent : Formula α

def universal (instances : List (Instance α)) : Formula α :=
  all (instances.map (fun row => .imp row.condition row.consequent))

/-- The existing conjunction constructor distributes over every frozen pair;
    no assumption that the tested interpretation is a subset is needed. -/
theorem all_frozen (M J : Atoms α) (formulas : List (Formula α)) :
    Satisfies J (Reduct M (all formulas)) ↔
      ∀ formula ∈ formulas, Satisfies J (Reduct M formula) := by
  induction formulas with
  | nil => simp [all, top, Reduct, Satisfies]
  | cons first rest ih => simp [all, RuleFactorization.reduct_conj, ih]

theorem original_semantics (M : Atoms α) (instances : List (Instance α)) :
    Satisfies M (universal instances) ↔
      ∀ row ∈ instances, Satisfies M row.condition → Satisfies M row.consequent := by
  simp [universal, ChoiceIntervals.all_classical, Satisfies]

/-- Each implication must be true in M, and its retained reduct implication
    must also hold in J. The second condition cannot use M in place of J. -/
theorem frozen_semantics (M J : Atoms α) (instances : List (Instance α)) :
    Satisfies J (Reduct M (universal instances)) ↔
      ∀ row ∈ instances,
        (Satisfies M row.condition → Satisfies M row.consequent) ∧
        (Satisfies J (Reduct M row.condition) → Satisfies J (Reduct M row.consequent)) := by
  simp [universal, all_frozen, RuleFactorization.reduct_imp, Satisfies]

/-- This empty list denotes successful finite completion, not an interrupted
    prefix or a generator whose remaining substitutions have not been covered. -/
theorem empty_vacuity (M J : Atoms α) :
    Satisfies M (universal ([] : List (Instance α))) ∧
      Satisfies J (Reduct M (universal ([] : List (Instance α)))) := by
  simp [original_semantics, frozen_semantics]

/-- Original falsity alone makes the discarded antecedent false in every
    frozen world, even when J is not a subset of M. -/
theorem false_condition_vacuity (M J : Atoms α) (row : Instance α)
    (inactive : ¬ Satisfies M row.condition) :
    Satisfies M (.imp row.condition row.consequent) ∧
      Satisfies J (Reduct M (.imp row.condition row.consequent)) := by
  simp [RuleFactorization.reduct_imp, RuleFactorization.false_reduct M row.condition inactive,
    Satisfies, inactive]

/-- A proposed filter's exact obligation for this fixed original candidate. -/
def DiscardedFalse (M : Atoms α) (instances : List (Instance α))
    (keep : Instance α → Bool) : Prop :=
  ∀ row ∈ instances, keep row = false → ¬ Satisfies M row.condition

theorem removal_original (M : Atoms α) (instances : List (Instance α))
    (keep : Instance α → Bool) (safe : DiscardedFalse M instances keep) :
    Satisfies M (universal instances) ↔ Satisfies M (universal (instances.filter keep)) := by
  rw [original_semantics, original_semantics]
  constructor
  · intro original row member
    exact original row (List.mem_filter.mp member).1
  · intro retained row member
    cases kept : keep row with
    | false => exact fun active => False.elim (safe row member kept active)
    | true => exact retained row (List.mem_filter.mpr ⟨member, kept⟩)

theorem removal_frozen (M J : Atoms α) (instances : List (Instance α))
    (keep : Instance α → Bool) (safe : DiscardedFalse M instances keep) :
    Satisfies J (Reduct M (universal instances)) ↔
      Satisfies J (Reduct M (universal (instances.filter keep))) := by
  rw [frozen_semantics, frozen_semantics]
  constructor
  · intro original row member
    exact original row (List.mem_filter.mp member).1
  · intro retained row member
    cases kept : keep row with
    | false =>
      have vacuous := false_condition_vacuity M J row (safe row member kept)
      exact (RuleFactorization.reduct_imp M J row.condition row.consequent).mp vacuous.2
    | true => exact retained row (List.mem_filter.mpr ⟨member, kept⟩)

/-- The remaining original body and head are arbitrary unchanged formulas. -/
def rule (body head : Formula α) (instances : List (Instance α)) : Formula α :=
  .imp (.conj body (universal instances)) head

/-- Removing impossible instances preserves their actual enclosing rule, not
    just the truth value of an isolated body conjunct. -/
theorem removal_in_rule (M J : Atoms α) (body head : Formula α)
    (instances : List (Instance α)) (keep : Instance α → Bool)
    (safe : DiscardedFalse M instances keep) :
    (Satisfies M (rule body head instances) ↔
      Satisfies M (rule body head (instances.filter keep))) ∧
    (Satisfies J (Reduct M (rule body head instances)) ↔
      Satisfies J (Reduct M (rule body head (instances.filter keep)))) := by
  have original := removal_original M instances keep safe
  have frozen := removal_frozen M J instances keep safe
  constructor
  · simp only [rule, Satisfies, original]
  · simp only [rule, RuleFactorization.reduct_imp, RuleFactorization.reduct_conj,
      Satisfies, original, frozen]

/-- The surrounding theory, candidate M and every proper-subset test remain
    unchanged. This equivalence is local to M unless falsity is uniform. -/
theorem removal_keeps_candidate_stability (M : Atoms α) (body head : Formula α)
    (instances : List (Instance α)) (keep : Instance α → Bool)
    (safe : DiscardedFalse M instances keep) (context : Theory α) :
    Stable M (rule body head instances :: context) ↔
      Stable M (rule body head (instances.filter keep) :: context) := by
  have original := (removal_in_rule M M body head instances keep safe).1
  have frozen := fun J => (removal_in_rule M J body head instances keep safe).2
  simp only [Stable, models_cons, ReductTheory, List.map_cons, original, frozen]

open Classical in
/-- Candidate-specific specialization keeps each surviving implication intact.
    It does not evaluate eligibility under a reduct interpretation J. -/
noncomputable def specialize (M : Atoms α) (instances : List (Instance α)) : List (Instance α) :=
  instances.filter (fun row => decide (Satisfies M row.condition))

/-- A fixed candidate can safely skip its false antecedents during exact
    reduct checking. The retained conditions are still tested in every J. -/
theorem specialization_exact (M J : Atoms α) (instances : List (Instance α)) :
    (Satisfies M (universal instances) ↔ Satisfies M (universal (specialize M instances))) ∧
    (Satisfies J (Reduct M (universal instances)) ↔
      Satisfies J (Reduct M (universal (specialize M instances)))) := by
  classical
  have safe : DiscardedFalse M instances (fun row => decide (Satisfies M row.condition)) := by
    intro row _ inactive
    simpa using inactive
  exact ⟨removal_original M instances _ safe, removal_frozen M J instances _ safe⟩

theorem specialization_keeps_candidate_stability (M : Atoms α) (body head : Formula α)
    (instances : List (Instance α)) (context : Theory α) :
    Stable M (rule body head instances :: context) ↔
      Stable M (rule body head (specialize M instances) :: context) := by
  classical
  apply removal_keeps_candidate_stability
  intro row _ inactive
  simpa using inactive

/-- A static filter is valid on a supplied candidate domain only with this
    uniform premise. Proving support analysis supplies it is separate work. -/
theorem uniform_removal_on_domain (domain : Atoms α → Prop) (body head : Formula α)
    (instances : List (Instance α)) (keep : Instance α → Bool)
    (safe : ∀ M, domain M → DiscardedFalse M instances keep) (context : Theory α)
    (M : Atoms α) (admitted : domain M) :
    Stable M (rule body head instances :: context) ↔
      Stable M (rule body head (instances.filter keep) :: context) :=
  removal_keeps_candidate_stability M body head instances keep (safe M admitted) context

/-- A list specialized for one candidate cannot be reused at another without
    re-establishing coverage: an omitted condition may become true there. -/
theorem cross_candidate_reuse_changes_truth :
    let instances : List (Instance Unit) := [⟨.atom (), .bot⟩]
    Satisfies Empty (universal (specialize Empty instances)) ∧
      Satisfies Full (universal (specialize Empty instances)) ∧
      ¬ Satisfies Full (universal instances) := by
  classical
  have empty : specialize Empty ([⟨.atom (), .bot⟩] : List (Instance Unit)) = [] := by
    apply List.eq_nil_iff_forall_not_mem.mpr
    intro row present
    have member := List.mem_filter.mp present
    have same := List.mem_singleton.mp member.1
    have active := of_decide_eq_true member.2
    rw [same] at active
    exact active
  simp [empty, universal, all, top, Satisfies, Full]

/-- Even an M-true condition must remain in its implication: J can falsify it.
    Replacing C→H with H after testing C in M changes the frozen query. -/
theorem erasing_true_antecedent_changes_frozen_truth :
    Satisfies (Full : Atoms Unit) (.atom ()) ∧
      Satisfies Empty (Reduct Full (universal [⟨.atom (), .atom ()⟩])) ∧
      ¬ Satisfies Empty (Reduct Full (.atom ())) := by
  classical
  simp [universal, all, top, Reduct, Satisfies, Empty, Full]

end Zetesis.UniversalConditionals
