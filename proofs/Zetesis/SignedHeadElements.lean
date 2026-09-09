import Zetesis.BooleanHeadElements

/-!
# Signed operands in choices and measured heads

Default negation changes an operand's activity and ordinary-choice identity.
Both `not` and `not not` freeze operand truth in candidate M; neither supplies
positive atom permission. Eligibility remains a recursively reduced formula.
The original and frozen laws below hold for arbitrary M and J.

Ordinary atoms coalesce by (sign, atom), while Boolean operands retain source
occurrence keys independently of sign. Explicit aggregates retain complete tuple
keys. This extends the Boolean-choice convention in `BooleanHeadElements`, not
clingo's internal representation. Exact source identity is a separate obligation.

A semantic embedding moves default-negated operands into eligibility with a true
Boolean head. It preserves activity and permission and reuses the finite aggregate
and candidate-bound laws. It does not require the compiler to allocate these
intermediate rows. Source binding, complete carriers, numeric guards, machine
arithmetic, resource behavior and Rust/WGSL refinement remain unproved boundaries.
-/

namespace Zetesis.SignedHeadElements

open Ferraris ChoiceIntervals

universe u v w
variable {A : Type u} {K : Type v} {O : Type w}

/-- The three default-negation forms. Strong negation belongs to the atom type A. -/
inductive Sign where
  | positive
  | negative
  | doubleNegative
  deriving DecidableEq

structure Operand (A : Type u) where
  sign : Sign
  value : BooleanHeadElements.Operand A
  deriving DecidableEq

def operand (head : Operand A) : Formula A :=
  match head.sign with
  | .positive => BooleanHeadElements.operand head.value
  | .negative => Neg (BooleanHeadElements.operand head.value)
  | .doubleNegative => Neg (Neg (BooleanHeadElements.operand head.value))

def Holds (M : Atoms A) (head : Operand A) : Prop :=
  match head.sign with
  | .positive => BooleanHeadElements.Holds M head.value
  | .negative => ¬ BooleanHeadElements.Holds M head.value
  | .doubleNegative => BooleanHeadElements.Holds M head.value

/-- Only an unsigned atom must remain in J; signs freeze their truth in M. -/
def FrozenHolds (M J : Atoms A) (head : Operand A) : Prop :=
  match head.sign with
  | .positive => Holds M head ∧ Holds J head
  | .negative | .doubleNegative => Holds M head

/-- All six sign/value forms retain their stated original truth. -/
theorem operand_original (M : Atoms A) (head : Operand A) :
    Satisfies M (operand head) ↔ Holds M head := by
  rcases head with ⟨sign, value⟩
  cases sign with
  | positive => exact BooleanHeadElements.operand_original M value
  | negative =>
    change (¬ Satisfies M (BooleanHeadElements.operand value)) ↔ _
    rw [BooleanHeadElements.operand_original]
    rfl
  | doubleNegative =>
    exact (double_neg_satisfies M _).trans (BooleanHeadElements.operand_original M value)

/-- Freezing either default-negation form needs no subset assumption on J. -/
theorem operand_frozen (M J : Atoms A) (head : Operand A) :
    Satisfies J (Reduct M (operand head)) ↔ FrozenHolds M J head := by
  rcases head with ⟨sign, value⟩
  cases sign with
  | positive => exact BooleanHeadElements.operand_frozen M J value
  | negative =>
    change Satisfies J (Reduct M (Neg (BooleanHeadElements.operand value))) ↔ _
    rw [negation_frozen, BooleanHeadElements.operand_original]
    rfl
  | doubleNegative =>
    exact (double_neg_formula_reduct M J _).trans (BooleanHeadElements.operand_original M value)

/-- Original truth equivalence does not justify replacing `not not a` by `a`.
    A missing J atom witnesses the difference between their frozen reducts. -/
theorem double_negative_differs_from_positive (M J : Atoms A) (atom : A)
    (present : M atom) (absent : ¬ J atom) :
    ¬ (Satisfies J (Reduct M (operand ⟨.doubleNegative, .atom atom⟩)) ↔
      Satisfies J (Reduct M (operand ⟨.positive, .atom atom⟩))) := by
  intro same
  have frozen : Satisfies J (Reduct M (operand ⟨.doubleNegative, .atom atom⟩)) :=
    (operand_frozen M J _).mpr present
  have retained : M atom ∧ J atom := (operand_frozen M J _).mp (same.mp frozen)
  exact absent retained.2

inductive ChoiceKey (A : Type u) (O : Type w) where
  | atom : Sign → A → ChoiceKey A O
  | boolean : O → ChoiceKey A O
  deriving DecidableEq

def choiceKey (occurrence : O) (head : Operand A) : ChoiceKey A O :=
  match head.value with
  | .atom atom => .atom head.sign atom
  | .boolean _ => .boolean occurrence

/-- Same-sign atomic witnesses coalesce independently of source occurrence. -/
theorem atomic_witness_key (first second : O) (sign : Sign) (atom : A) :
    choiceKey first ⟨sign, .atom atom⟩ = choiceKey second ⟨sign, .atom atom⟩ := rfl

/-- Distinct signs remain distinct ordinary atomic contributions. -/
theorem atomic_sign_keys_distinct (first second : O) (left right : Sign)
    (different : left ≠ right) (atom : A) :
    choiceKey first ⟨left, .atom atom⟩ ≠ choiceKey second ⟨right, .atom atom⟩ := by
  intro same
  exact different (ChoiceKey.atom.inj same).1

/-- Boolean identity belongs to the occurrence, not its truth or sign. -/
theorem boolean_witness_key (occurrence : O) (left right : Sign) (a b : Bool) :
    choiceKey occurrence (⟨left, .boolean a⟩ : Operand A) =
      choiceKey occurrence ⟨right, .boolean b⟩ := rfl

/-- Separate source occurrences remain separate Boolean contributions. -/
theorem boolean_occurrence_keys_distinct (first second : O) (different : first ≠ second)
    (left right : Sign) (a b : Bool) :
    choiceKey first (⟨left, .boolean a⟩ : Operand A) ≠
      choiceKey second ⟨right, .boolean b⟩ := by
  intro same
  exact different (ChoiceKey.boolean.inj same)

structure Row (K : Type v) (A : Type u) where
  key : K
  head : Operand A
  eligible : Formula A

open Classical in
noncomputable def activity (rows : List (Row K A)) (key : K) : Formula A :=
  RuleFactorization.any ((rows.filter (fun row => row.key = key)).map
    (fun row => .conj (operand row.head) row.eligible))

/-- All witnesses of a key OR-coalesce their signed truth and eligibility. -/
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

/-- Frozen signed activity retains eligibility's reduct at J. A constant operand
    does not permit replacing the whole witness by its original truth. -/
theorem activity_frozen (M J : Atoms A) (rows : List (Row K A)) (key : K) :
    Satisfies J (Reduct M (activity rows key)) ↔
      ∃ row ∈ rows, row.key = key ∧ FrozenHolds M J row.head ∧
        Satisfies J (Reduct M row.eligible) := by
  classical
  simp only [activity, RuleFactorization.reduct_any, List.mem_map,
    List.mem_filter, decide_eq_true_eq]
  constructor
  · rintro ⟨_, ⟨row, ⟨member, same⟩, rfl⟩, truth⟩
    have parts := (RuleFactorization.reduct_conj M J _ _).mp truth
    exact ⟨row, member, same, (operand_frozen M J row.head).mp parts.1, parts.2⟩
  · rintro ⟨row, member, same, head, eligible⟩
    exact ⟨_, ⟨row, ⟨member, same⟩, rfl⟩,
      (RuleFactorization.reduct_conj M J _ _).mpr
        ⟨(operand_frozen M J row.head).mpr head, eligible⟩⟩

/-- Permission remains distinct from measured activity. -/
def permission (body eligible : Formula A) (head : Operand A) : Formula A :=
  .imp (.conj body eligible) (.disj (operand head) (Neg (operand head)))

/-- Every permission is classically true, whatever its operand sign. -/
theorem permission_original (M : Atoms A) (body eligible : Formula A)
    (head : Operand A) : Satisfies M (permission body eligible head) := by
  classical
  intro _
  exact Classical.em (Satisfies M (operand head))

/-- Neither default-negation form imposes retention of an atom in J. This is the
    semantic reason that signed permission supplies no positive atom support. -/
theorem signed_permission_frozen (M J : Atoms A) (body eligible : Formula A)
    (head : Operand A) (signed : head.sign ≠ .positive) :
    Satisfies J (Reduct M (permission body eligible head)) := by
  classical
  rw [permission, RuleFactorization.reduct_imp]
  refine ⟨permission_original M body eligible head, ?_⟩
  intro _
  rw [RuleFactorization.reduct_disj, negation_frozen]
  by_cases original : Holds M head
  · apply Or.inl
    apply (operand_frozen M J head).mpr
    rcases head with ⟨sign, value⟩
    cases sign with
    | positive => exact False.elim (signed rfl)
    | negative | doubleNegative => exact original
  · exact Or.inr (fun truth => original ((operand_original M head).mp truth))

/-- An unsigned row representation moves signed truth into eligibility and
    preserves its key. The new Boolean head introduces no atom permission. -/
def unsignedRow (row : Row K A) : BooleanHeadElements.Row K A :=
  match row.head.sign with
  | .positive => ⟨row.key, row.head.value, row.eligible⟩
  | .negative | .doubleNegative =>
    ⟨row.key, .boolean true, .conj (operand row.head) row.eligible⟩

/-- The embedding preserves original and arbitrary frozen row activity. -/
theorem row_activity_equivalent (row : Row K A) :
    Equivalent (.conj (operand row.head) row.eligible)
      (.conj (BooleanHeadElements.operand (unsignedRow row).head)
        (unsignedRow row).eligible) := by
  rcases row with ⟨key, ⟨sign, value⟩, eligible⟩
  cases sign with
  | positive => exact equivalent_refl _
  | negative | doubleNegative =>
    constructor
    · intro M
      simp only [unsignedRow, BooleanHeadElements.operand, Satisfies,
        BooleanHeads.constant_original, true_and]
    · intro M J
      simp only [unsignedRow, RuleFactorization.reduct_conj,
        BooleanHeadElements.operand, BooleanHeads.constant_frozen, true_and]

/-- Coalescing the same keys after embedding preserves their activity. -/
theorem activity_equivalent (rows : List (Row K A)) (key : K) :
    Equivalent (activity rows key)
      (BooleanHeadElements.activity (rows.map unsignedRow) key) := by
  have key_preserved (row : Row K A) : (unsignedRow row).key = row.key := by
    cases sign : row.head.sign <;> simp only [unsignedRow, sign]
  constructor
  · intro M
    classical
    simp only [activity, BooleanHeadElements.activity, RuleFactorization.satisfies_any,
      List.mem_map, List.mem_filter, decide_eq_true_eq]
    constructor
    · rintro ⟨_, ⟨row, ⟨member, same⟩, rfl⟩, truth⟩
      exact ⟨_, ⟨unsignedRow row, ⟨⟨row, member, rfl⟩, (key_preserved row).trans same⟩,
        rfl⟩, ((row_activity_equivalent row).1 M).mp truth⟩
    · rintro ⟨_, ⟨_, ⟨⟨row, member, rfl⟩, same⟩, rfl⟩, truth⟩
      exact ⟨_, ⟨row, ⟨member, (key_preserved row).symm.trans same⟩, rfl⟩,
        ((row_activity_equivalent row).1 M).mpr truth⟩
  · intro M J
    classical
    simp only [activity, BooleanHeadElements.activity, RuleFactorization.reduct_any,
      List.mem_map, List.mem_filter, decide_eq_true_eq]
    constructor
    · rintro ⟨_, ⟨row, ⟨member, same⟩, rfl⟩, truth⟩
      exact ⟨_, ⟨unsignedRow row, ⟨⟨row, member, rfl⟩, (key_preserved row).trans same⟩,
        rfl⟩, ((row_activity_equivalent row).2 M J).mp truth⟩
    · rintro ⟨_, ⟨_, ⟨⟨row, member, rfl⟩, same⟩, rfl⟩, truth⟩
      exact ⟨_, ⟨row, ⟨member, (key_preserved row).symm.trans same⟩, rfl⟩,
        ((row_activity_equivalent row).2 M J).mpr truth⟩

/-- Embedding signed operands changes no permission, under either semantics. -/
theorem permission_equivalent (body : Formula A) (row : Row K A) :
    Equivalent (permission body row.eligible row.head)
      (BooleanHeadElements.permission body (unsignedRow row).eligible (unsignedRow row).head) := by
  rcases row with ⟨key, ⟨sign, value⟩, eligible⟩
  cases sign with
  | positive => exact equivalent_refl _
  | negative | doubleNegative =>
    constructor
    · intro M
      exact ⟨fun _ => BooleanHeadElements.permission_original M body _ _,
        fun _ => permission_original M body eligible _⟩
    · intro M J
      exact ⟨fun _ => BooleanHeadElements.boolean_permission_frozen M J body _ true,
        fun _ => signed_permission_frozen M J body eligible _ (by intro impossible; cases impossible)⟩

/-- The permission index contains exactly unsigned atom rows. Eligibility is
    retained as a formula; neither signed atoms nor Booleans create producers. -/
theorem unsigned_atom_rows (rows : List (Row K A)) (atom : A) (eligible : Formula A) :
    (∃ row ∈ rows.map unsignedRow, row.head = .atom atom ∧ row.eligible = eligible) ↔
      ∃ row ∈ rows, row.head = ⟨.positive, .atom atom⟩ ∧ row.eligible = eligible := by
  constructor
  · rintro ⟨_, member, head, condition⟩
    obtain ⟨row, inside, rfl⟩ := List.mem_map.mp member
    rcases row with ⟨key, ⟨sign, value⟩, sourceEligible⟩
    cases sign with
    | positive =>
      exact ⟨_, inside, by simpa only [unsignedRow] using congrArg (Operand.mk .positive) head,
        condition⟩
    | negative | doubleNegative => cases head
  · rintro ⟨row, member, head, condition⟩
    refine ⟨unsignedRow row, List.mem_map.mpr ⟨row, member, rfl⟩, ?_, ?_⟩
    · simp only [unsignedRow, head]
    · simpa only [unsignedRow, head] using condition

/-- An arbitrary fixed guard covers count, both sums and both extrema. The
    caller still owes complete keys and the intended numeric/empty-value meaning. -/
noncomputable def formula (rows : List (Row K A)) (keys : List K)
    (accepts : AggregateReduct.Mask keys.length → Bool) : Formula A :=
  AggregateReduct.aggregate (fun index => activity rows keys[index]) accepts
    (AggregateReduct.masks keys.length)

/-- Canonical aggregate evaluation is unchanged by the signed-row embedding. -/
theorem formula_equivalent (rows : List (Row K A)) (keys : List K)
    (accepts : AggregateReduct.Mask keys.length → Bool) :
    Equivalent (formula rows keys accepts)
      (BooleanHeadElements.formula (rows.map unsignedRow) keys accepts) := by
  have original_mask (M : Atoms A) :
      AggregateReduct.active M (fun index : Fin keys.length => activity rows keys[index]) =
        AggregateReduct.active M (fun index : Fin keys.length =>
          BooleanHeadElements.activity (rows.map unsignedRow) keys[index]) := by
    classical
    funext index
    simp only [AggregateReduct.active, (activity_equivalent rows keys[index]).1 M]
  have frozen_mask (M J : Atoms A) :
      AggregateReduct.active J (fun index : Fin keys.length => Reduct M (activity rows keys[index])) =
        AggregateReduct.active J (fun index : Fin keys.length =>
          Reduct M (BooleanHeadElements.activity (rows.map unsignedRow) keys[index])) := by
    classical
    funext index
    simp only [AggregateReduct.active, (activity_equivalent rows keys[index]).2 M J]
  constructor
  · intro M
    simp only [formula, BooleanHeadElements.formula,
      AggregateReduct.original _ _ _ _ (AggregateReduct.masks_complete _), original_mask M]
  · intro M J
    simp only [formula, BooleanHeadElements.formula,
      AggregateReduct.frozen _ _ _ _ _ (AggregateReduct.masks_complete _),
      original_mask M, frozen_mask M J]

noncomputable def group (body : Formula A) (rows : List (Row K A)) (keys : List K)
    (accepts : AggregateReduct.Mask keys.length → Bool) : Formula A :=
  .conj (all (rows.map (fun row => permission body row.eligible row.head)))
    (HeadMeasures.bound body (formula rows keys accepts))

/-- The entire head group preserves truth and reduct truth, retaining signed
    numeric activity independently of its positive-only permission index. -/
theorem group_equivalent (body : Formula A) (rows : List (Row K A)) (keys : List K)
    (accepts : AggregateReduct.Mask keys.length → Bool) :
    Equivalent (group body rows keys accepts)
      (BooleanHeadElements.group body (rows.map unsignedRow) keys accepts) := by
  have permissions : Equivalent
      (all (rows.map (fun row => permission body row.eligible row.head)))
      (BooleanHeadElements.permissions body (rows.map unsignedRow)) := by
    simp only [BooleanHeadElements.permissions, List.map_map]
    exact all_map_equivalent rows _ _ (permission_equivalent body)
  exact equivalent_conj _ _ _ _ permissions
    (HeadMeasures.bound_equivalent body _ _ (formula_equivalent rows keys accepts).1)

/-- The embedding preserves answer sets in any surrounding theory. Existing
    unsigned coalescing laws can then index only positive atom permissions. -/
theorem group_in_context (M : Atoms A) (body : Formula A) (rows : List (Row K A))
    (keys : List K) (accepts : AggregateReduct.Mask keys.length → Bool)
    (context : Theory A) :
    Stable M (group body rows keys accepts :: context) ↔
      Stable M (BooleanHeadElements.group body (rows.map unsignedRow) keys accepts :: context) := by
  have preservation := group_equivalent body rows keys accepts
  simp only [Stable, models_cons, ReductTheory, List.map_cons, preservation.1, preservation.2]

/-- A carrier need cover only unsigned atomic permissions. Signed operands
    remain in the independent numeric activity and do not enlarge this index. -/
theorem coalesced_group_in_context (M : Atoms A) (body : Formula A)
    (rows : List (Row K A)) (keys : List K)
    (accepts : AggregateReduct.Mask keys.length → Bool) (heads : List A)
    (coverage : ∀ row ∈ rows, ∀ atom, row.head = ⟨.positive, .atom atom⟩ → atom ∈ heads)
    (context : Theory A) :
    Stable M (group body rows keys accepts :: context) ↔
      Stable M (HeadMeasures.group body heads
        (BooleanHeadElements.eligibility (rows.map unsignedRow))
        (BooleanHeadElements.formula (rows.map unsignedRow) keys accepts) :: context) := by
  have unsigned_coverage : ∀ row ∈ rows.map unsignedRow, ∀ atom,
      row.head = .atom atom → atom ∈ heads := by
    intro row member atom head
    obtain ⟨source, inside, positive, _⟩ :=
      (unsigned_atom_rows rows atom row.eligible).mp ⟨row, member, head, rfl⟩
    exact coverage source inside atom positive
  exact (group_in_context M body rows keys accepts context).trans
    (BooleanHeadElements.coalesced_group_in_context M body (rows.map unsignedRow)
      keys accepts heads unsigned_coverage context)

/-- A head without unsigned atomic operands only filters the surrounding
    answer sets. Signed activity and eligibility cannot manufacture support. -/
theorem nonproducing_group_in_context (M : Atoms A) (body : Formula A)
    (rows : List (Row K A)) (keys : List K)
    (accepts : AggregateReduct.Mask keys.length → Bool)
    (nonproducing : ∀ row ∈ rows, ∀ atom, row.head ≠ ⟨.positive, .atom atom⟩)
    (context : Theory A) :
    Stable M (group body rows keys accepts :: context) ↔
      (Satisfies M body → Satisfies M (formula rows keys accepts)) ∧ Stable M context := by
  have booleans : ∀ row ∈ rows.map unsignedRow, ∃ value, row.head = .boolean value := by
    intro row member
    obtain ⟨source, inside, rfl⟩ := List.mem_map.mp member
    rcases source with ⟨key, ⟨sign, value⟩, eligible⟩
    cases sign with
    | negative | doubleNegative => exact ⟨true, rfl⟩
    | positive =>
      cases value with
      | atom atom => exact False.elim (nonproducing _ inside atom rfl)
      | boolean value => exact ⟨value, rfl⟩
  have embedded := BooleanHeadElements.boolean_group_in_context M body
    (rows.map unsignedRow) keys accepts booleans context
  have numeric := (formula_equivalent rows keys accepts).1 M
  rw [group_in_context, embedded, ← numeric]

end Zetesis.SignedHeadElements
