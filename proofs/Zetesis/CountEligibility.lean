import Zetesis.CountHeads

/-!
# Tuple-count heads with retained eligibility

A complete finite row records a tuple key, a positive head atom and its original
eligibility formula. The existing checked tuple/atom correspondence applies to
row identities. Eligibility may depend recursively on atoms; membership in the
finite row table does not assert its truth. Duplicate witnesses retain all of
their eligibility formulas and coalesce by disjunction.

The key laws establish equality of the key-indexed and atom-indexed eligibility
formulas, then factor each tuple's selected activity into its representative
atom and coalesced eligibility. Original and arbitrary frozen M/J truth agree.
Finite cardinality formulas and enclosing contexts inherit that equivalence.

Complete possible-support traversal, a complete distinct representative table,
source safety and preservation of every eligibility formula are premises. This
module does not prove Rust join completeness, map construction, support closure,
allocation/work accounting, negative head atoms or weighted/extremal heads.
-/

namespace Zetesis.CountEligibility
open Ferraris ChoiceIntervals
universe u v
variable {A : Type u} {K : Type v}

structure Row (K : Type v) (A : Type u) where
  identity : CountHeads.Row K A
  eligible : Formula A

def Correspondence (rows : List (Row K A)) : Prop :=
  CountHeads.Correspondence (rows.map Row.identity)

/-- The existing checked correspondence still applies to rows with conditions. -/
theorem key_iff_atom (rows : List (Row K A)) (valid : Correspondence rows)
    (left right : Row K A) (leftIn : left ∈ rows) (rightIn : right ∈ rows) :
    left.identity.key = right.identity.key ↔ left.identity.atom = right.identity.atom :=
  valid left.identity (List.mem_map.mpr ⟨left, leftIn, rfl⟩)
    right.identity (List.mem_map.mpr ⟨right, rightIn, rfl⟩)

open Classical in
noncomputable def keyed (rows : List (Row K A)) (key : K) : List (Row K A) :=
  rows.filter (fun row => row.identity.key = key)

open Classical in
noncomputable def headed (rows : List (Row K A)) (atom : A) : List (Row K A) :=
  rows.filter (fun row => row.identity.atom = atom)

/-- Both indices retain exactly the same ordered witnesses, including duplicates. -/
theorem indexed_rows_equal (rows : List (Row K A)) (valid : Correspondence rows)
    (representative : Row K A) (member : representative ∈ rows) :
    keyed rows representative.identity.key = headed rows representative.identity.atom := by
  classical
  apply List.filter_congr
  intro row rowIn
  have same := key_iff_atom rows valid row representative rowIn member
  by_cases key : row.identity.key = representative.identity.key
  · simp [key, same.mp key]
  · have different : row.identity.atom ≠ representative.identity.atom :=
      fun atom => key (same.mpr atom)
    simp [key, different]

noncomputable def eligibility (rows : List (Row K A)) (atom : A) : Formula A :=
  RuleFactorization.any ((headed rows atom).map Row.eligible)

noncomputable def tupleActivity (rows : List (Row K A)) (key : K) : Formula A :=
  RuleFactorization.any ((keyed rows key).map
    (fun row => .conj (.atom row.identity.atom) row.eligible))

/-- Original tuple activity requires its atom and at least one eligible witness. -/
theorem activity_original (M : Atoms A) (rows : List (Row K A))
    (valid : Correspondence rows) (representative : Row K A)
    (member : representative ∈ rows) :
    Satisfies M (tupleActivity rows representative.identity.key) ↔
      M representative.identity.atom ∧
        Satisfies M (eligibility rows representative.identity.atom) := by
  classical
  rw [tupleActivity, indexed_rows_equal rows valid representative member]
  simp only [eligibility, RuleFactorization.satisfies_any, List.mem_map]
  constructor
  · rintro ⟨_, ⟨row, rowIn, rfl⟩, atomTrue, eligibleTrue⟩
    have same : row.identity.atom = representative.identity.atom := by
      exact (List.mem_filter.mp rowIn).2 |> of_decide_eq_true
    exact ⟨same ▸ atomTrue, row.eligible, ⟨row, rowIn, rfl⟩, eligibleTrue⟩
  · rintro ⟨atomTrue, _, ⟨row, rowIn, rfl⟩, eligibleTrue⟩
    have same : row.identity.atom = representative.identity.atom := by
      exact (List.mem_filter.mp rowIn).2 |> of_decide_eq_true
    exact ⟨_, ⟨row, rowIn, rfl⟩, same.symm ▸ atomTrue, eligibleTrue⟩

/-- Frozen tuple activity retains the eligibility reduct, including recursion. -/
theorem activity_frozen (M J : Atoms A) (rows : List (Row K A))
    (valid : Correspondence rows) (representative : Row K A)
    (member : representative ∈ rows) :
    Satisfies J (Reduct M (tupleActivity rows representative.identity.key)) ↔
      Satisfies J (Reduct M (.conj (.atom representative.identity.atom)
        (eligibility rows representative.identity.atom))) := by
  classical
  rw [tupleActivity, indexed_rows_equal rows valid representative member]
  simp only [RuleFactorization.reduct_conj, eligibility,
    RuleFactorization.reduct_any, List.mem_map]
  constructor
  · rintro ⟨_, ⟨row, rowIn, rfl⟩, holds⟩
    have selected := (RuleFactorization.reduct_conj M J
      (.atom row.identity.atom) row.eligible).mp holds
    have same : row.identity.atom = representative.identity.atom := by
      exact (List.mem_filter.mp rowIn).2 |> of_decide_eq_true
    exact ⟨same ▸ selected.1, _, ⟨row, rowIn, rfl⟩, selected.2⟩
  · rintro ⟨atomTrue, _, ⟨row, rowIn, rfl⟩, eligibleTrue⟩
    have same : row.identity.atom = representative.identity.atom := by
      exact (List.mem_filter.mp rowIn).2 |> of_decide_eq_true
    exact ⟨_, ⟨row, rowIn, rfl⟩,
      (RuleFactorization.reduct_conj M J _ _).mpr
        ⟨same.symm ▸ atomTrue, eligibleTrue⟩⟩

/-- Factoring tuple activity is valid in both the original and frozen theories. -/
theorem activity_equivalent (rows : List (Row K A)) (valid : Correspondence rows)
    (representative : Row K A) (member : representative ∈ rows) :
    Equivalent (tupleActivity rows representative.identity.key)
      (.conj (.atom representative.identity.atom)
        (eligibility rows representative.identity.atom)) :=
  ⟨fun M => activity_original M rows valid representative member,
    fun M J => activity_frozen M J rows valid representative member⟩

/-- A complete representative carrier can count the factored activities exactly.
    Distinctness and coverage are needed to interpret this list as the source's
    tuple carrier; this algebraic substitution does not manufacture either. -/
theorem threshold_equivalent (needed : Nat) (rows : List (Row K A))
    (valid : Correspondence rows) (representatives : List { row // row ∈ rows }) :
    Equivalent
      (atLeast needed (representatives.map (fun row =>
        tupleActivity rows row.val.identity.key)))
      (atLeast needed (representatives.map (fun row =>
        .conj (.atom row.val.identity.atom) (eligibility rows row.val.identity.atom)))) := by
  apply atLeast_map_equivalent
  intro representative
  exact activity_equivalent rows valid representative.val representative.property

/-- Equality of the witness indices also preserves eligibility in support rules. -/
theorem eligibility_equal (rows : List (Row K A)) (valid : Correspondence rows)
    (representative : Row K A) (member : representative ∈ rows) :
    RuleFactorization.any ((keyed rows representative.identity.key).map Row.eligible) =
      eligibility rows representative.identity.atom := by
  rw [indexed_rows_equal rows valid representative member]
  rfl

end Zetesis.CountEligibility
