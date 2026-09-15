import Zetesis.ConditionalHeads
import Zetesis.BooleanHeads

/-!
# Necessary positive support for conditional disjuncts

Each finite head occurrence has its own condition and a signed atom or Boolean
literal. Removing an atom with no active positive occurrence preserves every
rule's reduct, contradicting stability. Both the outer body and local condition
belong to its producer permission. Conditions do not become producers, and
default-negated or Boolean heads do not produce atoms.

This theorem concerns a completed ground theory. Establishing complete possible
support, local substitutions, source safety and Rust resource/refusal behavior is
not part of the proof. An unconditional occurrence uses the true condition.
-/

namespace Zetesis.ConditionalHeadSupport

open Ferraris
universe u
variable {A : Type u}

structure Occurrence (A : Type u) where
  condition : Formula A
  literal : BooleanHeads.Literal A

def toInstance (row : Occurrence A) : UniversalConditionals.Instance A :=
  ⟨row.condition, BooleanHeads.literal row.literal⟩

def head (rows : List (Occurrence A)) : Formula A :=
  ConditionalHeads.head (rows.map toInstance)

structure Rule (A : Type u) where
  body : Formula A
  rows : List (Occurrence A)

def formula (rule : Rule A) : Formula A := .imp rule.body (head rule.rows)
def theory (rules : List (Rule A)) : Theory A := rules.map formula

def HasProducer (M : Atoms A) (rules : List (Rule A)) (a : A) : Prop :=
  ∃ rule ∈ rules, ∃ row ∈ rule.rows,
    row.literal = .atom (.positive a) ∧
      Satisfies M rule.body ∧ Satisfies M row.condition

/-- Keep every true positive head whose local condition holds. The witnessing
    conditional instance then remains true even if its condition changes in J. -/
theorem head_reduct_of_positive_kept (M J : Atoms A) (rows : List (Occurrence A))
    (kept : ∀ row ∈ rows, ∀ a, row.literal = .atom (.positive a) →
      Satisfies M row.condition → M a → J a)
    (holds : Satisfies M (head rows)) :
    Satisfies J (Reduct M (head rows)) := by
  obtain ⟨mapped, member, active, original⟩ :=
    (ConditionalHeads.head_original M (rows.map toInstance)).mp holds
  obtain ⟨row, inside, rfl⟩ := List.mem_map.mp member
  have literal_frozen : Satisfies J (Reduct M (BooleanHeads.literal row.literal)) := by
    have singleton : Satisfies M (BooleanHeads.head [row.literal]) := by
      simpa [toInstance, BooleanHeads.head, RuleFactorization.any, Satisfies] using original
    have preserved := BooleanHeads.head_reduct_of_positive_kept M J [row.literal]
      (fun a member present => kept row inside a (List.mem_singleton.mp member).symm
        active present) singleton
    change Satisfies J (Reduct M (.disj (BooleanHeads.literal row.literal) .bot)) at preserved
    rw [RuleFactorization.reduct_disj] at preserved
    exact preserved.elim id (fun impossible => False.elim impossible)
  apply (ConditionalHeads.head_frozen M J (rows.map toInstance)).mpr
  exact ⟨toInstance row, List.mem_map.mpr ⟨row, inside, rfl⟩,
    ⟨active, original⟩, fun _ => literal_frozen⟩

/-- Every atom in a stable model has an original positive occurrence whose
    outer body and local eligibility both hold. No disjunctive shifting occurs.

The proof removes an allegedly unsupported atom. Original truth selects a
conditional occurrence in each active rule. Its positive head cannot be the
removed atom; negative and Boolean heads keep their frozen truth. Hence the
proper subset still models every reduct rule, contrary to stability.
-/
theorem stable_has_positive_producer (M : Atoms A) (rules : List (Rule A))
    (stable : Stable M (theory rules)) (a : A) (present : M a) :
    HasProducer M rules a := by
  classical
  apply Classical.byContradiction
  intro unsupported
  let J : Atoms A := fun b => M b ∧ b ≠ a
  have proper : ProperSub J M := by
    refine ⟨fun _ member => member.1, ?_⟩
    intro back
    exact (back a present).2 rfl
  apply stable.2
  refine ⟨J, proper, ?_⟩
  intro reduced member
  obtain ⟨original, inside, rfl⟩ := List.mem_map.mp member
  obtain ⟨rule, included, rfl⟩ := List.mem_map.mp inside
  have holds : Satisfies M (formula rule) :=
    stable.1 _ (List.mem_map.mpr ⟨rule, included, rfl⟩)
  rw [formula, RuleFactorization.reduct_imp]
  refine ⟨holds, ?_⟩
  intro body_frozen
  have body_original : Satisfies M rule.body :=
    NegativeHeads.reduct_truth_requires_original M J rule.body body_frozen
  apply head_reduct_of_positive_kept M J rule.rows
  · intro row inside b positive active member
    refine ⟨member, ?_⟩
    intro equal
    subst b
    exact unsupported ⟨rule, included, row, inside, positive, body_original, active⟩
  · exact holds body_original

end Zetesis.ConditionalHeadSupport
