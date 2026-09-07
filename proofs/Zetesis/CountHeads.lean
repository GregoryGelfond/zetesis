import Zetesis.ChoiceIntervals
import Zetesis.NegativeHeads

/-!
# Checked finite count-head correspondence

The completed static-eligibility table retains each full tuple key and its
positive derived atom. A checked correspondence rejects either alias direction;
identical duplicate rows may coalesce. A complete representative table has one
position per tuple. The correspondence makes its atom carrier duplicate-free.

Each tuple is active when some row with that key has a true atom. The checked
correspondence identifies that disjunction with the representative atom, in
original truth and every frozen reduct. Replacing tuple activity by distinct
atom activity therefore preserves the whole choice/constraint group in context.
Cardinality bounds are frozen candidate constraints and supply no support.

Complete finite eligibility, representative-table coverage and static source
safety are premises. The module does not prove the Rust BTreeMaps, cursor,
resource accounting, source parser, arbitrary dynamic function-head semantics,
negative derived literals, objective presence or functions other than count.
-/

namespace Zetesis.CountHeads

open Ferraris
open ChoiceIntervals (Equivalent equivalent_refl equivalent_conj equivalent_imp
  all atLeast top group count)

universe u v
variable {α : Type u} {κ : Type v}

structure Row (κ : Type v) (α : Type u) where
  key : κ
  atom : α
  deriving DecidableEq

def Correspondence (rows : List (Row κ α)) : Prop :=
  ∀ left ∈ rows, ∀ right ∈ rows, left.key = right.key ↔ left.atom = right.atom

def check [DecidableEq κ] [DecidableEq α] (rows : List (Row κ α)) : Bool :=
  rows.all (fun left => rows.all (fun right =>
    decide ((left.key = right.key) ↔ (left.atom = right.atom))))

theorem check_exact [DecidableEq κ] [DecidableEq α] (rows : List (Row κ α)) :
    check rows = true ↔ Correspondence rows := by
  simp [check, Correspondence]

abbrev Member (rows : List (Row κ α)) := { row : Row κ α // row ∈ rows }

/-- One representative per complete tuple, covering every completed raw row. -/
structure Table (rows : List (Row κ α)) where
  representatives : List (Member rows)
  distinctKeys : (representatives.map (fun row => row.val.key)).Nodup
  covers : ∀ row ∈ rows, ∃ representative ∈ representatives,
    representative.val.key = row.key

theorem distinct_atoms_of_distinct_keys (rows : List (Row κ α))
    (valid : Correspondence rows) (representatives : List (Member rows))
    (keys : (representatives.map (fun row => row.val.key)).Nodup) :
    (representatives.map (fun row => row.val.atom)).Nodup := by
  induction representatives with
  | nil => simp
  | cons representative rest ih =>
    simp only [List.map_cons, List.nodup_cons] at keys ⊢
    constructor
    · intro member
      obtain ⟨other, inside, same⟩ := List.mem_map.mp member
      exact keys.1 (List.mem_map.mpr ⟨other, inside,
        (valid other.val other.property representative.val representative.property).mpr same⟩)
    · exact ih keys.2

theorem table_atoms_nodup (rows : List (Row κ α)) (valid : Correspondence rows)
    (table : Table rows) :
    (table.representatives.map (fun row => row.val.atom)).Nodup :=
  distinct_atoms_of_distinct_keys rows valid table.representatives table.distinctKeys

theorem table_covers_atoms (rows : List (Row κ α)) (valid : Correspondence rows)
    (table : Table rows) (row : Row κ α) (member : row ∈ rows) :
    ∃ representative ∈ table.representatives, representative.val.atom = row.atom := by
  obtain ⟨representative, inside, same⟩ := table.covers row member
  exact ⟨representative, inside,
    (valid representative.val representative.property row member).mp same⟩

open Classical in
noncomputable def tupleActivity (rows : List (Row κ α)) (key : κ) : Formula α :=
  RuleFactorization.any ((rows.filter (fun row => row.key = key)).map
    (fun row => .atom row.atom))

theorem activity_original (M : Atoms α) (rows : List (Row κ α))
    (valid : Correspondence rows) (representative : Member rows) :
    Satisfies M (tupleActivity rows representative.val.key) ↔ M representative.val.atom := by
  classical
  simp only [tupleActivity, RuleFactorization.satisfies_any, List.mem_map,
    List.mem_filter, decide_eq_true_eq]
  constructor
  · rintro ⟨_, ⟨row, ⟨member, same⟩, rfl⟩, holds⟩
    have equal := (valid row member representative.val representative.property).mp same
    simpa [equal, Satisfies] using holds
  · intro holds
    exact ⟨.atom representative.val.atom,
      ⟨representative.val, ⟨representative.property, rfl⟩, rfl⟩, holds⟩

theorem activity_frozen (M J : Atoms α) (rows : List (Row κ α))
    (valid : Correspondence rows) (representative : Member rows) :
    Satisfies J (Reduct M (tupleActivity rows representative.val.key)) ↔
      Satisfies J (Reduct M (.atom representative.val.atom)) := by
  classical
  simp only [tupleActivity, RuleFactorization.reduct_any, List.mem_map,
    List.mem_filter, decide_eq_true_eq]
  constructor
  · rintro ⟨_, ⟨row, ⟨member, same⟩, rfl⟩, holds⟩
    have equal := (valid row member representative.val representative.property).mp same
    simpa [equal] using holds
  · intro holds
    exact ⟨.atom representative.val.atom,
      ⟨representative.val, ⟨representative.property, rfl⟩, rfl⟩, holds⟩

noncomputable def sourceGroup (rows : List (Row κ α)) (table : Table rows)
    (body : Formula α) (lower upper : Nat) : Formula α :=
  .imp body (.conj
    (all (table.representatives.map (fun row =>
      .imp top (.disj (.atom row.val.atom) (Neg (.atom row.val.atom))))))
    (.conj
      (Neg (Neg (atLeast lower (table.representatives.map
        (fun row => tupleActivity rows row.val.key)))))
      (Neg (atLeast (upper + 1) (table.representatives.map
        (fun row => tupleActivity rows row.val.key))))))

def compiledGroup (rows : List (Row κ α)) (table : Table rows)
    (body : Formula α) (lower upper : Nat) : Formula α :=
  group body lower upper (table.representatives.map (fun row => row.val.atom)) (fun _ => top)

theorem group_equivalent (rows : List (Row κ α)) (valid : Correspondence rows)
    (table : Table rows) (body : Formula α) (lower upper : Nat) :
    Equivalent (sourceGroup rows table body lower upper)
      (compiledGroup rows table body lower upper) := by
  have selected (row : Member rows) :
      Equivalent (tupleActivity rows row.val.key) (.conj (.atom row.val.atom) top) := by
    constructor
    · intro M
      simpa [Satisfies, top] using activity_original M rows valid row
    · intro M J
      rw [RuleFactorization.reduct_conj]
      have true_top : Satisfies J (Reduct M (top : Formula α)) := by
        simp [top, Reduct, Satisfies]
      simpa only [true_top, and_true] using activity_frozen M J rows valid row
  simp only [sourceGroup, compiledGroup, group, List.map_map, Function.comp_def]
  apply equivalent_imp _ _ _ _ (equivalent_refl _)
  apply equivalent_conj
  · exact equivalent_refl _
  · apply equivalent_conj
    · exact equivalent_imp _ _ _ _
        (equivalent_imp _ _ _ _
          (ChoiceIntervals.atLeast_map_equivalent lower table.representatives _ _ selected)
          (equivalent_refl _)) (equivalent_refl _)
    · exact equivalent_imp _ _ _ _
        (ChoiceIntervals.atLeast_map_equivalent (upper + 1) table.representatives _ _ selected)
        (equivalent_refl _)

theorem stable_in_context (M : Atoms α) (rows : List (Row κ α))
    (valid : Correspondence rows) (table : Table rows) (body : Formula α)
    (lower upper : Nat) (context : Theory α) :
    Stable M (sourceGroup rows table body lower upper :: context) ↔
      Stable M (compiledGroup rows table body lower upper :: context) := by
  have original := (group_equivalent rows valid table body lower upper).1
  have frozen := (group_equivalent rows valid table body lower upper).2
  simp only [Stable, models_cons, ReductTheory, List.map_cons, original, frozen]

/-- Bounds inspect M's tuple count; they cannot force any atom into J. -/
theorem bounds_are_candidate_constraints (M J : Atoms α) (rows : List (Row κ α))
    (table : Table rows) (lower upper : Nat) :
    Satisfies J (Reduct M (.conj
      (Neg (Neg (atLeast lower (table.representatives.map
        (fun row => tupleActivity rows row.val.key)))))
      (Neg (atLeast (upper + 1) (table.representatives.map
        (fun row => tupleActivity rows row.val.key)))))) ↔
      lower ≤ count (Satisfies M) (table.representatives.map
        (fun row => tupleActivity rows row.val.key)) ∧
      count (Satisfies M) (table.representatives.map
        (fun row => tupleActivity rows row.val.key)) ≤ upper :=
  ChoiceIntervals.cardinality_constraints_frozen M J lower upper _

theorem alias_directions_are_rejected :
    check ([⟨false, false⟩, ⟨false, true⟩] : List (Row Bool Bool)) = false ∧
      check ([⟨false, false⟩, ⟨true, false⟩] : List (Row Bool Bool)) = false := by
  decide

theorem identical_duplicates_are_compatible :
    check ([⟨false, true⟩, ⟨false, true⟩] : List (Row Bool Bool)) = true := by
  decide

end Zetesis.CountHeads
