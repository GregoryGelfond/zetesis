import Std

/-!
# Finite aggregate-assignment candidate coverage

FullTuple is the complete ordered scalar tuple, not merely its weight. Raw
supports with equal tuples are coalesced by OR eligibility; each tuple then
contributes once. Weight is a function of that complete key, so equal keys
cannot carry conflicting weights. The source adapter separately establishes
that #sum uses an admitted numeric first component and #count uses one.

The candidate generator below is a total, finite denotational algorithm over
unbounded Int. It has no runtime work, memory, cancellation, numeric-width or
recursive-support completion guarantee. Only a completed covering carrier can
supply the final theorem's coverage hypothesis. A stopped enumeration cannot
claim that its current values exhaust the candidates.
-/

namespace Zetesis.AggregateAssignment

universe u
variable {κ : Type u}

/-- The scalar constructors remain distinct in complete tuple identity. -/
inductive Scalar where
  | number : Int → Scalar
  | symbol : String → Scalar
  | text : String → Scalar
  deriving DecidableEq

abbrev FullTuple := List Scalar

/-- One ground alternative supporting a complete tuple in a tested interpretation. -/
structure Element where
  tuple : FullTuple
  eligible : Bool

/-- Stable finite deduplication retaining the last occurrence of each key.
    Ordering is an enumeration choice, not part of aggregate-set semantics. -/
def unique [DecidableEq κ] : List κ → List κ
  | [] => []
  | key :: rest =>
      let tail := unique rest
      if key ∈ tail then tail else key :: tail

theorem mem_unique [DecidableEq κ] (key : κ) (keys : List κ) :
    key ∈ unique keys ↔ key ∈ keys := by
  induction keys generalizing key with
  | nil => simp [unique]
  | cons head rest ih =>
    by_cases present : head ∈ unique rest
    · have inside : head ∈ rest := (ih head).mp present
      simp only [unique, present, ↓reduceIte, ih, List.mem_cons]
      constructor
      · exact Or.inr
      · rintro (same | member)
        · simpa [same] using inside
        · exact member
    · simp [unique, present, ih]

theorem unique_nodup [DecidableEq κ] (keys : List κ) : (unique keys).Nodup := by
  induction keys with
  | nil => simp [unique]
  | cons head rest ih =>
    by_cases present : head ∈ unique rest <;> simp [unique, present, ih]

/-- Deduplication cannot increase a finite carrier's length. -/
theorem unique_length_le [DecidableEq κ] (keys : List κ) :
    (unique keys).length ≤ keys.length := by
  induction keys with
  | nil => simp [unique]
  | cons head rest ih =>
    by_cases present : head ∈ unique rest <;> simp [unique, present] <;> omega

/-- Integer values from all subsets, with numerical duplicates removed each row. -/
def subsetSums : List Int → List Int
  | [] => [0]
  | weight :: rest =>
      let previous := subsetSums rest
      unique (previous ++ previous.map (fun value => weight + value))

def selectedSum (weights : List Int) (selected : Nat → Bool) : Int :=
  match weights with
  | [] => 0
  | weight :: rest =>
      (if selected 0 then weight else 0) + selectedSum rest (fun index => selected (index + 1))

/-- A direct include/exclude interpretation always appears in the finite generator. -/
theorem selected_sum_is_candidate (weights : List Int) (selected : Nat → Bool) :
    selectedSum weights selected ∈ subsetSums weights := by
  induction weights generalizing selected with
  | nil => simp [selectedSum, subsetSums]
  | cons weight rest ih =>
    have tail := ih (fun index => selected (index + 1))
    rw [subsetSums, mem_unique, List.mem_append]
    cases truth : selected 0 with
    | false => exact Or.inl (by simpa [selectedSum, truth] using tail)
    | true =>
      apply Or.inr
      apply List.mem_map.mpr
      exact ⟨_, tail, by simp [selectedSum, truth]⟩

/-- Every candidate row is a set of numerical values, not a multiset of witnesses. -/
theorem subset_sums_nodup (weights : List Int) : (subsetSums weights).Nodup := by
  cases weights with
  | nil => simp [subsetSums]
  | cons weight rest => exact unique_nodup _

/-- The worst-case value count is at most the number of subsets. This is
    neither a runtime work bound nor a requirement to pre-enumerate that space. -/
theorem subset_sums_length_le_pow (weights : List Int) :
    (subsetSums weights).length ≤ 2 ^ weights.length := by
  induction weights with
  | nil => simp [subsetSums]
  | cons weight rest ih =>
    have first := unique_length_le (subsetSums rest ++ (subsetSums rest).map (fun value => weight + value))
    simp only [List.length_append, List.length_map] at first
    have second := Nat.add_le_add ih ih
    apply Nat.le_trans first
    simpa [Nat.pow_succ, Nat.mul_two] using second

def weightedSum (weight : κ → Int) : List κ → Int
  | [] => 0
  | key :: rest => weight key + weightedSum weight rest

theorem perm_weighted_sum (weight : κ → Int) {left right : List κ}
    (permutation : left.Perm right) : weightedSum weight left = weightedSum weight right := by
  induction permutation with
  | nil => rfl
  | cons head permutation ih => simp [weightedSum, ih]
  | swap first second rest => simp [weightedSum, Int.add_left_comm]
  | trans first second ihFirst ihSecond => exact ihFirst.trans ihSecond

/-- Filtering a carrier chooses one of its generated subset sums. -/
theorem filtered_sum_is_candidate (weight : κ → Int) (keys : List κ) (eligible : κ → Bool) :
    weightedSum weight (keys.filter eligible) ∈ subsetSums (keys.map weight) := by
  induction keys with
  | nil => simp [weightedSum, subsetSums]
  | cons head rest ih =>
    simp only [List.map_cons, subsetSums, mem_unique, List.mem_append]
    cases truth : eligible head with
    | false => exact Or.inl (by simpa [truth] using ih)
    | true =>
      apply Or.inr
      apply List.mem_map.mpr
      exact ⟨_, ih, by simp [truth, weightedSum]⟩

theorem filter_nodup (eligible : κ → Bool) (keys : List κ) (nodup : keys.Nodup) :
    (keys.filter eligible).Nodup := by
  induction keys with
  | nil => simp
  | cons head rest ih =>
    obtain ⟨absent, tail⟩ := List.nodup_cons.mp nodup
    cases truth : eligible head <;> simp [truth]
    · exact ih tail
    · exact ⟨absent, ih tail⟩

/-- Coverage of the independently supplied actual key set is essential: the
    theorem does not define the actual sum by silently dropping unknown keys. -/
theorem covered_actual_sum_is_candidate [DecidableEq κ] (weight : κ → Int)
    (carrier actual : List κ) (carrier_unique : carrier.Nodup) (actual_unique : actual.Nodup)
    (covers : ∀ key, key ∈ actual → key ∈ carrier) :
    weightedSum weight actual ∈ subsetSums (carrier.map weight) := by
  let eligible := fun key => decide (key ∈ actual)
  have same : actual.Perm (carrier.filter eligible) := by
    apply (List.perm_ext_iff_of_nodup actual_unique (filter_nodup eligible carrier carrier_unique)).mpr
    intro key
    simp only [List.mem_filter, eligible, decide_eq_true_eq]
    exact ⟨fun member => ⟨covers key member, member⟩, And.right⟩
  rw [perm_weighted_sum weight same]
  exact filtered_sum_is_candidate weight carrier eligible

/-- A tuple is active if at least one of its raw supports is eligible. -/
def activeTuples (elements : List Element) : List FullTuple :=
  unique ((elements.filter Element.eligible).map Element.tuple)

theorem active_tuples_nodup (elements : List Element) : (activeTuples elements).Nodup :=
  unique_nodup _

theorem mem_active_tuples (tuple : FullTuple) (elements : List Element) :
    tuple ∈ activeTuples elements ↔
      ∃ element, element ∈ elements ∧ element.eligible = true ∧ element.tuple = tuple := by
  simp only [activeTuples, mem_unique, List.mem_map, List.mem_filter]
  constructor
  · rintro ⟨element, ⟨member, enabled⟩, same⟩
    exact ⟨element, member, enabled, same⟩
  · rintro ⟨element, member, enabled, same⟩
    exact ⟨element, ⟨member, enabled⟩, same⟩

/-- OR all ground supports for the same complete tuple. -/
def eligibleFor (tuple : FullTuple) (elements : List Element) : Bool :=
  elements.any (fun element => decide (element.tuple = tuple) && element.eligible)

theorem eligible_for_iff_active (tuple : FullTuple) (elements : List Element) :
    eligibleFor tuple elements = true ↔ tuple ∈ activeTuples elements := by
  rw [mem_active_tuples]
  simp only [eligibleFor, List.any_eq_true, Bool.and_eq_true, decide_eq_true_eq]
  constructor
  · rintro ⟨element, member, same, enabled⟩
    exact ⟨element, member, enabled, same⟩
  · rintro ⟨element, member, enabled, same⟩
    exact ⟨element, member, same, enabled⟩

/-- One element per distinct whole tuple, preserving every alternative support. -/
def coalesced (elements : List Element) : List Element :=
  (unique (elements.map Element.tuple)).map (fun tuple => ⟨tuple, eligibleFor tuple elements⟩)

theorem coalesced_active_tuples (tuple : FullTuple) (elements : List Element) :
    tuple ∈ activeTuples (coalesced elements) ↔ tuple ∈ activeTuples elements := by
  constructor
  · intro active
    obtain ⟨element, member, enabled, same⟩ := (mem_active_tuples tuple _).mp active
    obtain ⟨key, _, entry⟩ := List.mem_map.mp member
    subst element
    simp only at same enabled
    subst tuple
    exact (eligible_for_iff_active key elements).mp enabled
  · intro active
    have key : tuple ∈ unique (elements.map Element.tuple) := by
      obtain ⟨element, member, _, same⟩ := (mem_active_tuples tuple elements).mp active
      exact (mem_unique tuple _).mpr (List.mem_map.mpr ⟨element, member, same⟩)
    apply (mem_active_tuples tuple _).mpr
    exact ⟨⟨tuple, eligibleFor tuple elements⟩, List.mem_map.mpr ⟨tuple, key, rfl⟩,
      (eligible_for_iff_active tuple elements).mpr active, rfl⟩

/-- The actual aggregate sums each active full tuple once across all supports. -/
def actualSum (weight : FullTuple → Int) (elements : List Element) : Int :=
  weightedSum weight (activeTuples elements)

/-- Whole-tuple coalescing preserves the actual signed integer sum. -/
theorem coalescing_preserves_actual_sum (weight : FullTuple → Int) (elements : List Element) :
    actualSum weight (coalesced elements) = actualSum weight elements := by
  apply perm_weighted_sum weight
  apply (List.perm_ext_iff_of_nodup (active_tuples_nodup _) (active_tuples_nodup _)).mpr
  intro tuple
  exact coalesced_active_tuples tuple elements

/-- Candidate values cover every actual aggregate result when every active raw
    tuple is present in the finite carrier. Signed weights and negative sums are
    admitted. No independence or realizability assumption is made about supports. -/
theorem actual_tuple_sum_is_candidate (weight : FullTuple → Int)
    (carrier : List FullTuple) (elements : List Element)
    (covers : ∀ element, element ∈ elements → element.eligible = true → element.tuple ∈ carrier) :
    actualSum weight elements ∈ subsetSums ((unique carrier).map weight) := by
  apply covered_actual_sum_is_candidate weight _ _ (unique_nodup carrier) (active_tuples_nodup elements)
  intro tuple member
  obtain ⟨element, inside, enabled, same⟩ := (mem_active_tuples tuple elements).mp member
  apply (mem_unique tuple carrier).mpr
  simpa [same] using covers element inside enabled

/-- The first numerical candidate alone does not establish value coverage. -/
theorem initial_zero_may_miss_actual_sum :
    selectedSum [1] (fun _ => true) ∉ ([0] : List Int) := by
  decide

/-- Correlated tuple conditions can leave generated values unrealizable. The
    generator remains a sound overapproximation, requiring a later oracle check. -/
theorem subset_candidates_need_not_be_realizable :
    (1 : Int) ∈ subsetSums [1, 1] ∧
      ∀ eligible : Bool, selectedSum [1, 1] (fun _ => eligible) ≠ 1 := by
  constructor
  · decide
  · intro eligible
    cases eligible <;> decide

end Zetesis.AggregateAssignment
