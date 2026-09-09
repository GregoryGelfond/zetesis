import Zetesis.BooleanHeads
import Zetesis.HeadMeasures
import Zetesis.AggregateReduct

/-!
# Unsigned Boolean operands inside choice and measured heads

An element has an unsigned atomic or Boolean operand and an arbitrary eligibility
formula. A true operand contributes its eligibility to tuple activity; a false
operand contributes nothing. Neither Boolean operand supplies an atom permission.
The numeric bound remains a candidate constraint, including in arbitrary contexts.

Identity is separate from truth. Explicit aggregates use complete tuple keys.
Ordinary choices use atom identity for atomic operands and source-element
occurrence identity for Boolean operands within one activated outer group. Local
witnesses of one Boolean occurrence share a key; two source occurrences do not.
`ChoiceKey` records this distinction without identifying a key with its Boolean
value. A completed source adapter must supply those identities and finite rows.

The canonical mask formula supports any fixed numeric guard. These laws do not
prove source binding/occurrence assignment, term evaluation, machine arithmetic,
optimized lowering, resource accounting or Rust/device refinement. Default-negated
operands are outside this unsigned element grammar.
-/

namespace Zetesis.BooleanHeadElements

open Ferraris ChoiceIntervals

universe u v w
variable {A : Type u} {K : Type v} {O : Type w}

/-- Constants have truth but no atom identity. Atomic polarity, when present in
    the source vocabulary, belongs to A; default negation is not an operand. -/
inductive Operand (A : Type u) where
  | atom : A → Operand A
  | boolean : Bool → Operand A
  deriving DecidableEq

def operand : Operand A → Formula A
  | .atom atom => .atom atom
  | .boolean value => BooleanHeads.constant value

def Holds (M : Atoms A) : Operand A → Prop
  | .atom atom => M atom
  | .boolean value => value = true

/-- A constant's original truth is independent of interpretation membership. -/
theorem operand_original (M : Atoms A) (head : Operand A) :
    Satisfies M (operand head) ↔ Holds M head := by
  cases head with
  | atom atom => rfl
  | boolean value => exact BooleanHeads.constant_original M value

/-- Frozen atoms require both M and J membership; constants keep their truth. -/
theorem operand_frozen (M J : Atoms A) (head : Operand A) :
    Satisfies J (Reduct M (operand head)) ↔ Holds M head ∧ Holds J head := by
  cases head with
  | atom atom => exact atom_reduct M J atom
  | boolean value =>
    rw [operand, BooleanHeads.constant_frozen]
    simp only [Holds, and_self]

/-- Ordinary atomic elements coalesce by atom; Boolean elements by source
    occurrence. O is local to one activated outer group, not a local witness. -/
inductive ChoiceKey (A : Type u) (O : Type w) where
  | atom : A → ChoiceKey A O
  | boolean : O → ChoiceKey A O
  deriving DecidableEq

def choiceKey (occurrence : O) : Operand A → ChoiceKey A O
  | .atom atom => .atom atom
  | .boolean _ => .boolean occurrence

/-- Equal Boolean values at distinct source occurrences remain distinct keys. -/
theorem boolean_keys_distinct (first second : O) (different : first ≠ second)
    (left right : Bool) :
    choiceKey first (.boolean left : Operand A) ≠ choiceKey second (.boolean right) := by
  intro same
  exact different (ChoiceKey.boolean.inj same)

/-- All local witnesses of one Boolean source occurrence use its same key. -/
theorem boolean_witness_key (occurrence : O) (left right : Bool) :
    choiceKey occurrence (.boolean left : Operand A) =
      choiceKey occurrence (.boolean right) := rfl

/-- Atomic choices retain atom identity independently of source occurrence. -/
theorem atom_choice_key (first second : O) (atom : A) :
    choiceKey first (.atom atom) = choiceKey second (.atom atom) := rfl

structure Row (K : Type v) (A : Type u) where
  key : K
  head : Operand A
  eligible : Formula A

/-- Exact complete-key coverage is required separately from the row semantics.
    An ordinary choice uses ChoiceKey; an explicit aggregate uses its tuples. -/
structure Carrier (rows : List (Row K A)) where
  keys : List K
  distinct : keys.Nodup
  complete : ∀ key, key ∈ keys ↔ ∃ row ∈ rows, row.key = key

open Classical in
noncomputable def activity (rows : List (Row K A)) (key : K) : Formula A :=
  RuleFactorization.any ((rows.filter (fun row => row.key = key)).map
    (fun row => .conj (operand row.head) row.eligible))

/-- True operands contribute their eligible rows, false operands none. Rows
    sharing a complete key OR their activity rather than being counted twice. -/
theorem activity_original (M : Atoms A) (rows : List (Row K A)) (key : K) :
    Satisfies M (activity rows key) ↔
      ∃ row ∈ rows, row.key = key ∧ Holds M row.head ∧ Satisfies M row.eligible := by
  classical
  simp only [activity, RuleFactorization.satisfies_any, List.mem_map,
    List.mem_filter, decide_eq_true_eq]
  constructor
  · rintro ⟨_, ⟨row, ⟨member, same⟩, rfl⟩, head, eligible⟩
    exact ⟨row, member, same, (operand_original M row.head).mp head, eligible⟩
  · rintro ⟨row, member, same, head, eligible⟩
    exact ⟨_, ⟨row, ⟨member, same⟩, rfl⟩,
      (operand_original M row.head).mpr head, eligible⟩

/-- Frozen tuple activity keeps each witness's recursively frozen eligibility.
    A true Boolean operand never replaces that condition by its original truth. -/
theorem activity_frozen (M J : Atoms A) (rows : List (Row K A)) (key : K) :
    Satisfies J (Reduct M (activity rows key)) ↔
      ∃ row ∈ rows, row.key = key ∧ (Holds M row.head ∧ Holds J row.head) ∧
        Satisfies J (Reduct M row.eligible) := by
  classical
  simp only [activity, RuleFactorization.reduct_any, List.mem_map,
    List.mem_filter, decide_eq_true_eq]
  constructor
  · rintro ⟨_, ⟨row, ⟨member, same⟩, rfl⟩, active⟩
    have parts := (RuleFactorization.reduct_conj M J _ _).mp active
    exact ⟨row, member, same, (operand_frozen M J row.head).mp parts.1, parts.2⟩
  · rintro ⟨row, member, same, head, eligible⟩
    exact ⟨_, ⟨row, ⟨member, same⟩, rfl⟩,
      (RuleFactorization.reduct_conj M J _ _).mpr
        ⟨(operand_frozen M J row.head).mpr head, eligible⟩⟩

/-- Permission to select an operand is distinct from its measured activity. -/
def permission (body eligible : Formula A) (head : Operand A) : Formula A :=
  .imp (.conj body eligible) (.disj (operand head) (Neg (operand head)))

/-- Every permission is classically true; its reduct can still retain an atom. -/
theorem permission_original (M : Atoms A) (body eligible : Formula A)
    (head : Operand A) : Satisfies M (permission body eligible head) := by
  classical
  intro _
  exact Classical.em (Satisfies M (operand head))

/-- Frozen permission retains a selected atomic operand only when the same
    body's and eligibility's reducts hold. Boolean truth is already constant. -/
theorem permission_frozen (M J : Atoms A) (body eligible : Formula A)
    (head : Operand A) :
    Satisfies J (Reduct M (permission body eligible head)) ↔
      (Satisfies J (Reduct M body) → Satisfies J (Reduct M eligible) →
        Holds M head → Holds J head) := by
  have choice : Satisfies J (Reduct M (.disj (operand head) (Neg (operand head)))) ↔
      (Holds M head → Holds J head) := by
    rw [RuleFactorization.reduct_disj, operand_frozen, negation_frozen, operand_original]
    constructor
    · intro selected original
      rcases selected with both | absent
      · exact both.2
      · exact False.elim (absent original)
    · intro retained
      by_cases original : Holds M head
      · exact Or.inl ⟨original, retained original⟩
      · exact Or.inr original
  rw [permission, RuleFactorization.reduct_imp]
  constructor
  · intro holds body_true eligible_true
    exact choice.mp (holds.2
      ((RuleFactorization.reduct_conj M J body eligible).mpr ⟨body_true, eligible_true⟩))
  · intro retained
    refine ⟨permission_original M body eligible head, ?_⟩
    intro antecedent
    have parts := (RuleFactorization.reduct_conj M J body eligible).mp antecedent
    exact choice.mpr (retained parts.1 parts.2)

/-- Boolean choice permission is true in every frozen interpretation. Removing
    it cannot remove support for any atom, whatever its eligibility formula. -/
theorem boolean_permission_frozen (M J : Atoms A) (body eligible : Formula A)
    (value : Bool) :
    Satisfies J (Reduct M (permission body eligible (.boolean value))) := by
  rw [permission, RuleFactorization.reduct_imp]
  refine ⟨permission_original M body eligible (.boolean value), ?_⟩
  intro _
  rw [RuleFactorization.reduct_disj]
  cases value with
  | false =>
    apply Or.inr
    rw [negation_frozen, operand, BooleanHeads.constant_original]
    decide
  | true =>
    exact Or.inl ((operand_frozen M J (.boolean true)).mpr ⟨rfl, rfl⟩)

def permissions (body : Formula A) (rows : List (Row K A)) : Formula A :=
  all (rows.map (fun row => permission body row.eligible row.head))

def atomicRows (rows : List (Row K A)) : List (Row K A) :=
  rows.filter (fun row => match row.head with | .atom _ => true | .boolean _ => false)

/-- Boolean rows may be omitted from the permission conjunction, while remaining
    in the independent activity table. Original and arbitrary frozen truth agree. -/
theorem remove_boolean_permissions (body : Formula A) (rows : List (Row K A)) :
    Equivalent (permissions body rows) (permissions body (atomicRows rows)) := by
  have original (M : Atoms A) (table : List (Row K A)) :
      Satisfies M (permissions body table) := by
    rw [permissions, all_classical, List.forall_mem_map]
    intro row _
    exact permission_original M body row.eligible row.head
  constructor
  · intro M
    exact ⟨fun _ => original M _, fun _ => original M _⟩
  · intro M J
    simp only [permissions, AggregateReduct.all_frozen, List.forall_mem_map]
    constructor
    · intro holds row member
      exact holds row (List.mem_filter.mp member).1
    · intro holds row member
      cases head : row.head with
      | atom atom =>
        simpa only [head] using
          holds row (List.mem_filter.mpr ⟨member, by simp only [head]⟩)
      | boolean value =>
        exact boolean_permission_frozen M J body row.eligible value

open Classical in
/-- Only atomic operands supply entries to the coalesced permission index. -/
noncomputable def eligibility (rows : List (Row K A)) (atom : A) : Formula A :=
  RuleFactorization.any ((rows.filter (fun row => row.head = .atom atom)).map Row.eligible)

/-- Coalescing keeps each atomic row's original eligibility. Boolean operands
    cannot witness membership in this permission index. -/
theorem eligibility_original (M : Atoms A) (rows : List (Row K A)) (atom : A) :
    Satisfies M (eligibility rows atom) ↔
      ∃ row ∈ rows, row.head = .atom atom ∧ Satisfies M row.eligible := by
  classical
  simp only [eligibility, RuleFactorization.satisfies_any, List.mem_map,
    List.mem_filter, decide_eq_true_eq]
  constructor
  · rintro ⟨_, ⟨row, ⟨member, head⟩, rfl⟩, truth⟩
    exact ⟨row, member, head, truth⟩
  · rintro ⟨row, member, head, truth⟩
    exact ⟨_, ⟨row, ⟨member, head⟩, rfl⟩, truth⟩

/-- Coalescing atom permissions preserves arbitrary frozen eligibility, not
    merely the set of atoms whose conditions were originally possible. -/
theorem eligibility_frozen (M J : Atoms A) (rows : List (Row K A)) (atom : A) :
    Satisfies J (Reduct M (eligibility rows atom)) ↔
      ∃ row ∈ rows, row.head = .atom atom ∧ Satisfies J (Reduct M row.eligible) := by
  classical
  simp only [eligibility, RuleFactorization.reduct_any, List.mem_map,
    List.mem_filter, decide_eq_true_eq]
  constructor
  · rintro ⟨_, ⟨row, ⟨member, head⟩, rfl⟩, truth⟩
    exact ⟨row, member, head, truth⟩
  · rintro ⟨row, member, head, truth⟩
    exact ⟨_, ⟨row, ⟨member, head⟩, rfl⟩, truth⟩

/-- A covering atom carrier can replace row permissions by their atom-indexed
    eligibility ORs. Boolean rows remain solely in the activity table. -/
theorem permissions_coalesced (body : Formula A) (rows : List (Row K A))
    (heads : List A)
    (coverage : ∀ row ∈ rows, ∀ atom, row.head = .atom atom → atom ∈ heads) :
    Equivalent (permissions body rows)
      (all (heads.map (fun atom => HeadMeasures.permission body (eligibility rows atom) atom))) := by
  have atomic_permission (atom : A) :
      HeadMeasures.permission body (eligibility rows atom) atom =
        permission body (eligibility rows atom) (.atom atom) := rfl
  constructor
  · intro M
    have row_truth : Satisfies M (permissions body rows) := by
      rw [permissions, all_classical, List.forall_mem_map]
      intro row _
      exact permission_original M body row.eligible row.head
    have atom_truth : Satisfies M
        (all (heads.map (fun atom => HeadMeasures.permission body (eligibility rows atom) atom))) := by
      rw [all_classical, List.forall_mem_map]
      intro atom _
      rw [atomic_permission]
      exact permission_original M body _ (.atom atom)
    exact ⟨fun _ => atom_truth, fun _ => row_truth⟩
  · intro M J
    simp only [permissions, AggregateReduct.all_frozen, List.forall_mem_map,
      atomic_permission, permission_frozen, eligibility_frozen]
    constructor
    · intro retained atom _ body_true witness atom_true
      obtain ⟨row, member, head, eligible_true⟩ := witness
      have keep := retained row member body_true eligible_true
      rw [head] at keep
      exact keep atom_true
    · intro retained row member body_true eligible_true head_true
      cases head : row.head with
      | atom atom =>
        have permitted := retained atom (coverage row member atom head) body_true
          ⟨row, member, head, eligible_true⟩
        rw [head] at head_true
        exact permitted head_true
      | boolean value =>
        rw [head] at head_true
        exact head_true

open Classical in
noncomputable def selected (M : Atoms A) (rows : List (Row K A))
    (keys : List K) : AggregateReduct.Mask keys.length :=
  fun index => decide (∃ row ∈ rows, row.key = keys[index] ∧
    Holds M row.head ∧ Satisfies M row.eligible)

/-- The guard may express count, sum or extrema over distinct supplied keys.
    It must retain the numeric and empty-value semantics appropriate to that key. -/
noncomputable def formula (rows : List (Row K A)) (keys : List K)
    (accepts : AggregateReduct.Mask keys.length → Bool) : Formula A :=
  AggregateReduct.aggregate (fun index => activity rows keys[index]) accepts
    (AggregateReduct.masks keys.length)

/-- Numeric truth uses the complete tuple or occurrence carrier. This does not
    permit substituting Boolean value identity for source occurrence identity. -/
theorem formula_original (M : Atoms A) (rows : List (Row K A))
    (carrier : Carrier rows) (accepts : AggregateReduct.Mask carrier.keys.length → Bool) :
    Satisfies M (formula rows carrier.keys accepts) ↔
      accepts (selected M rows carrier.keys) = true := by
  have activity_mask : AggregateReduct.active M (fun index : Fin carrier.keys.length =>
      activity rows carrier.keys[index]) = selected M rows carrier.keys := by
    classical
    funext index
    simp only [AggregateReduct.active, selected, activity_original]
  rw [formula, AggregateReduct.original _ _ _ _ (AggregateReduct.masks_complete _),
    activity_mask]

/-- A measured group retains row permissions independently of its candidate bound. -/
noncomputable def group (body : Formula A) (rows : List (Row K A)) (keys : List K)
    (accepts : AggregateReduct.Mask keys.length → Bool) : Formula A :=
  .conj (permissions body rows) (HeadMeasures.bound body (formula rows keys accepts))

/-- The separate atom-permission index preserves stable models in every context
    while all Boolean activity remains available to the unchanged numeric guard. -/
theorem coalesced_group_in_context (M : Atoms A) (body : Formula A)
    (rows : List (Row K A)) (keys : List K)
    (accepts : AggregateReduct.Mask keys.length → Bool) (heads : List A)
    (coverage : ∀ row ∈ rows, ∀ atom, row.head = .atom atom → atom ∈ heads)
    (context : Theory A) :
    Stable M (group body rows keys accepts :: context) ↔
      Stable M (HeadMeasures.group body heads (eligibility rows)
        (formula rows keys accepts) :: context) := by
  have preservation : Equivalent (group body rows keys accepts)
      (HeadMeasures.group body heads (eligibility rows) (formula rows keys accepts)) :=
    equivalent_conj _ _ _ _ (permissions_coalesced body rows heads coverage) (equivalent_refl _)
  simp only [Stable, models_cons, ReductTheory, List.map_cons, preservation.1, preservation.2]

/-- A Boolean-only head filters existing stable models without supplying support
    for an atom in the body or in any Boolean eligibility formula. -/
theorem boolean_group_in_context (M : Atoms A) (body : Formula A)
    (rows : List (Row K A)) (keys : List K)
    (accepts : AggregateReduct.Mask keys.length → Bool)
    (booleans : ∀ row ∈ rows, ∃ value, row.head = .boolean value)
    (context : Theory A) :
    Stable M (group body rows keys accepts :: context) ↔
      (Satisfies M body → Satisfies M (formula rows keys accepts)) ∧ Stable M context := by
  have permitted (candidate : Atoms A) : Satisfies candidate (permissions body rows) := by
    rw [permissions, all_classical, List.forall_mem_map]
    intro row _
    exact permission_original candidate body row.eligible row.head
  have frozen_permissions (J : Atoms A) :
      Satisfies J (Reduct M (permissions body rows)) := by
    rw [permissions, AggregateReduct.all_frozen, List.forall_mem_map]
    intro row member
    obtain ⟨value, same⟩ := booleans row member
    rw [same]
    exact boolean_permission_frozen M J body row.eligible value
  have original : Satisfies M (group body rows keys accepts) ↔
      (Satisfies M body → Satisfies M (formula rows keys accepts)) := by
    simp only [group, Satisfies, HeadMeasures.bound_original, permitted M, true_and]
  have frozen (J : Atoms A) : Satisfies J (Reduct M (group body rows keys accepts)) ↔
      (Satisfies M body → Satisfies M (formula rows keys accepts)) := by
    rw [group, RuleFactorization.reduct_conj, HeadMeasures.bound_frozen]
    exact and_iff_right (frozen_permissions J)
  constructor
  · intro stable
    have model : Satisfies M (group body rows keys accepts) ∧ Models M context :=
      (models_cons M _ context).mp stable.1
    have numeric : Satisfies M body → Satisfies M (formula rows keys accepts) :=
      original.mp model.1
    have minimal : ¬ ∃ J, ProperSub J M ∧ Models J (ReductTheory M context) := by
      rintro ⟨J, proper, reduct⟩
      apply stable.2
      refine ⟨J, proper, ?_⟩
      exact (models_cons J _ _).mpr ⟨(frozen J).mpr numeric, reduct⟩
    exact ⟨numeric, model.2, minimal⟩
  · rintro ⟨numeric, stable⟩
    have model : Models M (group body rows keys accepts :: context) :=
      (models_cons M _ context).mpr ⟨original.mpr numeric, stable.1⟩
    have minimal : ¬ ∃ J, ProperSub J M ∧
        Models J (ReductTheory M (group body rows keys accepts :: context)) := by
      rintro ⟨J, proper, reduct⟩
      exact stable.2 ⟨J, proper, ((models_cons J _ _).mp reduct).2⟩
    exact ⟨model, minimal⟩

/-- Repeated or reordered identical rows preserve the complete measured group.
    Keys remain fixed: this law never merges distinct Boolean occurrences. -/
theorem same_rows_in_context (M : Atoms A) (body : Formula A)
    (first second : List (Row K A)) (same : ∀ row, row ∈ first ↔ row ∈ second)
    (keys : List K) (accepts : AggregateReduct.Mask keys.length → Bool)
    (context : Theory A) :
    Stable M (group body first keys accepts :: context) ↔
      Stable M (group body second keys accepts :: context) := by
  have permissions_equal : Equivalent (permissions body first) (permissions body second) := by
    constructor
    · intro candidate
      simp only [permissions, all_classical, List.forall_mem_map, same]
    · intro candidate tested
      simp only [permissions, AggregateReduct.all_frozen, List.forall_mem_map, same]
  have numeric (candidate : Atoms A) :
      Satisfies candidate (formula first keys accepts) ↔
        Satisfies candidate (formula second keys accepts) := by
    have activities : AggregateReduct.active candidate (fun index : Fin keys.length =>
        activity first keys[index]) =
        AggregateReduct.active candidate (fun index : Fin keys.length =>
          activity second keys[index]) := by
      classical
      funext index
      simp only [AggregateReduct.active, activity_original, same]
    rw [formula, formula,
      AggregateReduct.original _ _ _ _ (AggregateReduct.masks_complete _),
      AggregateReduct.original _ _ _ _ (AggregateReduct.masks_complete _), activities]
  have preservation : Equivalent (group body first keys accepts)
      (group body second keys accepts) :=
    equivalent_conj _ _ _ _ permissions_equal (HeadMeasures.bound_equivalent body _ _ numeric)
  simp only [Stable, models_cons, ReductTheory, List.map_cons, preservation.1, preservation.2]

end Zetesis.BooleanHeadElements
