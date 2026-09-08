import Zetesis.HeadMeasures

/-!
# Count heads without a tuple/atom bijection

A row has a complete tuple key, a positive head atom, and an eligibility formula.
Permission is indexed by atoms; numeric activity is indexed independently by
complete tuples. The same atom may occur under several keys, and several atoms
may activate one key. No tuple/atom correspondence is assumed below.

The central laws characterize these two indices in original and arbitrary frozen
interpretations. A finite complete key carrier counts each tuple once. Equivalent
tuple activities and equivalent atom permissions compose through `HeadMeasures`
into context-preserving groups. Bounds remain candidate constraints and supply
no reduct support.

These are semantic laws over complete finite row tables. They do not prove source
join completeness, support-closure construction, the Rust maps, finite-width
accounting, guard compilation, or the concrete solver/shader implementation.
The older bijective `CountHeads` and `CountEligibility` laws remain available for
optimizations requiring that stronger premise.
-/

namespace Zetesis.CountHeadActivity
open Ferraris ChoiceIntervals
open CountEligibility (Row keyed headed eligibility tupleActivity)
universe u v
variable {A : Type u} {K : Type v}

/-- The key carrier covers exactly the finite tuple set, with one position per
    complete key. Its size is independent of the number of permitted atoms. -/
structure Carrier (rows : List (Row K A)) where
  keys : List K
  distinct : keys.Nodup
  complete : ∀ key, key ∈ keys ↔ ∃ row ∈ rows, row.identity.key = key

/-- A tuple is selected when at least one matching row has its own true head
    and its own true eligibility. Alias directions require no special case. -/
theorem activity_original (M : Atoms A) (rows : List (Row K A)) (key : K) :
    Satisfies M (tupleActivity rows key) ↔
      ∃ row ∈ rows, row.identity.key = key ∧
        M row.identity.atom ∧ Satisfies M row.eligible := by
  classical
  simp only [tupleActivity, keyed, RuleFactorization.satisfies_any, List.mem_map,
    List.mem_filter, decide_eq_true_eq]
  constructor
  · rintro ⟨_, ⟨row, ⟨member, same⟩, rfl⟩, selected, eligible⟩
    exact ⟨row, member, same, selected, eligible⟩
  · rintro ⟨row, member, same, selected, eligible⟩
    exact ⟨_, ⟨row, ⟨member, same⟩, rfl⟩, selected, eligible⟩

/-- Frozen activity preserves the reduct of each eligible witness. Original
    truth alone never substitutes for recursive or default-negated eligibility. -/
theorem activity_frozen (M J : Atoms A) (rows : List (Row K A)) (key : K) :
    Satisfies J (Reduct M (tupleActivity rows key)) ↔
      ∃ row ∈ rows, row.identity.key = key ∧
        Satisfies J (Reduct M (.atom row.identity.atom)) ∧
          Satisfies J (Reduct M row.eligible) := by
  classical
  simp only [tupleActivity, keyed, RuleFactorization.reduct_any, List.mem_map,
    List.mem_filter, decide_eq_true_eq]
  constructor
  · rintro ⟨_, ⟨row, ⟨member, same⟩, rfl⟩, active⟩
    have selected := (RuleFactorization.reduct_conj M J _ _).mp active
    exact ⟨row, member, same, selected⟩
  · rintro ⟨row, member, same, selected, eligible⟩
    exact ⟨_, ⟨row, ⟨member, same⟩, rfl⟩,
      (RuleFactorization.reduct_conj M J _ _).mpr ⟨selected, eligible⟩⟩

/-- Permission coalesces the eligibility of every row with the same atom,
    independently of the rows' tuple keys. -/
theorem eligibility_original (M : Atoms A) (rows : List (Row K A)) (atom : A) :
    Satisfies M (eligibility rows atom) ↔
      ∃ row ∈ rows, row.identity.atom = atom ∧ Satisfies M row.eligible := by
  classical
  simp only [CountEligibility.eligibility, headed, RuleFactorization.satisfies_any,
    List.mem_map, List.mem_filter, decide_eq_true_eq]
  constructor
  · rintro ⟨_, ⟨row, ⟨member, same⟩, rfl⟩, eligible⟩
    exact ⟨row, member, same, eligible⟩
  · rintro ⟨row, member, same, eligible⟩
    exact ⟨_, ⟨row, ⟨member, same⟩, rfl⟩, eligible⟩

/-- Coalesced permission retains all original witness reducts. -/
theorem eligibility_frozen (M J : Atoms A) (rows : List (Row K A)) (atom : A) :
    Satisfies J (Reduct M (eligibility rows atom)) ↔
      ∃ row ∈ rows, row.identity.atom = atom ∧ Satisfies J (Reduct M row.eligible) := by
  classical
  simp only [CountEligibility.eligibility, headed, RuleFactorization.reduct_any,
    List.mem_map, List.mem_filter, decide_eq_true_eq]
  constructor
  · rintro ⟨_, ⟨row, ⟨member, same⟩, rfl⟩, eligible⟩
    exact ⟨row, member, same, eligible⟩
  · rintro ⟨row, member, same, eligible⟩
    exact ⟨_, ⟨row, ⟨member, same⟩, rfl⟩, eligible⟩

/-- Row multiplicity and ordering do not change an activity when both tables
    contain exactly the same complete row/eligibility records. -/
theorem activity_of_same_rows (first second : List (Row K A))
    (same : ∀ row, row ∈ first ↔ row ∈ second) (key : K) :
    Equivalent (tupleActivity first key) (tupleActivity second key) := by
  constructor
  · intro M
    simp only [activity_original, same]
  · intro M J
    simp only [activity_frozen, same]

/-- The independent permission index has the same row-preservation law. -/
theorem eligibility_of_same_rows (first second : List (Row K A))
    (same : ∀ row, row ∈ first ↔ row ∈ second) (atom : A) :
    Equivalent (eligibility first atom) (eligibility second atom) := by
  constructor
  · intro M
    simp only [eligibility_original, same]
  · intro M J
    simp only [eligibility_frozen, same]

/-- Count thresholds range over unique complete tuples, never unique heads. -/
theorem threshold_original (M : Atoms A) (rows : List (Row K A))
    (carrier : Carrier rows) (needed : Nat) :
    Satisfies M (atLeast needed (carrier.keys.map (tupleActivity rows))) ↔
      needed ≤ count (fun key => ∃ row ∈ rows, row.identity.key = key ∧
        M row.identity.atom ∧ Satisfies M row.eligible) carrier.keys := by
  classical
  have counted : count (Satisfies M) (carrier.keys.map (tupleActivity rows)) =
      count (fun key => ∃ row ∈ rows, row.identity.key = key ∧
        M row.identity.atom ∧ Satisfies M row.eligible) carrier.keys := by
    simp only [count, List.filter_map, List.length_map, Function.comp_def]
    congr 2
    funext key
    by_cases active : ∃ row ∈ rows, row.identity.key = key ∧
        M row.identity.atom ∧ Satisfies M row.eligible
    · have selected := (activity_original M rows key).mpr active
      simp [active, selected]
    · have inactive : ¬ Satisfies M (tupleActivity rows key) :=
        fun selected => active ((activity_original M rows key).mp selected)
      simp [active, inactive]
  rw [atLeast_classical, counted]

/-- Bounds inspect original tuple activity and cannot assert a head in J. -/
theorem bounds_are_candidate_constraints (M J : Atoms A) (rows : List (Row K A))
    (carrier : Carrier rows) (body : Formula A) (lower upper : Nat) :
    Satisfies J (Reduct M (HeadMeasures.bound body
      (.conj (atLeast lower (carrier.keys.map (tupleActivity rows)))
        (Neg (atLeast (upper + 1) (carrier.keys.map (tupleActivity rows))))))) ↔
      (Satisfies M body →
        lower ≤ count (Satisfies M) (carrier.keys.map (tupleActivity rows)) ∧
        count (Satisfies M) (carrier.keys.map (tupleActivity rows)) ≤ upper) := by
  classical
  rw [HeadMeasures.bound_frozen]
  simp [Satisfies, Ferraris.Neg, atLeast_classical, Nat.not_le, Nat.lt_succ_iff]

/-- Preserved rows preserve atom permissions and each tuple's count activity;
    composing them retains the complete measured group in every context. -/
theorem stable_in_context (M : Atoms A) (first second : List (Row K A))
    (same : ∀ row, row ∈ first ↔ row ∈ second) (keys : List K) (heads : List A)
    (body : Formula A) (needed : Nat) (context : Theory A) :
    Stable M (HeadMeasures.group body heads (eligibility first)
      (atLeast needed (keys.map (tupleActivity first))) :: context) ↔
    Stable M (HeadMeasures.group body heads (eligibility second)
      (atLeast needed (keys.map (tupleActivity second))) :: context) := by
  have activities : Equivalent (atLeast needed (keys.map (tupleActivity first)))
      (atLeast needed (keys.map (tupleActivity second))) := by
    apply atLeast_map_equivalent
    intro key
    exact activity_of_same_rows first second same key
  exact HeadMeasures.stable_in_context M body heads
    (eligibility first) (eligibility second) _ _
    (eligibility_of_same_rows first second same) activities.1 context

end Zetesis.CountHeadActivity
