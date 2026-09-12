import Zetesis.Propagation

/-!
# Partitioned finite scans for strict-subset propagation

A finite partition covers the scalar atom list by permutation. This premise
preserves occurrence multiplicity; membership coverage alone is insufficient.
Each part counts eligible atoms and takes their maximum index. Summing counts
and taking the maximum of maxima gives the scalar result in any part order.

For a frozen candidate, an atom is eligible when it belongs to the candidate
and its current domain permits false. A zero count excludes every fitting
strict-subset completion. A count of one identifies the atom that every such
completion must make false. The maximum is used as an identity only under that
count premise: zero is a valid atom index, not an absence marker.

The laws use mathematical natural numbers and an immutable domain snapshot.
They do not prove 64-way strided coverage, machine-word bounds, semantic-atom
to-node correspondence, barriers, atomic visibility, resource accounting or
Rust/WGSL execution. Those remain implementation obligations.
-/

namespace Zetesis.PartitionedScan

/-- Maximum of finite indices; zero is the neutral value for an empty list. -/
def maximum : List Nat → Nat
  | [] => 0
  | atom :: rest => max atom (maximum rest)

/-- Scalar number of eligible occurrences. -/
def count (eligible : Nat → Bool) (atoms : List Nat) : Nat :=
  (atoms.filter eligible).length

/-- Scalar maximum; only a positive count establishes a present witness. -/
def index (eligible : Nat → Bool) (atoms : List Nat) : Nat :=
  maximum (atoms.filter eligible)

/-- Counts add across all finite parts, including empty parts. -/
def partitionCount (eligible : Nat → Bool) (parts : List (List Nat)) : Nat :=
  (parts.map (count eligible)).sum

/-- Empty-part maxima contribute only the neutral value. -/
def partitionIndex (eligible : Nat → Bool) (parts : List (List Nat)) : Nat :=
  maximum (parts.map (index eligible))

/-- Concatenation combines the two maxima without inspecting their order. -/
theorem maximum_append (left right : List Nat) :
    maximum (left ++ right) = max (maximum left) (maximum right) := by
  induction left with
  | nil => simp [maximum]
  | cons atom rest tail => simp [maximum, tail, Nat.max_assoc]

/-- Reordering a list preserves its maximum, including repeated indices. -/
theorem maximum_permutation {left right : List Nat} (same : left.Perm right) :
    maximum left = maximum right := by
  induction same with
  | nil => rfl
  | cons atom same tail => simp [maximum, tail]
  | swap left right rest => simp [maximum, Nat.max_left_comm]
  | trans first second left_equal right_equal => exact left_equal.trans right_equal

/-- Summing the part counts counts exactly the flattened occurrences. -/
theorem partition_count_flatten (eligible : Nat → Bool) (parts : List (List Nat)) :
    partitionCount eligible parts = count eligible parts.flatten := by
  induction parts with
  | nil => simp [partitionCount, count]
  | cons part rest tail =>
    simpa [partitionCount, count, List.filter_append] using
      congrArg (fun total => count eligible part + total) tail

/-- Taking the maximum of part maxima gives the flattened maximum. -/
theorem partition_index_flatten (eligible : Nat → Bool) (parts : List (List Nat)) :
    partitionIndex eligible parts = index eligible parts.flatten := by
  induction parts with
  | nil => simp [partitionIndex, index, maximum]
  | cons part rest tail =>
    simpa [partitionIndex, index, maximum, List.filter_append, maximum_append] using
      congrArg (fun other => max (index eligible part) other) tail

/-- Exact partition coverage preserves the scalar eligible count. Filtering
preserves the coverage permutation, and permutation preserves length. -/
theorem partition_count_exact (eligible : Nat → Bool) (parts : List (List Nat))
    (atoms : List Nat) (coverage : parts.flatten.Perm atoms) :
    partitionCount eligible parts = count eligible atoms := by
  have selected : (parts.flatten.filter eligible).Perm (atoms.filter eligible) :=
    coverage.filter eligible
  rw [partition_count_flatten]
  exact selected.length_eq

/-- Exact partition coverage also preserves the scalar maximum. -/
theorem partition_index_exact (eligible : Nat → Bool) (parts : List (List Nat))
    (atoms : List Nat) (coverage : parts.flatten.Perm atoms) :
    partitionIndex eligible parts = index eligible atoms := by
  rw [partition_index_flatten]
  exact maximum_permutation (coverage.filter eligible)

/-- A zero merged count means that no covered occurrence is eligible. -/
theorem zero_excludes_eligible (eligible : Nat → Bool) (parts : List (List Nat))
    (atoms : List Nat) (coverage : parts.flatten.Perm atoms)
    (empty : partitionCount eligible parts = 0) :
    ∀ atom, atom ∈ atoms → eligible atom = false := by
  have absent : atoms.filter eligible = [] := by
    apply List.length_eq_zero_iff.mp
    simpa only [partition_count_exact eligible parts atoms coverage, count] using empty
  intro atom member
  cases enabled : eligible atom with
  | false => rfl
  | true =>
    have selected : atom ∈ atoms.filter eligible := List.mem_filter.mpr ⟨member, enabled⟩
    rw [absent] at selected
    exact False.elim (List.not_mem_nil selected)

/-- A merged count of one names the sole eligible occurrence. The exact
singleton statement supplies both presence and uniqueness, even at index zero. -/
theorem one_identifies_singleton (eligible : Nat → Bool) (parts : List (List Nat))
    (atoms : List Nat) (coverage : parts.flatten.Perm atoms)
    (one : partitionCount eligible parts = 1) :
    atoms.filter eligible = [partitionIndex eligible parts] := by
  have length_one : (atoms.filter eligible).length = 1 := by
    simpa only [partition_count_exact eligible parts atoms coverage, count] using one
  obtain ⟨atom, singleton⟩ := List.length_eq_one_iff.mp length_one
  have identified : partitionIndex eligible parts = atom := by
    rw [partition_index_exact eligible parts atoms coverage]
    simp [index, singleton, maximum]
  simpa only [identified] using singleton

/-- Availability is a frozen-candidate atom whose domain still permits false. -/
def available (candidate : Propagation.Valuation Nat) (domains : Propagation.Domains Nat)
    (atom : Nat) : Prop := candidate atom = true ∧ domains atom false

/-- With complete candidate coverage, zero availability excludes a strict
drop: any dropped atom would itself be an eligible covered occurrence. -/
theorem zero_excludes_strict_drop (candidate tested : Propagation.Valuation Nat)
    (domains : Propagation.Domains Nat) (eligible : Nat → Bool)
    (classification : ∀ atom, eligible atom = true ↔ available candidate domains atom)
    (parts : List (List Nat)) (atoms : List Nat) (coverage : parts.flatten.Perm atoms)
    (candidate_coverage : ∀ atom, candidate atom = true → atom ∈ atoms)
    (empty : partitionCount eligible parts = 0) (fits : Propagation.Fits domains tested) :
    ¬ Propagation.StrictDrop candidate tested := by
  rintro ⟨atom, inside, dropped⟩
  have false_available : domains atom false := by simpa only [dropped] using fits atom
  have enabled : eligible atom = true :=
    (classification atom).mpr ⟨inside, false_available⟩
  have disabled : eligible atom = false :=
    zero_excludes_eligible eligible parts atoms coverage empty atom
      (candidate_coverage atom inside)
  rw [disabled] at enabled
  cases enabled

/-- With one available atom, every fitting strict drop makes that atom false.
Every drop witness is eligible; the singleton law identifies it with the merged
index. This justifies intersecting that domain with false, not accepting a model. -/
theorem one_forces_strict_drop (candidate tested : Propagation.Valuation Nat)
    (domains : Propagation.Domains Nat) (eligible : Nat → Bool)
    (classification : ∀ atom, eligible atom = true ↔ available candidate domains atom)
    (parts : List (List Nat)) (atoms : List Nat) (coverage : parts.flatten.Perm atoms)
    (candidate_coverage : ∀ atom, candidate atom = true → atom ∈ atoms)
    (one : partitionCount eligible parts = 1) (fits : Propagation.Fits domains tested)
    (strict : Propagation.StrictDrop candidate tested) :
    tested (partitionIndex eligible parts) = false := by
  obtain ⟨atom, inside, dropped⟩ := strict
  have false_available : domains atom false := by simpa only [dropped] using fits atom
  have selected : atom ∈ atoms.filter eligible :=
    List.mem_filter.mpr ⟨candidate_coverage atom inside,
      (classification atom).mpr ⟨inside, false_available⟩⟩
  rw [one_identifies_singleton eligible parts atoms coverage one] at selected
  have identity : atom = partitionIndex eligible parts := List.mem_singleton.mp selected
  simpa only [← identity] using dropped

end Zetesis.PartitionedScan
