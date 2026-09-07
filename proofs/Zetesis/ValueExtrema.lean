import Zetesis.GroundGuards

/-! Complete-value extrema: finite candidate coverage, ordered-selection witness
laws, and original/frozen formula laws for every min/max guard connective.

The carrier is arbitrary. The comparison function must separately satisfy the
supplied ordered-selection predicate law; this does not axiomatize or prove the
Rust ASP comparator. Keys, complete support, source safety, cache identity,
numeric endpoint policy, budgets and Rust refinement remain adapter obligations.
Witness predicates are fixed before a candidate is frozen. Eligibility formulas
remain arbitrary, including recursive, default-negated and double-negated ones.
-/
namespace Zetesis.ValueExtrema

universe u v
variable {κ : Type u} {α : Type v}
open Ferraris
open RuleFactorization (any)

def choose (before : κ → κ → Bool) (left right : κ) : κ :=
  if before left right then left else right

def extreme (before : κ → κ → Bool) : List κ → Option κ
  | [] => none
  | head :: tail => match extreme before tail with
    | none => some head
    | some best => some (choose before head best)

def encode (empty : κ) : Option κ → κ
  | none => empty
  | some value => value

def candidates (empty : κ) (possible : List κ) : List κ := empty :: possible

theorem choose_is_input (before : κ → κ → Bool) (left right : κ) :
    choose before left right = left ∨ choose before left right = right := by
  simp only [choose]
  split <;> simp

theorem extreme_none_iff (before : κ → κ → Bool) (values : List κ) :
    extreme before values = none ↔ values = [] := by
  cases values with
  | nil => simp [extreme]
  | cons head tail =>
    simp only [extreme, List.cons_ne_nil, iff_false]
    cases extreme before tail <;> simp

theorem extreme_some_mem (before : κ → κ → Bool) (values : List κ) (value : κ)
    (result : extreme before values = some value) : value ∈ values := by
  induction values generalizing value with
  | nil => simp [extreme] at result
  | cons head tail ih =>
    cases ht : extreme before tail with
    | none => exact List.mem_cons.mpr (Or.inl (by simpa [extreme, ht] using result.symm))
    | some best =>
      have same : choose before head best = value := by simpa [extreme, ht] using result
      rcases choose_is_input before head best with left | right
      · exact List.mem_cons.mpr (Or.inl (same.symm.trans left))
      · exact List.mem_cons.mpr (Or.inr (by
          have member : best ∈ tail := ih best ht
          simpa [same.symm.trans right] using member))

theorem completed_candidate_coverage (before : κ → κ → Bool) (empty : κ)
    (actual possible : List κ) (coverage : ∀ value ∈ actual, value ∈ possible) :
    encode empty (extreme before actual) ∈ candidates empty possible := by
  cases result : extreme before actual with
  | none => simp [encode, candidates]
  | some value =>
    exact List.mem_cons.mpr (Or.inr (coverage value (extreme_some_mem before actual value result)))

theorem empty_result (before : κ → κ → Bool) (empty : κ) :
    encode empty (extreme before []) = empty := rfl

theorem candidate_length (empty : κ) (possible : List κ) :
    (candidates empty possible).length = possible.length + 1 := by simp [candidates]

/-- The same selection law covers min <=/< and max >=/> bound witnesses.
The empty sentinel case is stated separately instead of using a numeric proxy. -/
theorem selected_predicate (before : κ → κ → Bool) (P : κ → Prop)
    (law : ∀ left right, P (choose before left right) ↔ P left ∨ P right)
    (values : List κ) (value : κ) (result : extreme before values = some value) :
    P value ↔ ∃ member ∈ values, P member := by
  induction values generalizing value with
  | nil => simp [extreme] at result
  | cons head tail ih =>
    cases ht : extreme before tail with
    | none =>
      have empty := (extreme_none_iff before tail).mp ht
      subst tail
      simp [extreme] at result
      subst value
      simp
    | some best =>
      have same : choose before head best = value := by simpa [extreme, ht] using result
      rw [← same, law, ih best ht]
      simp

theorem selected_or_empty_predicate (before : κ → κ → Bool) (empty : κ) (P : κ → Prop)
    (law : ∀ left right, P (choose before left right) ↔ P left ∨ P right)
    (values : List κ) :
    P (encode empty (extreme before values)) ↔
      (values = [] ∧ P empty) ∨ ∃ value ∈ values, P value := by
  cases result : extreme before values with
  | none =>
    have emptyValues := (extreme_none_iff before values).mp result
    subst values
    simp [encode]
  | some value =>
    have nonempty : values ≠ [] := by intro h; subst values; simp [extreme] at result
    simp only [encode, nonempty, false_and, false_or]
    exact selected_predicate before P law values value result

structure Element (κ : Type u) (α : Type v) where
  key : κ
  condition : Formula α

/-- A key predicate can inspect the complete first value via a separately
supplied key-to-value map. Repeated raw keys preserve their condition OR. -/
def witness (P : κ → Bool) (rows : List (Element κ α)) : Formula α :=
  any ((rows.filter (fun row => P row.key)).map Element.condition)

theorem witness_original (M : Atoms α) (P : κ → Bool) (rows : List (Element κ α)) :
    Satisfies M (witness P rows) ↔
      ∃ row ∈ rows, P row.key = true ∧ Satisfies M row.condition := by
  simp only [witness, RuleFactorization.satisfies_any, List.mem_map, List.mem_filter]
  constructor
  · rintro ⟨condition, ⟨row, ⟨member, selected⟩, rfl⟩, truth⟩
    exact ⟨row, member, selected, truth⟩
  · rintro ⟨row, member, selected, truth⟩
    exact ⟨row.condition, ⟨row, ⟨member, selected⟩, rfl⟩, truth⟩

theorem witness_frozen (M J : Atoms α) (P : κ → Bool) (rows : List (Element κ α)) :
    Satisfies J (Reduct M (witness P rows)) ↔
      ∃ row ∈ rows, P row.key = true ∧ Satisfies J (Reduct M row.condition) := by
  simp only [witness, RuleFactorization.reduct_any, List.mem_map, List.mem_filter]
  constructor
  · rintro ⟨condition, ⟨row, ⟨member, selected⟩, rfl⟩, truth⟩
    exact ⟨row, member, selected, truth⟩
  · rintro ⟨row, member, selected, truth⟩
    exact ⟨row.condition, ⟨row, ⟨member, selected⟩, rfl⟩, truth⟩

def withEmpty (emptyWitness : Bool) (P : κ → Bool) (rows : List (Element κ α)) : Formula α :=
  .disj (GroundGuards.constant emptyWitness) (witness P rows)

theorem empty_witness_original (M : Atoms α) (emptyWitness : Bool)
    (P : κ → Bool) (rows : List (Element κ α)) :
    Satisfies M (withEmpty emptyWitness P rows) ↔
      emptyWitness = true ∨ ∃ row ∈ rows, P row.key = true ∧ Satisfies M row.condition := by
  simp only [withEmpty, Satisfies, GroundGuards.constant_original, witness_original]

theorem empty_witness_frozen (M J : Atoms α) (emptyWitness : Bool)
    (P : κ → Bool) (rows : List (Element κ α)) :
    Satisfies J (Reduct M (withEmpty emptyWitness P rows)) ↔
      emptyWitness = true ∨ ∃ row ∈ rows, P row.key = true ∧
        Satisfies J (Reduct M row.condition) := by
  simp only [withEmpty, RuleFactorization.reduct_disj,
    GroundGuards.constant_frozen, witness_frozen]

/-- Coalesce by complete key. Sharing only a first value is not this operation. -/
def coalesced [DecidableEq κ] (keys : List κ) (rows : List (Element κ α)) : List (Element κ α) :=
  keys.map (fun key => ⟨key, witness (fun other => decide (other = key)) rows⟩)

theorem coalesced_witness_original [DecidableEq κ] (M : Atoms α) (P : κ → Bool)
    (keys : List κ) (rows : List (Element κ α))
    (coverage : ∀ row ∈ rows, row.key ∈ keys) :
    Satisfies M (witness P (coalesced keys rows)) ↔ Satisfies M (witness P rows) := by
  rw [witness_original, witness_original]
  constructor
  · rintro ⟨row, member, selected, truth⟩
    obtain ⟨key, _, same⟩ := List.mem_map.mp member
    subst row
    obtain ⟨original, member, same, truth⟩ :=
      (witness_original M (fun other => decide (other = key)) rows).mp truth
    simp only [decide_eq_true_eq] at same
    exact ⟨original, member, by simpa only [same] using selected, truth⟩
  · rintro ⟨row, member, selected, truth⟩
    refine ⟨⟨row.key, witness (fun other => decide (other = row.key)) rows⟩,
      List.mem_map.mpr ⟨row.key, coverage row member, rfl⟩, selected, ?_⟩
    exact (witness_original M (fun other => decide (other = row.key)) rows).mpr
      ⟨row, member, by simp, truth⟩

theorem coalesced_witness_frozen [DecidableEq κ] (M J : Atoms α) (P : κ → Bool)
    (keys : List κ) (rows : List (Element κ α))
    (coverage : ∀ row ∈ rows, row.key ∈ keys) :
    Satisfies J (Reduct M (witness P (coalesced keys rows))) ↔
      Satisfies J (Reduct M (witness P rows)) := by
  rw [witness_frozen, witness_frozen]
  constructor
  · rintro ⟨row, member, selected, truth⟩
    obtain ⟨key, _, same⟩ := List.mem_map.mp member
    subst row
    obtain ⟨original, member, same, truth⟩ :=
      (witness_frozen M J (fun other => decide (other = key)) rows).mp truth
    simp only [decide_eq_true_eq] at same
    exact ⟨original, member, by simpa only [same] using selected, truth⟩
  · rintro ⟨row, member, selected, truth⟩
    refine ⟨⟨row.key, witness (fun other => decide (other = row.key)) rows⟩,
      List.mem_map.mpr ⟨row.key, coverage row member, rfl⟩, selected, ?_⟩
    exact (witness_frozen M J (fun other => decide (other = row.key)) rows).mpr
      ⟨row, member, by simp, truth⟩

inductive Guard where
  | inclusive | strict | notInclusive | notStrict | equal | unequal

def accepted : Guard → Prop → Prop → Prop
  | .inclusive, inclusive, _ => inclusive
  | .strict, _, strict => strict
  | .notInclusive, inclusive, _ => ¬ inclusive
  | .notStrict, _, strict => ¬ strict
  | .equal, inclusive, strict => inclusive ∧ ¬ strict
  | .unequal, inclusive, strict => inclusive → strict

def guard : Guard → Formula α → Formula α → Formula α
  | .inclusive, inclusive, _ => inclusive
  | .strict, _, strict => strict
  | .notInclusive, inclusive, _ => Neg inclusive
  | .notStrict, _, strict => Neg strict
  | .equal, inclusive, strict => .conj inclusive (Neg strict)
  | .unequal, inclusive, strict => .imp inclusive strict

theorem guard_original (M : Atoms α) (kind : Guard) (inclusive strict : Formula α) :
    Satisfies M (guard kind inclusive strict) ↔
      accepted kind (Satisfies M inclusive) (Satisfies M strict) := by
  cases kind <;> rfl

theorem frozen_requires_original (M J : Atoms α) (F : Formula α)
    (frozen : Satisfies J (Reduct M F)) : Satisfies M F := by
  classical
  by_cases original : Satisfies M F
  · exact original
  · rw [RuleFactorization.false_reduct M F original] at frozen
    exact False.elim frozen

/-- Every frozen world is covered, without restricting J to a subset of M.
For max, inclusive/strict mean >=/>; for min they mean <=/<. The opposite
relations and != retain implication connectives, not classical rewrites. -/
theorem guard_frozen (M J : Atoms α) (kind : Guard) (inclusive strict : Formula α) :
    Satisfies J (Reduct M (guard kind inclusive strict)) ↔
      accepted kind (Satisfies M inclusive) (Satisfies M strict) ∧
      accepted kind (Satisfies J (Reduct M inclusive)) (Satisfies J (Reduct M strict)) := by
  classical
  have incl := frozen_requires_original M J inclusive
  have str := frozen_requires_original M J strict
  by_cases a : Satisfies M inclusive <;> by_cases b : Satisfies M strict <;>
    by_cases c : Satisfies J (Reduct M inclusive) <;>
    by_cases d : Satisfies J (Reduct M strict) <;> cases kind <;>
    simp_all [guard, accepted, Ferraris.Neg, Satisfies, Reduct]

/-- Replacing witness families by formulas with identical original and frozen
meaning is valid under every surrounding formula, including not and not not. -/
theorem guard_congruence (kind : Guard) (left right left' right' : Formula α)
    (inclusive : ChoiceIntervals.Equivalent left left')
    (strict : ChoiceIntervals.Equivalent right right') :
    ChoiceIntervals.Equivalent (guard kind left right) (guard kind left' right') := by
  constructor
  · intro M
    simp only [guard_original, inclusive.1 M, strict.1 M]
  · intro M J
    simp only [guard_frozen, inclusive.1 M, strict.1 M, inclusive.2 M J, strict.2 M J]

end Zetesis.ValueExtrema
