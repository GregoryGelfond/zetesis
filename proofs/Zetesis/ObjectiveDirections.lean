import Zetesis.AggregateAssignment
import Zetesis.Ferraris

/-!
# Direction normalization of finite mixed objectives

A mixed objective is a finite list of already-ground entries. Its raw reference
evaluator applies direction-aware key equality to active entries and charges
each equivalence class once. The executable compiler instead negates maximizing
weights, groups the resulting complete keys globally, and retains all support
alternatives for OR eligibility. Their fixed-priority integer vectors agree.

Weights are mathematical Int. Priority slots and objective presence are fixed
external parameters. This layer does not establish source grounding, clingo
priority presence, finite-width negation, resource completion, Rust refinement,
or any change to the original Ferraris theory.
-/

namespace Zetesis.ObjectiveDirections

open AggregateAssignment (unique weightedSum)

universe u v w
variable {σ : Type u} {χ : Type v} {α : Type w} [DecidableEq σ]

inductive Direction where
  | minimize
  | maximize
  deriving DecidableEq

/-- One already-ground source contribution with an abstract eligibility index. -/
structure Entry (σ : Type u) (χ : Type v) where
  direction : Direction
  weight : Int
  priority : Int
  tuple : List σ
  condition : χ

/-- Full identity after direction normalization. Priority and all tuple scalar
    constructors and positions remain part of the key. -/
structure Key (σ : Type u) where
  priority : Int
  weight : Int
  tuple : List σ
  deriving DecidableEq

def signedWeight (entry : Entry σ χ) : Int :=
  match entry.direction with
  | .minimize => entry.weight
  | .maximize => -entry.weight

def normalizedKey (entry : Entry σ χ) : Key σ :=
  ⟨entry.priority, signedWeight entry, entry.tuple⟩

/-- Source-level equivalence of contributions is applied after interpreting
    direction; equality of the original unsigned records is insufficient. -/
def SameContribution (left right : Entry σ χ) : Prop :=
  left.priority = right.priority ∧ signedWeight left = signedWeight right ∧
    left.tuple = right.tuple

instance (left right : Entry σ χ) : Decidable (SameContribution left right) :=
  inferInstanceAs (Decidable (_ ∧ _ ∧ _))

omit [DecidableEq σ] in
theorem same_contribution_iff_key (left right : Entry σ χ) :
    SameContribution left right ↔ normalizedKey left = normalizedKey right := by
  simp [SameContribution, normalizedKey, Key.mk.injEq]

def contributionAt (priority : Int) (entry : Entry σ χ) : Int :=
  if entry.priority = priority then signedWeight entry else 0

def keyAt (priority : Int) (key : Key σ) : Int :=
  if key.priority = priority then key.weight else 0

/-- Independent raw evaluator: scan active source entries, ignore an entry
    with an equivalent later active entry, and otherwise add its signed weight. -/
def rawCost (priority : Int) : List (Entry σ χ) → Int
  | [] => 0
  | entry :: rest =>
      if rest.any (fun other => decide (SameContribution entry other)) then
        rawCost priority rest
      else contributionAt priority entry + rawCost priority rest

def mixedCost (priority : Int) (truth : χ → Bool) (entries : List (Entry σ χ)) : Int :=
  rawCost priority (entries.filter (fun entry => truth entry.condition))

theorem raw_cost_keys (priority : Int) (entries : List (Entry σ χ)) :
    rawCost priority entries = weightedSum (keyAt priority)
      (unique (entries.map normalizedKey)) := by
  induction entries with
  | nil => rfl
  | cons entry rest ih =>
    have duplicate :
        (rest.any (fun other => decide (SameContribution entry other))) = true ↔
          normalizedKey entry ∈ unique (rest.map normalizedKey) := by
      simp only [List.any_eq_true, decide_eq_true_eq, same_contribution_iff_key,
        AggregateAssignment.mem_unique, List.mem_map]
      constructor
      · rintro ⟨other, member, same⟩
        exact ⟨other, member, same.symm⟩
      · rintro ⟨other, member, same⟩
        exact ⟨other, member, same.symm⟩
    by_cases present : normalizedKey entry ∈ unique (rest.map normalizedKey)
    · have active := duplicate.mpr present
      simp [rawCost, unique, present, active, ih]
    · have inactive :
          (rest.any (fun other => decide (SameContribution entry other))) = false := by
        cases value : rest.any (fun other => decide (SameContribution entry other))
        · rfl
        · exact False.elim (present (duplicate.mp value))
      simp only [rawCost, inactive, Bool.false_eq_true, ↓reduceIte, List.map_cons,
        unique, if_neg present, weightedSum, ih]
      rfl

/-- A compiled key retains every alternative's eligibility index, including
    alternatives originating in different minimize and maximize directives. -/
structure Group (σ : Type u) (χ : Type v) where
  key : Key σ
  conditions : List χ

def conditionsFor (key : Key σ) (entries : List (Entry σ χ)) : List χ :=
  (entries.filter (fun entry => decide (normalizedKey entry = key))).map Entry.condition

def normalize (entries : List (Entry σ χ)) : List (Group σ χ) :=
  (unique (entries.map normalizedKey)).map (fun key => ⟨key, conditionsFor key entries⟩)

def groupActive (truth : χ → Bool) (group : Group σ χ) : Bool :=
  group.conditions.any truth

def compiledCost (priority : Int) (truth : χ → Bool) (groups : List (Group σ χ)) : Int :=
  weightedSum (fun group => if groupActive truth group then keyAt priority group.key else 0) groups

theorem conditions_for_active (key : Key σ) (truth : χ → Bool) (entries : List (Entry σ χ)) :
    (conditionsFor key entries).any truth = true ↔
      ∃ entry, entry ∈ entries ∧ truth entry.condition = true ∧ normalizedKey entry = key := by
  simp only [conditionsFor, List.any_eq_true, List.mem_map, List.mem_filter,
    decide_eq_true_eq]
  constructor
  · rintro ⟨condition, ⟨entry, ⟨member, same⟩, rfl⟩, active⟩
    exact ⟨entry, member, active, same⟩
  · rintro ⟨entry, member, active, same⟩
    exact ⟨entry.condition, ⟨entry, ⟨member, same⟩, rfl⟩, active⟩

theorem normalized_keys_unique (entries : List (Entry σ χ)) :
    ((normalize entries).map Group.key).Nodup := by
  simpa [normalize, List.map_map, Function.comp_def] using
    AggregateAssignment.unique_nodup (entries.map normalizedKey)

def enabledKeys (truth : χ → Bool) (entries : List (Entry σ χ)) : List (Key σ) :=
  (unique (entries.map normalizedKey)).filter
    (fun key => (conditionsFor key entries).any truth)

theorem enabled_key_iff (key : Key σ) (truth : χ → Bool) (entries : List (Entry σ χ)) :
    key ∈ enabledKeys truth entries ↔
      key ∈ unique ((entries.filter (fun entry => truth entry.condition)).map normalizedKey) := by
  simp only [enabledKeys, List.mem_filter, AggregateAssignment.mem_unique,
    List.mem_map, conditions_for_active]
  constructor
  · rintro ⟨_, entry, member, active, same⟩
    exact ⟨entry, ⟨member, active⟩, same⟩
  · rintro ⟨entry, member, same⟩
    obtain ⟨inside, active⟩ := member
    exact ⟨⟨entry, inside, same⟩, entry, inside, active, same⟩

omit [DecidableEq σ] in
theorem weighted_filter (weight : Key σ → Int) (enabled : Key σ → Bool) (keys : List (Key σ)) :
    weightedSum (fun key => if enabled key then weight key else 0) keys =
      weightedSum weight (keys.filter enabled) := by
  induction keys with
  | nil => rfl
  | cons key keys ih => cases value : enabled key <;> simp [weightedSum, value, ih]

theorem weighted_map {δ : Type u} {ε : Type v} (weight : ε → Int)
    (mapping : δ → ε) (values : List δ) :
    weightedSum weight (values.map mapping) = weightedSum (fun value => weight (mapping value)) values := by
  induction values with
  | nil => rfl
  | cons value values ih => simp [weightedSum, ih]

theorem compiled_cost_enabled (priority : Int) (truth : χ → Bool) (entries : List (Entry σ χ)) :
    compiledCost priority truth (normalize entries) =
      weightedSum (keyAt priority) (enabledKeys truth entries) := by
  rw [compiledCost, normalize, weighted_map]
  exact weighted_filter (keyAt priority) _ _

/-- Normalization followed by global full-key grouping preserves each integer
    priority cost of the independent mixed-direction reference evaluator. -/
theorem normalization_preserves_cost (priority : Int) (truth : χ → Bool)
    (entries : List (Entry σ χ)) :
    compiledCost priority truth (normalize entries) = mixedCost priority truth entries := by
  rw [compiled_cost_enabled, mixedCost, raw_cost_keys]
  apply AggregateAssignment.perm_weighted_sum
  apply (List.perm_ext_iff_of_nodup
    (AggregateAssignment.filter_nodup _ _ (AggregateAssignment.unique_nodup _))
    (AggregateAssignment.unique_nodup _)).mpr
  intro key
  exact enabled_key_iff key truth entries

/-- This single fixed list is the admitted descending priority interface. The
    equality result actually holds for any list, including zero-cost slots. -/
def mixedVector (priorities : List Int) (truth : χ → Bool) (entries : List (Entry σ χ)) : List Int :=
  priorities.map (fun priority => mixedCost priority truth entries)

def compiledVector (priorities : List Int) (truth : χ → Bool) (groups : List (Group σ χ)) : List Int :=
  priorities.map (fun priority => compiledCost priority truth groups)

/-- Normalization preserves every slot and its signed integer value, rather
    than merely producing the same best scalar total. -/
theorem normalization_preserves_vector (priorities : List Int) (truth : χ → Bool)
    (entries : List (Entry σ χ)) :
    compiledVector priorities truth (normalize entries) = mixedVector priorities truth entries := by
  simp [compiledVector, mixedVector, normalization_preserves_cost]

/-- Executable lexicographic minimization. All compared vectors constructed
    above use the same fixed priority list and therefore the same length. -/
def compareVectors : List Int → List Int → Ordering
  | [], [] => .eq
  | [], _ :: _ => .lt
  | _ :: _, [] => .gt
  | left :: lefts, right :: rights =>
      if left < right then .lt
      else if right < left then .gt
      else compareVectors lefts rights

theorem normalization_preserves_comparison (priorities : List Int)
    (left right : χ → Bool) (entries : List (Entry σ χ)) :
    compareVectors (compiledVector priorities left (normalize entries))
      (compiledVector priorities right (normalize entries)) =
    compareVectors (mixedVector priorities left entries) (mixedVector priorities right entries) := by
  rw [normalization_preserves_vector, normalization_preserves_vector]

/-- Ranking adds a condition to stability of the original, unchanged theory. -/
def Optimal (T : Ferraris.Theory α) (score : Atoms α → List Int) (M : Atoms α) : Prop :=
  Ferraris.Stable M T ∧
    ∀ N, Ferraris.Stable N T → compareVectors (score M) (score N) ≠ .gt

/-- The complete set of optimal models, including every optimal tie, is
    unchanged. This theorem neither assumes nor supplies search exhaustion. -/
theorem normalization_preserves_optimal_models (T : Ferraris.Theory α)
    (priorities : List Int) (truth : Atoms α → χ → Bool) (entries : List (Entry σ χ)) (M : Atoms α) :
    Optimal T (fun N => compiledVector priorities (truth N) (normalize entries)) M ↔
      Optimal T (fun N => mixedVector priorities (truth N) entries) M := by
  simp only [Optimal, normalization_preserves_comparison]

/-- A score cannot replace the original candidate's frozen-reduct test. -/
theorem normalized_optimum_retains_reduct (T : Ferraris.Theory α)
    (priorities : List Int) (truth : Atoms α → χ → Bool) (entries : List (Entry σ χ)) (M : Atoms α)
    (optimal : Optimal T (fun N => compiledVector priorities (truth N) (normalize entries)) M) :
    Ferraris.MinimalModel M (Ferraris.ReductTheory M T) :=
  (Ferraris.stable_iff_minimal_reduct M T).mp optimal.1

/-- Retain all entries tied with a supplied incumbent; distinct interpretations
    and repeated list entries retain their original order and multiplicity. -/
def ties (score : Atoms α → List Int) (incumbent : Atoms α) (candidates : List (Atoms α)) :
    List (Atoms α) :=
  candidates.filter (fun candidate => score candidate == score incumbent)

theorem normalization_preserves_tie_list (priorities : List Int)
    (truth : Atoms α → χ → Bool) (entries : List (Entry σ χ))
    (incumbent : Atoms α) (candidates : List (Atoms α)) :
    ties (fun N => compiledVector priorities (truth N) (normalize entries)) incumbent candidates =
      ties (fun N => mixedVector priorities (truth N) entries) incumbent candidates := by
  simp only [ties, normalization_preserves_vector]

omit [DecidableEq σ] in
/-- Opposite directions with opposite source weights have the same full key.
    Their eligibility conditions need not be equal. -/
theorem opposite_directions_coalesce (weight priority : Int) (tuple : List σ)
    (left right : χ) :
    normalizedKey (⟨.minimize, -weight, priority, tuple, left⟩ : Entry σ χ) =
      normalizedKey (⟨.maximize, weight, priority, tuple, right⟩ : Entry σ χ) := by
  rfl

omit [DecidableEq σ] in
/-- Equal source weights of opposite directions remain distinct keys except
    at zero; merely deduplicating by priority and tuple would be incorrect. -/
theorem same_weight_directions_distinct (weight priority : Int) (tuple : List σ)
    (left right : χ) (nonzero : weight ≠ 0) :
    normalizedKey (⟨.minimize, weight, priority, tuple, left⟩ : Entry σ χ) ≠
      normalizedKey (⟨.maximize, weight, priority, tuple, right⟩ : Entry σ χ) := by
  intro same
  have weights := congrArg Key.weight same
  simp only [normalizedKey, signedWeight] at weights
  omega

/-- Mixed-direction alternatives coalesce globally and use OR eligibility;
    signed cancellation of genuinely distinct keys still leaves both keys. -/
theorem collision_and_cancellation_examples :
    let collision : List (Entry Int Bool) :=
      [⟨.minimize, -2, 1, [], true⟩, ⟨.maximize, 2, 1, [], false⟩]
    let cancellation : List (Entry Int Bool) :=
      [⟨.minimize, 2, 1, [], true⟩, ⟨.maximize, 2, 1, [], true⟩]
    (normalize collision).length = 1 ∧ compiledVector [1, 0] id (normalize collision) = [-2, 0] ∧
      (normalize cancellation).length = 2 ∧ compiledVector [1, 0] id (normalize cancellation) = [0, 0] := by
  decide

end Zetesis.ObjectiveDirections
