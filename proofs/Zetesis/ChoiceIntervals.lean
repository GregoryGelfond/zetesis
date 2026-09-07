import Zetesis.RuleFactorization
import Zetesis.AggregateAssignment

/-!
# Closed intervals inside one choice group

The executable product enumerates each interval argument occurrence independently.
Its raw head/condition rows implement an independently stated relational binding
contract. Grouping retains every condition as an OR, counts each head once, and
preserves original truth and every frozen M/J reduct under explicit finite-table
coverage. Conditions are arbitrary Ferraris formulas, including recursive and
default-negated conditions. Empty products do not delete the surrounding bound.

This is a denotational finite expansion law. It does not verify source recognition,
scope safety, Rust cursors, machine-width arithmetic, resource bounds, support
completion, objective presence, or the Rust cardinality/DAG constructors.
-/

namespace Zetesis.ChoiceIntervals

open Ferraris
open RuleFactorization (any)

universe u v
variable {α : Type u} {κ : Type v}

structure Interval where
  lower : Int
  upper : Int

/-- Inclusive mathematical integer range; a reversed interval has no members. -/
def interval (range : Interval) : List Int :=
  (List.range (range.upper - range.lower + 1).toNat).map
    (fun (offset : Nat) => range.lower + (offset : Int))

theorem mem_interval (range : Interval) (value : Int) :
    value ∈ interval range ↔ range.lower ≤ value ∧ value ≤ range.upper := by
  simp only [interval, List.mem_map, List.mem_range]
  constructor
  · rintro ⟨offset, inside, rfl⟩
    omega
  · intro ⟨lower, upper⟩
    refine ⟨(value - range.lower).toNat, ?_, ?_⟩ <;> omega

theorem reversed_interval_empty (range : Interval) (reversed : range.upper < range.lower) :
    interval range = [] := by
  apply List.eq_nil_iff_forall_not_mem.mpr
  intro value member
  obtain ⟨lower, upper⟩ := (mem_interval range value).mp member
  omega

/-- Each list position is one independent fresh argument occurrence. -/
def product : List Interval → List (List Int)
  | [] => [[]]
  | range :: ranges => (interval range).flatMap
      (fun value => (product ranges).map (fun values => value :: values))

/-- Independent relational specification: one bound value per argument slot. -/
def Binds : List Interval → List Int → Prop
  | [], [] => True
  | range :: ranges, value :: values =>
      (range.lower ≤ value ∧ value ≤ range.upper) ∧ Binds ranges values
  | _, _ => False

theorem product_complete (ranges : List Interval) (values : List Int) :
    values ∈ product ranges ↔ Binds ranges values := by
  induction ranges generalizing values with
  | nil => cases values <;> simp [product, Binds]
  | cons range ranges ih =>
    cases values with
    | nil => simp [product, Binds]
    | cons value values =>
      simp [product, List.mem_flatMap, mem_interval, Binds, ih]

theorem binding_length (ranges : List Interval) (values : List Int)
    (bound : Binds ranges values) : values.length = ranges.length := by
  induction ranges generalizing values with
  | nil => cases values <;> simp_all [Binds]
  | cons range ranges ih =>
    cases values with
    | nil => contradiction
    | cons value values => simpa using congrArg Nat.succ (ih values bound.2)

/-- Appending fresh values cannot alias an existing outer slot. -/
theorem fresh_slot_read (outer values : List Int) (slot : Nat) :
    (outer ++ values)[outer.length + slot]? = values[slot]? := by
  rw [List.getElem?_append_right (by omega)]
  simp

theorem fresh_slot_indices_distinct (outer : List Int) (first second : Nat)
    (different : first ≠ second) : outer.length + first ≠ outer.length + second := by
  omega

/-- In particular two identical interval spellings do not share one variable. -/
theorem independent_occurrences_include_off_diagonal :
    [1, 2] ∈ product [⟨1, 2⟩, ⟨1, 2⟩] ∧
      [2, 1] ∈ product [⟨1, 2⟩, ⟨1, 2⟩] := by
  decide

structure Element (α : Type u) where
  ranges : List Interval
  head : List Int → α
  condition : List Int → Formula α

structure Row (α : Type u) where
  head : α
  condition : Formula α

/-- Expand local argument bindings without creating additional choice groups. -/
def expand (elements : List (Element α)) : List (Row α) :=
  elements.flatMap (fun element => (product element.ranges).map
    (fun values => ⟨element.head values, element.condition values⟩))

/-- A conceptual explicit expansion is independently specified by relational
    bindings; this contract is not equality with the executable product. -/
def Represents (elements : List (Element α)) (rows : List (Row α)) : Prop :=
  ∀ row, row ∈ rows ↔ ∃ element ∈ elements, ∃ values,
    Binds element.ranges values ∧
      element.head values = row.head ∧ element.condition values = row.condition

theorem expansion_represents (elements : List (Element α)) :
    Represents elements (expand elements) := by
  intro row
  simp only [expand, List.mem_flatMap, List.mem_map, product_complete]
  constructor
  · rintro ⟨element, member, values, bound, same⟩
    cases same
    exact ⟨element, member, values, bound, rfl, rfl⟩
  · rintro ⟨element, member, values, bound, head, condition⟩
    exact ⟨element, member, values, bound, by cases row; simp_all⟩

section Eligibility

variable [DecidableEq α]

/-- All alternatives for a complete head atom; no classical simplification of
    their OR is allowed before freezing a candidate. -/
def eligibility (head : α) (rows : List (Row α)) : Formula α :=
  any ((rows.filter (fun row => decide (row.head = head))).map Row.condition)

theorem eligibility_classical (M : Atoms α) (head : α) (rows : List (Row α)) :
    Satisfies M (eligibility head rows) ↔
      ∃ row ∈ rows, row.head = head ∧ Satisfies M row.condition := by
  simp only [eligibility, RuleFactorization.satisfies_any, List.mem_map,
    List.mem_filter, decide_eq_true_eq]
  constructor
  · rintro ⟨condition, ⟨row, ⟨member, same⟩, rfl⟩, truth⟩
    exact ⟨row, member, same, truth⟩
  · rintro ⟨row, member, same, truth⟩
    exact ⟨row.condition, ⟨row, ⟨member, same⟩, rfl⟩, truth⟩

theorem eligibility_frozen (M J : Atoms α) (head : α) (rows : List (Row α)) :
    Satisfies J (Reduct M (eligibility head rows)) ↔
      ∃ row ∈ rows, row.head = head ∧ Satisfies J (Reduct M row.condition) := by
  simp only [eligibility, RuleFactorization.reduct_any, List.mem_map,
    List.mem_filter, decide_eq_true_eq]
  constructor
  · rintro ⟨condition, ⟨row, ⟨member, same⟩, rfl⟩, truth⟩
    exact ⟨row, member, same, truth⟩
  · rintro ⟨row, member, same, truth⟩
    exact ⟨row.condition, ⟨row, ⟨member, same⟩, rfl⟩, truth⟩

theorem expanded_eligibility_classical (M : Atoms α) (head : α)
    (elements : List (Element α)) :
    Satisfies M (eligibility head (expand elements)) ↔
      ∃ element ∈ elements, ∃ values, Binds element.ranges values ∧
        element.head values = head ∧ Satisfies M (element.condition values) := by
  rw [eligibility_classical]
  constructor
  · rintro ⟨row, member, same, truth⟩
    obtain ⟨element, inside, values, bound, headEq, conditionEq⟩ :=
      (expansion_represents elements row).mp member
    exact ⟨element, inside, values, bound, headEq.trans same, conditionEq ▸ truth⟩
  · rintro ⟨element, member, values, bound, same, truth⟩
    exact ⟨⟨element.head values, element.condition values⟩,
      (expansion_represents elements _).mpr ⟨element, member, values, bound, rfl, rfl⟩,
      same, truth⟩

theorem expanded_eligibility_frozen (M J : Atoms α) (head : α)
    (elements : List (Element α)) :
    Satisfies J (Reduct M (eligibility head (expand elements))) ↔
      ∃ element ∈ elements, ∃ values, Binds element.ranges values ∧
        element.head values = head ∧ Satisfies J (Reduct M (element.condition values)) := by
  rw [eligibility_frozen]
  constructor
  · rintro ⟨row, member, same, truth⟩
    obtain ⟨element, inside, values, bound, headEq, conditionEq⟩ :=
      (expansion_represents elements row).mp member
    exact ⟨element, inside, values, bound, headEq.trans same, conditionEq ▸ truth⟩
  · rintro ⟨element, member, values, bound, same, truth⟩
    exact ⟨⟨element.head values, element.condition values⟩,
      (expansion_represents elements _).mpr ⟨element, member, values, bound, rfl, rfl⟩,
      same, truth⟩

end Eligibility

/-- Strong equivalence here states original truth and every frozen pair directly;
    neither direction assumes that J is a subset of M. -/
def Equivalent (left right : Formula α) : Prop :=
  (∀ M, Satisfies M left ↔ Satisfies M right) ∧
    ∀ M J, Satisfies J (Reduct M left) ↔ Satisfies J (Reduct M right)

theorem equivalent_refl (F : Formula α) : Equivalent F F :=
  ⟨fun _ => Iff.rfl, fun _ _ => Iff.rfl⟩

theorem equivalent_conj (F G F' G' : Formula α)
    (left : Equivalent F F') (right : Equivalent G G') :
    Equivalent (.conj F G) (.conj F' G') := by
  constructor
  · intro M
    exact and_congr (left.1 M) (right.1 M)
  · intro M J
    simp only [RuleFactorization.reduct_conj, left.2 M J, right.2 M J]

theorem equivalent_disj (F G F' G' : Formula α)
    (left : Equivalent F F') (right : Equivalent G G') :
    Equivalent (.disj F G) (.disj F' G') := by
  constructor
  · intro M
    exact or_congr (left.1 M) (right.1 M)
  · intro M J
    simp only [RuleFactorization.reduct_disj, left.2 M J, right.2 M J]

theorem equivalent_imp (F G F' G' : Formula α)
    (left : Equivalent F F') (right : Equivalent G G') :
    Equivalent (.imp F G) (.imp F' G') := by
  constructor
  · intro M
    exact imp_congr (left.1 M) (right.1 M)
  · intro M J
    simp only [RuleFactorization.reduct_imp, Satisfies, left.1 M, right.1 M,
      left.2 M J, right.2 M J]

/-- Formula contexts retain implication and negation, rather than treating
    classical equivalence of eligibility expressions as sufficient. -/
def substitute (F : Formula κ) (replacement : κ → Formula α) : Formula α :=
  match F with
  | .atom key => replacement key
  | .bot => .bot
  | .conj left right => .conj (substitute left replacement) (substitute right replacement)
  | .disj left right => .disj (substitute left replacement) (substitute right replacement)
  | .imp left right => .imp (substitute left replacement) (substitute right replacement)

theorem substitute_equivalent (context : Formula κ) (left right : κ → Formula α)
    (leaves : ∀ key, Equivalent (left key) (right key)) :
    Equivalent (substitute context left) (substitute context right) := by
  induction context with
  | atom key => exact leaves key
  | bot => exact equivalent_refl _
  | conj F G ihF ihG => exact equivalent_conj _ _ _ _ ihF ihG
  | disj F G ihF ihG => exact equivalent_disj _ _ _ _ ihF ihG
  | imp F G ihF ihG => exact equivalent_imp _ _ _ _ ihF ihG

theorem represented_eligibility_equivalent [DecidableEq α] (elements : List (Element α))
    (rows : List (Row α)) (represented : Represents elements rows) (head : α) :
    Equivalent (eligibility head (expand elements)) (eligibility head rows) := by
  have member : ∀ row, row ∈ expand elements ↔ row ∈ rows :=
    fun row => (expansion_represents elements row).trans (represented row).symm
  constructor
  · intro M
    simp only [eligibility_classical, member]
  · intro M J
    simp only [eligibility_frozen, member]

def top : Formula α := .imp .bot .bot

def all : List (Formula α) → Formula α
  | [] => top
  | first :: rest => .conj first (all rest)

/-- A finite threshold formula over whole head identities. Its leaves may be
    arbitrary formulas, so recursive eligibility is not evaluated away. -/
def atLeast : Nat → List (Formula α) → Formula α
  | 0, _ => top
  | _ + 1, [] => .bot
  | needed + 1, first :: rest =>
      .disj (.conj first (atLeast needed rest)) (atLeast (needed + 1) rest)

noncomputable def count (predicate : κ → Prop) (values : List κ) : Nat := by
  classical
  exact (values.filter (fun value => decide (predicate value))).length

theorem count_cons (predicate : κ → Prop) [DecidablePred predicate] (first : κ) (rest : List κ) :
    count predicate (first :: rest) =
      (if predicate first then 1 else 0) + count predicate rest := by
  classical
  by_cases truth : predicate first <;> simp [count, truth, Nat.add_comm]

theorem atLeast_classical (M : Atoms α) (needed : Nat) (formulas : List (Formula α)) :
    Satisfies M (atLeast needed formulas) ↔ needed ≤ count (Satisfies M) formulas := by
  classical
  induction formulas generalizing needed with
  | nil => cases needed <;> simp [atLeast, top, Satisfies, count]
  | cons first rest ih =>
    cases needed with
    | zero => simp [atLeast, top, Satisfies]
    | succ needed =>
      simp only [atLeast, Satisfies, ih, count_cons]
      by_cases truth : Satisfies M first <;> simp only [truth, ite_true, ite_false,
        true_and, false_and, false_or] <;> omega

theorem atLeast_frozen (M J : Atoms α) (needed : Nat) (formulas : List (Formula α)) :
    Satisfies J (Reduct M (atLeast needed formulas)) ↔
      needed ≤ count (fun F => Satisfies J (Reduct M F)) formulas := by
  classical
  induction formulas generalizing needed with
  | nil => cases needed <;> simp [atLeast, top, Reduct, Satisfies, count]
  | cons first rest ih =>
    cases needed with
    | zero => simp [atLeast, top, Reduct, Satisfies]
    | succ needed =>
      simp only [atLeast, RuleFactorization.reduct_disj, RuleFactorization.reduct_conj,
        ih, count_cons]
      by_cases truth : Satisfies J (Reduct M first) <;> simp only [truth, ite_true, ite_false,
        true_and, false_and, false_or] <;> omega

theorem all_classical (M : Atoms α) (formulas : List (Formula α)) :
    Satisfies M (all formulas) ↔ ∀ F ∈ formulas, Satisfies M F := by
  induction formulas with
  | nil => simp [all, top, Satisfies]
  | cons first rest ih => simp [all, Satisfies, ih]

theorem all_map_equivalent (keys : List κ) (left right : κ → Formula α)
    (leaves : ∀ key, Equivalent (left key) (right key)) :
    Equivalent (all (keys.map left)) (all (keys.map right)) := by
  induction keys with
  | nil => exact equivalent_refl _
  | cons key keys ih => exact equivalent_conj _ _ _ _ (leaves key) ih

theorem atLeast_map_equivalent (needed : Nat) (keys : List κ)
    (left right : κ → Formula α) (leaves : ∀ key, Equivalent (left key) (right key)) :
    Equivalent (atLeast needed (keys.map left)) (atLeast needed (keys.map right)) := by
  induction keys generalizing needed with
  | nil => exact equivalent_refl _
  | cons key keys ih =>
    cases needed with
    | zero => exact equivalent_refl _
    | succ needed =>
      exact equivalent_disj _ _ _ _
        (equivalent_conj _ _ _ _ (leaves key) (ih needed)) (ih (needed + 1))

/-- The lower and upper constraints surround the entire original group.
    Double negation makes the lower bound a candidate constraint, never support. -/
def group (body : Formula α) (lower upper : Nat) (heads : List α)
    (eligible : α → Formula α) : Formula α :=
  .imp body (.conj
    (all (heads.map (fun head =>
      .imp (eligible head) (.disj (.atom head) (Neg (.atom head))))))
    (.conj
      (Neg (Neg (atLeast lower (heads.map (fun head => .conj (.atom head) (eligible head))))))
      (Neg (atLeast (upper + 1)
        (heads.map (fun head => .conj (.atom head) (eligible head)))))))

theorem group_equivalent (body : Formula α) (lower upper : Nat) (heads : List α)
    (left right : α → Formula α) (leaves : ∀ head, Equivalent (left head) (right head)) :
    Equivalent (group body lower upper heads left) (group body lower upper heads right) := by
  have selected : ∀ head, Equivalent (.conj (.atom head) (left head))
      (.conj (.atom head) (right head)) :=
    fun head => equivalent_conj _ _ _ _ (equivalent_refl _) (leaves head)
  apply equivalent_imp _ _ _ _ (equivalent_refl _)
  apply equivalent_conj
  · apply all_map_equivalent
    intro head
    exact equivalent_imp _ _ _ _ (leaves head) (equivalent_refl _)
  · apply equivalent_conj
    · exact equivalent_imp _ _ _ _
        (equivalent_imp _ _ _ _
          (atLeast_map_equivalent lower heads _ _ selected) (equivalent_refl _))
        (equivalent_refl _)
    · exact equivalent_imp _ _ _ _
        (atLeast_map_equivalent (upper + 1) heads _ _ selected) (equivalent_refl _)

/-- Bounds count satisfied whole head/eligibility pairs. With a duplicate-free
    head carrier this is a cardinality of distinct selected atoms, not witnesses. -/
theorem group_classical (M : Atoms α) (body : Formula α) (lower upper : Nat)
    (heads : List α) (eligible : α → Formula α) :
    Satisfies M (group body lower upper heads eligible) ↔
      (Satisfies M body →
        lower ≤ count (fun head => M head ∧ Satisfies M (eligible head)) heads ∧
          count (fun head => M head ∧ Satisfies M (eligible head)) heads ≤ upper) := by
  classical
  have support : Satisfies M (all (heads.map (fun head =>
      Formula.imp (eligible head) (.disj (.atom head) (Neg (.atom head)))))) := by
    rw [all_classical]
    intro F member
    obtain ⟨head, _, rfl⟩ := List.mem_map.mp member
    exact fun _ => Classical.em (M head)
  have counted : count (Satisfies M)
      (heads.map (fun head => Formula.conj (.atom head) (eligible head))) =
      count (fun head => M head ∧ Satisfies M (eligible head)) heads := by
    simp only [count, List.filter_map, List.length_map, Function.comp_def, Satisfies]
    rfl
  simp only [group, Satisfies, support, true_and]
  simp only [Ferraris.Neg, Satisfies, atLeast_classical, counted]
  apply imp_congr Iff.rfl
  apply and_congr Classical.not_not
  constructor
  · intro notGreater
    exact Nat.le_of_not_gt (fun greater => notGreater (by omega))
  · intro bounded greater
    omega

section Groups

variable [DecidableEq α]

/-- One carrier position per complete head identity. -/
def headCarrier (rows : List (Row α)) : List α :=
  AggregateAssignment.unique (rows.map Row.head)

theorem headCarrier_nodup (rows : List (Row α)) : (headCarrier rows).Nodup :=
  AggregateAssignment.unique_nodup _

theorem mem_headCarrier (head : α) (rows : List (Row α)) :
    head ∈ headCarrier rows ↔ ∃ row ∈ rows, row.head = head := by
  simp only [headCarrier, AggregateAssignment.mem_unique, List.mem_map]

theorem carrier_covers_exactly (elements : List (Element α)) (head : α) :
    head ∈ headCarrier (expand elements) ↔
      ∃ element ∈ elements, ∃ values,
        Binds element.ranges values ∧ element.head values = head := by
  rw [mem_headCarrier]
  constructor
  · rintro ⟨row, member, same⟩
    obtain ⟨element, inside, values, bound, headEq, _⟩ :=
      (expansion_represents elements row).mp member
    exact ⟨element, inside, values, bound, headEq.trans same⟩
  · rintro ⟨element, member, values, bound, same⟩
    exact ⟨⟨element.head values, element.condition values⟩,
      (expansion_represents elements _).mpr ⟨element, member, values, bound, rfl, rfl⟩,
      same⟩

def expandedGroup (body : Formula α) (lower upper : Nat) (elements : List (Element α)) :
    Formula α :=
  group body lower upper (headCarrier (expand elements))
    (fun head => eligibility head (expand elements))

/-- The conceptual explicit table uses the same distinct complete head carrier;
    rows may be reordered or duplicated while satisfying the relational contract. -/
def explicitGroup (body : Formula α) (lower upper : Nat)
    (elements : List (Element α)) (rows : List (Row α)) : Formula α :=
  group body lower upper (headCarrier (expand elements)) (fun head => eligibility head rows)

theorem expansion_group_equivalent (body : Formula α) (lower upper : Nat)
    (elements : List (Element α)) (rows : List (Row α))
    (represented : Represents elements rows) :
    Equivalent (expandedGroup body lower upper elements)
      (explicitGroup body lower upper elements rows) :=
  group_equivalent _ _ _ _ _ _ (represented_eligibility_equivalent elements rows represented)

theorem expansion_group_classical (M : Atoms α) (body : Formula α) (lower upper : Nat)
    (elements : List (Element α)) (rows : List (Row α))
    (represented : Represents elements rows) :
    Satisfies M (expandedGroup body lower upper elements) ↔
      Satisfies M (explicitGroup body lower upper elements rows) :=
  (expansion_group_equivalent body lower upper elements rows represented).1 M

theorem expansion_group_frozen (M J : Atoms α) (body : Formula α) (lower upper : Nat)
    (elements : List (Element α)) (rows : List (Row α))
    (represented : Represents elements rows) :
    Satisfies J (Reduct M (expandedGroup body lower upper elements)) ↔
      Satisfies J (Reduct M (explicitGroup body lower upper elements rows)) :=
  (expansion_group_equivalent body lower upper elements rows represented).2 M J

/-- Replacing a group inside an unchanged theory preserves complete stable-model
    identity, including all proper-subset reduct checks. -/
theorem expansion_stable_in_context (M : Atoms α) (body : Formula α) (lower upper : Nat)
    (elements : List (Element α)) (rows : List (Row α))
    (represented : Represents elements rows) (context : Theory α) :
    Stable M (expandedGroup body lower upper elements :: context) ↔
      Stable M (explicitGroup body lower upper elements rows :: context) := by
  simp only [Stable, models_cons, ReductTheory, List.map_cons,
    expansion_group_classical M body lower upper elements rows represented,
    expansion_group_frozen M _ body lower upper elements rows represented]

end Groups

/-- One reversed occurrence empties the Cartesian product, wherever it occurs. -/
theorem product_empty_at (before suffix : List Interval) (range : Interval)
    (reversed : range.upper < range.lower) :
    product (before ++ range :: suffix) = [] := by
  induction before with
  | nil => simp [product, reversed_interval_empty range reversed]
  | cons first before ih => simp [product, ih]

theorem reversed_element_has_no_rows (element : Element α) (before suffix : List Interval)
    (range : Interval) (shape : element.ranges = before ++ range :: suffix)
    (reversed : range.upper < range.lower) : expand [element] = [] := by
  simp [expand, shape, product_empty_at before suffix range reversed]

/-- The element expansion may be empty, but a positive lower bound still
    forbids a true outer body, both before and after freezing. -/
theorem positive_empty_group_is_constraint (body : Formula α) (lower upper : Nat)
    (eligible : α → Formula α) :
    Equivalent (group body (lower + 1) upper [] eligible) (Neg body) := by
  classical
  constructor
  · intro M
    simp [group, all, atLeast, top, Ferraris.Neg, Satisfies]
  · intro M J
    simp [group, all, atLeast, top, Ferraris.Neg, Reduct, Satisfies]

/-- A reversed local interval removes its rows, not the original lower bound. -/
theorem positive_reversed_expansion_is_constraint [DecidableEq α]
    (body : Formula α) (lower upper : Nat) (element : Element α)
    (before after : List Interval) (range : Interval)
    (shape : element.ranges = before ++ range :: after)
    (reversed : range.upper < range.lower) :
    Equivalent (expandedGroup body (lower + 1) upper [element]) (Neg body) := by
  simp only [expandedGroup,
    reversed_element_has_no_rows element before after range shape reversed,
    headCarrier, List.map_nil, AggregateAssignment.unique]
  exact positive_empty_group_is_constraint _ _ _ _

section Duplicates

variable [DecidableEq α]

/-- Distinct witnesses for the same head are a disjunction of conditions. -/
theorem duplicate_head_eligibility (head : α) (left right : Formula α) :
    Equivalent (eligibility head [⟨head, left⟩, ⟨head, right⟩]) (.disj left right) := by
  constructor
  · intro M
    simp [eligibility, any, Satisfies]
  · intro M J
    rw [eligibility_frozen, RuleFactorization.reduct_disj]
    simp

theorem duplicate_head_has_one_position (head : α) (left right : Formula α) :
    headCarrier [⟨head, left⟩, ⟨head, right⟩] = [head] := by
  simp [headCarrier, AggregateAssignment.unique]

/-- Duplicating a raw witness does not affect the coalesced condition even in
    a reduct. Distinct eligibility formulas remain ORed, as above. -/
theorem repeated_row_eligibility (head : α) (row : Row α) (rows : List (Row α)) :
    Equivalent (eligibility head (row :: row :: rows)) (eligibility head (row :: rows)) := by
  constructor
  · intro M
    simp only [eligibility_classical, List.mem_cons, exists_eq_or_imp]
    simp only [or_self_left]
  · intro M J
    simp only [eligibility_frozen, List.mem_cons, exists_eq_or_imp]
    simp only [or_self_left]

/-- A classically tautological eligibility OR must retain its frozen meaning. -/
theorem tautological_eligibility_is_not_frozen_true (head : α) :
    ¬ Satisfies (fun _ => False) (Reduct (fun atom => atom = head)
      (eligibility head [⟨head, .atom head⟩, ⟨head, Neg (.atom head)⟩])) := by
  rw [(duplicate_head_eligibility head (.atom head) (Neg (.atom head))).2]
  simp [choice_reduct_guard]

end Duplicates

/-- Splitting a 1..1 group into two 1..1 singleton groups changes its models. -/
theorem splitting_bounded_group_changes_models :
    Satisfies (fun atom : Bool => atom = false)
      (group top 1 1 [false, true] (fun _ => top)) ∧
    ¬ Satisfies (fun atom : Bool => atom = false)
      (.conj (group top 1 1 [false] (fun _ => top))
        (group top 1 1 [true] (fun _ => top))) := by
  simp [group_classical, Satisfies, count, top]

/-- A negated upper-threshold constraint depends only on candidate truth. -/
theorem negation_frozen (M J : Atoms α) (F : Formula α) :
    Satisfies J (Reduct M (Neg F)) ↔ ¬ Satisfies M F := by
  classical
  by_cases truth : Satisfies M F <;> simp [Ferraris.Neg, Reduct, Satisfies, truth, RuleFactorization.false_reduct M F]

/-- Neither cardinality bound can support an atom in a reduct interpretation;
    both check the original candidate's active head/eligibility count only. -/
theorem cardinality_constraints_frozen (M J : Atoms α) (lower upper : Nat)
    (formulas : List (Formula α)) :
    Satisfies J (Reduct M (.conj (Neg (Neg (atLeast lower formulas)))
      (Neg (atLeast (upper + 1) formulas)))) ↔
      lower ≤ count (Satisfies M) formulas ∧ count (Satisfies M) formulas ≤ upper := by
  rw [RuleFactorization.reduct_conj, double_neg_formula_reduct, negation_frozen,
    atLeast_classical, atLeast_classical]
  apply and_congr Iff.rfl
  omega

/-- The recursive singleton group `1 {a:a} 1` has no stable model. A positive
    lower-threshold root would incorrectly supply support and invalidate this law. -/
theorem recursive_singleton_has_no_stable_model (M : Atoms Unit) :
    ¬ Stable M [group top 1 1 [()] (fun head => .atom head)] := by
  intro stable
  have present : M () := by
    have accepted := stable.1 _ (List.mem_singleton.mpr rfl)
    by_cases h : M ()
    · exact h
    · simp [group_classical, top, Satisfies, count, h] at accepted
  apply stable.2
  refine ⟨(fun _ => False), ⟨?_, ?_⟩, ?_⟩
  · intro atom impossible
    exact False.elim impossible
  · intro backwards
    exact backwards () present
  · simp [Models, ReductTheory, group, all, atLeast, top, Ferraris.Neg,
      Reduct, Satisfies, present]

/-- A head supported by another rule still does not count when this group's
    own eligibility is false. -/
theorem unrelated_head_does_not_meet_lower_bound :
    ¬ Satisfies (fun _ : Unit => True) (group top 1 1 [()] (fun _ => .bot)) := by
  simp [group_classical, top, Satisfies, count]

end Zetesis.ChoiceIntervals
