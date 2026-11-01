import Zetesis.Frontier
import Zetesis.WorldViews

/-!
# Folds along an axis

A question the solver answers is often a reduction along one axis of its work:
over the answer sets of a world view, over the leaves of a candidate's subset
query, over the candidates of an enumeration. Such a reduction is the fold of a
commutative monoid over the members' values, sometimes with an absorbing
element, a value that no further member can change. Two rules decide every such
fold, and this module states them once:

- **Order independence.** Folding a permutation of the members gives the same
  value (`fold_perm`), so the order in which a schedule delivers the members is
  not a property of the result. For an idempotent monoid the fold depends only on
  which members occur (`fold_same_members`), so a repeated member changes
  nothing either.
- **Early exit at absorption.** Once the accumulator is the absorbing element,
  no further member changes it (`foldFrom_absorbing`), so every extension of a
  prefix that reached it folds to it (`fold_decided_prefix`). Otherwise the
  fold's value is known only when every member has been folded.

Two corollaries place the rules. A walk of a coverage tree that empties its
frontier, whoever held its regions, folds its emitted leaves to the fold of the
tree's outputs (`walk_exhausted_fold`). Over a program's world view, its set of
answer sets, an idempotent fold's value is a property of the family: any two
lists that represent the world view fold alike (`world_view_fold_complete`), and
a list of answer sets whose fold reached the absorbing element decides the
world view's fold whatever answer sets remain unseen (`world_view_fold_decided`).
An empty world view is an inconsistency result
(`WorldViews.world_view_empty_iff_inconsistent`), not a fold value.

The reductions the world-view questions use are defined here with their laws:
`all` and `any` on Booleans (the epistemic K and M of a literal), `inter` and
`union` on sets of atoms (cautious and brave consequences), and `pair`, the
product, which carries the three-valued reading's two flags.

Not proved here, and obligations of the implementation: that a session's stream
represents the world view when its enumeration is exhausted
(`WorldViews.completed_capture_represents_world_view` states the premises); that
every folded member is an answer set
(`WorldViews.partial_capture_has_only_answer_sets`); and that each accumulator
computes its named reduction's operation and tests for its absorbing element.
-/

namespace Zetesis.AxisFolds

universe u v w

/-- A commutative monoid, the reduction along an axis: an associative and
commutative operation with an identity, `unit`. -/
structure Reduction (V : Type u) where
  op : V → V → V
  unit : V
  assoc : ∀ a b c, op (op a b) c = op a (op b c)
  comm : ∀ a b, op a b = op b a
  unit_op : ∀ a, op unit a = a

namespace Reduction

variable {V : Type u} {M : Type v} (r : Reduction V)

/-- Fold the members' values into `start`, left to right. -/
def foldFrom (value : M → V) (start : V) (members : List M) : V :=
  members.foldl (fun acc member => r.op acc (value member)) start

/-- The fold of the members' values from the identity. -/
def fold (value : M → V) (members : List M) : V :=
  r.foldFrom value r.unit members

/-- `absorber` absorbs every value: combining anything with it gives it back. -/
def Absorbing (absorber : V) : Prop :=
  ∀ a, r.op a absorber = absorber

/-- Every value absorbs itself, so a repeated member changes no fold. -/
def Idempotent : Prop :=
  ∀ a, r.op a a = a

/-- The identity is also a right identity, by commutativity. -/
theorem op_unit (a : V) : r.op a r.unit = a := by
  rw [r.comm]
  exact r.unit_op a

/-- Folding from any start is that start combined with the fold from the
identity. This is the one algebraic fact the two rules rest on: a fold is the
monoid's product of its members' values, however it is bracketed.

Proof outline: by induction on the members, with the start generalized. The
empty fold leaves the start, which is the start combined with the identity. A
nonempty fold first combines the start with the head's value; the hypothesis
for the tail rewrites both this fold and the fold from the identity, and
associativity matches them. -/
theorem foldFrom_eq_op (value : M → V) (start : V) (members : List M) :
    r.foldFrom value start members = r.op start (r.fold value members) := by
  induction members generalizing start with
  | nil =>
    show start = r.op start r.unit
    exact (r.op_unit start).symm
  | cons head rest ih =>
    have stepped : r.foldFrom value start (head :: rest)
        = r.op (r.op start (value head)) (r.fold value rest) :=
      ih (r.op start (value head))
    have fromUnit : r.fold value (head :: rest)
        = r.op (r.op r.unit (value head)) (r.fold value rest) :=
      ih (r.op r.unit (value head))
    rw [stepped, fromUnit, r.unit_op, r.assoc]

/-- The fold of a nonempty list combines the head's value with the tail's fold. -/
theorem fold_cons (value : M → V) (head : M) (rest : List M) :
    r.fold value (head :: rest) = r.op (value head) (r.fold value rest) := by
  have expanded : r.fold value (head :: rest)
      = r.op (r.op r.unit (value head)) (r.fold value rest) :=
    r.foldFrom_eq_op value (r.op r.unit (value head)) rest
  rw [expanded, r.unit_op]

/-- The fold of a concatenation combines the two parts' folds. -/
theorem fold_append (value : M → V) (first second : List M) :
    r.fold value (first ++ second) = r.op (r.fold value first) (r.fold value second) := by
  have split : r.fold value (first ++ second)
      = r.foldFrom value (r.fold value first) second := by
    simp only [fold, foldFrom, List.foldl_append]
  rw [split, r.foldFrom_eq_op]

/-- **Order independence.** Folding a permutation of the members gives the same
value: the order in which a schedule delivers them is not a property of the
fold.

Proof outline: by induction on the permutation. Prepending one member to both
sides, or chaining two permutations, preserves the equality; exchanging the
first two members is associativity and commutativity of the operation. -/
theorem fold_perm (value : M → V) {first second : List M} (same : first.Perm second) :
    r.fold value first = r.fold value second := by
  induction same with
  | nil => rfl
  | cons head _ ih =>
    rw [r.fold_cons, r.fold_cons, ih]
  | swap x y rest =>
    simp only [r.fold_cons]
    rw [← r.assoc, ← r.assoc, r.comm (value y) (value x)]
  | trans _ _ first_eq second_eq =>
    exact first_eq.trans second_eq

/-- A fold depends only on its members' values: two value functions that agree
on every member fold alike. -/
theorem fold_congr {value other : M → V} {members : List M}
    (agree : ∀ m ∈ members, value m = other m) :
    r.fold value members = r.fold other members := by
  induction members with
  | nil => rfl
  | cons head rest ih =>
    have headAgrees : value head = other head := agree head (List.mem_cons_self ..)
    have restAgrees : ∀ m ∈ rest, value m = other m :=
      fun m member => agree m (List.mem_cons_of_mem head member)
    rw [r.fold_cons, r.fold_cons, headAgrees, ih restAgrees]

/-- Folding the images of the members is folding the members' composed values. -/
theorem fold_map {N : Type w} (value : N → V) (image : M → N) (members : List M) :
    r.fold value (members.map image) = r.fold (fun m => value (image m)) members := by
  induction members with
  | nil => rfl
  | cons head rest ih => rw [List.map_cons, r.fold_cons, r.fold_cons, ih]

/-- Folding the identity for every member gives the identity. -/
theorem fold_unit (members : List M) : r.fold (fun _ => r.unit) members = r.unit := by
  induction members with
  | nil => rfl
  | cons head rest ih => rw [r.fold_cons, ih, r.unit_op]

/-- **Absorption.** Folding any further members into an absorbing accumulator
leaves it unchanged. -/
theorem foldFrom_absorbing (value : M → V) {absorber : V} (absorbing : r.Absorbing absorber)
    (rest : List M) : r.foldFrom value absorber rest = absorber := by
  rw [r.foldFrom_eq_op, r.comm]
  exact absorbing (r.fold value rest)

/-- **Early exit.** A prefix whose fold reached the absorbing element decides
the fold of every extension of it: the remaining members need not be folded. -/
theorem fold_decided_prefix (value : M → V) {absorber : V} (absorbing : r.Absorbing absorber)
    {seen : List M} (decided : r.fold value seen = absorber) (rest : List M) :
    r.fold value (seen ++ rest) = absorber := by
  rw [r.fold_append, decided, r.comm]
  exact absorbing (r.fold value rest)

/-- For an idempotent reduction, a fold already contains each of its members'
values: combining one of them with the fold changes nothing.

Proof outline: by induction on the members. When the member is the head, the
head's value combined with itself is the head's value; when it lies in the
tail, commutativity brings it beside the tail's fold, which absorbs it by the
hypothesis. -/
theorem fold_absorbs_member (value : M → V) (idempotent : r.Idempotent)
    {members : List M} {member : M} (present : member ∈ members) :
    r.op (value member) (r.fold value members) = r.fold value members := by
  induction members with
  | nil => exact (List.not_mem_nil present).elim
  | cons head rest ih =>
    rw [r.fold_cons]
    rcases List.mem_cons.mp present with same | later
    · subst same
      rw [← r.assoc, idempotent]
    · rw [← r.assoc, r.comm (value member) (value head), r.assoc, ih later]

/-- For an idempotent reduction, the fold of a list absorbs the fold of any list
whose members it contains. -/
theorem fold_absorbs_contained (value : M → V) (idempotent : r.Idempotent)
    {big small : List M} (inside : ∀ m, m ∈ small → m ∈ big) :
    r.op (r.fold value small) (r.fold value big) = r.fold value big := by
  induction small with
  | nil => exact r.unit_op (r.fold value big)
  | cons head rest ih =>
    have restInside : ∀ m, m ∈ rest → m ∈ big :=
      fun m mem => inside m (List.mem_cons.mpr (Or.inr mem))
    have headInside : head ∈ big :=
      inside head (List.mem_cons.mpr (Or.inl rfl))
    rw [r.fold_cons, r.assoc, ih restInside,
      r.fold_absorbs_member value idempotent headInside]

/-- **Members alone.** For an idempotent reduction, two lists with the same
members fold alike, whatever their order and repetitions.

Proof outline: each list's fold absorbs the other's, since each contains the
other's members; commutativity then equates the two. -/
theorem fold_same_members (value : M → V) (idempotent : r.Idempotent)
    {first second : List M} (same : ∀ m, m ∈ first ↔ m ∈ second) :
    r.fold value first = r.fold value second := by
  have firstInSecond : ∀ m, m ∈ first → m ∈ second := fun m mem => (same m).mp mem
  have secondInFirst : ∀ m, m ∈ second → m ∈ first := fun m mem => (same m).mpr mem
  calc r.fold value first
      = r.op (r.fold value second) (r.fold value first) :=
        (r.fold_absorbs_contained value idempotent secondInFirst).symm
    _ = r.op (r.fold value first) (r.fold value second) := r.comm _ _
    _ = r.fold value second := r.fold_absorbs_contained value idempotent firstInSecond

end Reduction

/-! ## The reductions of the world-view questions -/

/-- "In every member": Boolean conjunction from `true`. `false` absorbs it; the
epistemic K of a literal folds whether each answer set holds the literal. -/
def all : Reduction Bool where
  op := fun a b => a && b
  unit := true
  assoc := fun a b c => by cases a <;> cases b <;> cases c <;> rfl
  comm := fun a b => by cases a <;> cases b <;> rfl
  unit_op := fun a => by cases a <;> rfl

theorem all_absorbing : all.Absorbing false := fun a => by cases a <;> rfl

theorem all_idempotent : all.Idempotent := fun a => by cases a <;> rfl

/-- "In some member": Boolean disjunction from `false`. `true` absorbs it; the
epistemic M of a literal folds whether each answer set holds the literal. -/
def any : Reduction Bool where
  op := fun a b => a || b
  unit := false
  assoc := fun a b c => by cases a <;> cases b <;> cases c <;> rfl
  comm := fun a b => by cases a <;> cases b <;> rfl
  unit_op := fun a => by cases a <;> rfl

theorem any_absorbing : any.Absorbing true := fun a => by cases a <;> rfl

theorem any_idempotent : any.Idempotent := fun a => by cases a <;> rfl

/-- Conjunction reduction is true exactly when every listed value is true.
The list retains repeated occurrences; no distinctness premise is needed. -/
theorem all_fold_true_iff {α : Type w} (value : α → Bool) (members : List α) :
    all.fold value members = true ↔ ∀ member ∈ members, value member = true := by
  induction members with
  | nil =>
    simp [Reduction.fold, Reduction.foldFrom, all]
  | cons first rest induction =>
    have step : all.fold value (first :: rest) = true ↔
        value first = true ∧ all.fold value rest = true := by
      rw [Reduction.fold_cons]
      change (value first && all.fold value rest) = true ↔ _
      simp only [Bool.and_eq_true]
    have sound : all.fold value (first :: rest) = true →
        ∀ member ∈ first :: rest, value member = true := by
      intro folded member listed
      have parts := step.mp folded
      rcases List.mem_cons.mp listed with same | later
      · exact same.symm ▸ parts.1
      · exact induction.mp parts.2 member later
    have complete : (∀ member ∈ first :: rest, value member = true) →
        all.fold value (first :: rest) = true := by
      intro each
      exact step.mpr ⟨each first List.mem_cons_self,
        induction.mpr (fun member listed => each member (List.mem_cons_of_mem first listed))⟩
    exact ⟨sound, complete⟩

/-- Disjunction reduction is true exactly when a listed value is true.
The empty list contributes false; repeated producers leave this law unchanged. -/
theorem any_fold_true_iff {α : Type w} (value : α → Bool) (members : List α) :
    any.fold value members = true ↔ ∃ member ∈ members, value member = true := by
  induction members with
  | nil =>
    simp [Reduction.fold, Reduction.foldFrom, any]
  | cons first rest induction =>
    have step : any.fold value (first :: rest) = true ↔
        value first = true ∨ any.fold value rest = true := by
      rw [Reduction.fold_cons]
      change (value first || any.fold value rest) = true ↔ _
      simp only [Bool.or_eq_true]
    have sound : any.fold value (first :: rest) = true →
        ∃ member ∈ first :: rest, value member = true := by
      intro folded
      rcases step.mp folded with atFirst | later
      · exact ⟨first, List.mem_cons_self, atFirst⟩
      · obtain ⟨member, listed, present⟩ := induction.mp later
        exact ⟨member, List.mem_cons_of_mem first listed, present⟩
    have complete : (∃ member ∈ first :: rest, value member = true) →
        any.fold value (first :: rest) = true := by
      rintro ⟨member, listed, present⟩
      rcases List.mem_cons.mp listed with same | later
      · exact step.mpr (Or.inl (same ▸ present))
      · exact step.mpr (Or.inr (induction.mpr ⟨member, later, present⟩))
    exact ⟨sound, complete⟩

/-- Intersection of sets of atoms, from the set of every atom. The empty set
absorbs it; cautious consequences fold each answer set's atoms. -/
def inter (α : Type w) : Reduction (Atoms α) where
  op := fun s t => fun a => s a ∧ t a
  unit := fun _ => True
  assoc := fun _ _ _ => funext fun _ =>
    propext ⟨fun ⟨⟨x, y⟩, z⟩ => ⟨x, y, z⟩, fun ⟨x, y, z⟩ => ⟨⟨x, y⟩, z⟩⟩
  comm := fun _ _ => funext fun _ =>
    propext ⟨fun ⟨x, y⟩ => ⟨y, x⟩, fun ⟨x, y⟩ => ⟨y, x⟩⟩
  unit_op := fun _ => funext fun _ =>
    propext ⟨fun ⟨_, y⟩ => y, fun y => ⟨trivial, y⟩⟩

theorem inter_absorbing (α : Type w) : (inter α).Absorbing (fun _ => False) :=
  fun _ => funext fun _ => propext ⟨fun ⟨_, f⟩ => f, fun f => f.elim⟩

theorem inter_idempotent (α : Type w) : (inter α).Idempotent :=
  fun _ => funext fun _ => propext ⟨fun ⟨x, _⟩ => x, fun x => ⟨x, x⟩⟩

/-- Union of sets of atoms, from the empty set. The set of every atom absorbs
it; brave consequences fold each answer set's atoms. -/
def union (α : Type w) : Reduction (Atoms α) where
  op := fun s t => fun a => s a ∨ t a
  unit := fun _ => False
  assoc := fun _ _ _ => funext fun _ =>
    propext ⟨fun h => h.elim (fun st => st.elim Or.inl (fun t => Or.inr (Or.inl t)))
        (fun u => Or.inr (Or.inr u)),
      fun h => h.elim (fun s => Or.inl (Or.inl s))
        (fun tu => tu.elim (fun t => Or.inl (Or.inr t)) Or.inr)⟩
  comm := fun _ _ => funext fun _ =>
    propext ⟨fun h => h.elim Or.inr Or.inl, fun h => h.elim Or.inr Or.inl⟩
  unit_op := fun _ => funext fun _ =>
    propext ⟨fun h => h.elim False.elim id, Or.inr⟩

theorem union_absorbing (α : Type w) : (union α).Absorbing (fun _ => True) :=
  fun _ => funext fun _ => propext ⟨fun _ => trivial, fun t => Or.inr t⟩

theorem union_idempotent (α : Type w) : (union α).Idempotent :=
  fun _ => funext fun _ => propext ⟨fun h => h.elim id id, Or.inl⟩

/-- The product of two reductions, componentwise. The three-valued reading of a
query folds two flags at once: whether some answer set leaves the query not
true, and whether some leaves it not false. -/
def pair {V : Type u} {W : Type v} (left : Reduction V) (right : Reduction W) :
    Reduction (V × W) where
  op := fun a b => (left.op a.1 b.1, right.op a.2 b.2)
  unit := (left.unit, right.unit)
  assoc := fun a b c => by
    show (left.op (left.op a.1 b.1) c.1, right.op (right.op a.2 b.2) c.2)
      = (left.op a.1 (left.op b.1 c.1), right.op a.2 (right.op b.2 c.2))
    rw [left.assoc, right.assoc]
  comm := fun a b => by
    show (left.op a.1 b.1, right.op a.2 b.2) = (left.op b.1 a.1, right.op b.2 a.2)
    rw [left.comm, right.comm]
  unit_op := fun a => by
    show (left.op left.unit a.1, right.op right.unit a.2) = a
    rw [left.unit_op, right.unit_op]

/-- A pair of absorbing elements absorbs the product. -/
theorem pair_absorbing {V : Type u} {W : Type v} (left : Reduction V) (right : Reduction W)
    {leftAbsorber : V} {rightAbsorber : W}
    (leftAbsorbing : left.Absorbing leftAbsorber) (rightAbsorbing : right.Absorbing rightAbsorber) :
    (pair left right).Absorbing (leftAbsorber, rightAbsorber) := fun a => by
  show (left.op a.1 leftAbsorber, right.op a.2 rightAbsorber) = (leftAbsorber, rightAbsorber)
  rw [leftAbsorbing, rightAbsorbing]

/-- **One coordinate may stop while the other continues.** Once the left
accumulator of a product reduction is absorbing, further members cannot change
it. The right accumulator still folds every remaining member. In particular,
absorption of one coordinate does not decide the whole product.

Proof outline: the empty continuation leaves both accumulators unchanged. For
one more member, absorption fixes the left coordinate; the induction hypothesis
then describes the right coordinate's remaining fold. The absorber premise is
mathematical: this law does not verify an implementation's test for absorption. -/
theorem pair_foldFrom_left_absorbing {V : Type u} {W : Type v} {M : Type w}
    (left : Reduction V) (right : Reduction W)
    (leftValue : M → V) (rightValue : M → W)
    {absorber : V} (absorbing : left.Absorbing absorber)
    (start : W) (members : List M) :
    (pair left right).foldFrom (fun member => (leftValue member, rightValue member))
        (absorber, start) members =
      (absorber, right.foldFrom rightValue start members) := by
  induction members generalizing start with
  | nil => rfl
  | cons head rest ih =>
    have leftStays : left.op absorber (leftValue head) = absorber := by
      rw [left.comm]
      exact absorbing (leftValue head)
    have remaining :
        (pair left right).foldFrom (fun member => (leftValue member, rightValue member))
            (absorber, right.op start (rightValue head)) rest =
          (absorber, right.foldFrom rightValue (right.op start (rightValue head)) rest) :=
      ih (right.op start (rightValue head))
    change
      (pair left right).foldFrom (fun member => (leftValue member, rightValue member))
          (left.op absorber (leftValue head), right.op start (rightValue head)) rest =
        (absorber, right.foldFrom rightValue (right.op start (rightValue head)) rest)
    rw [leftStays]
    exact remaining

/-- The product of idempotent reductions is idempotent. -/
theorem pair_idempotent {V : Type u} {W : Type v} (left : Reduction V) (right : Reduction W)
    (leftIdempotent : left.Idempotent) (rightIdempotent : right.Idempotent) :
    (pair left right).Idempotent := fun a => by
  show (left.op a.1 a.1, right.op a.2 a.2) = a
  rw [leftIdempotent, rightIdempotent]

/-! ## Where the rules apply -/

/-- A walk of a coverage tree that empties its frontier, whoever held its
regions, folds its emitted leaves to the fold of the tree's outputs: a
reduction over an axis searched by several workers does not depend on the
schedule. This is order independence applied to `Pending.Walk.exhausted`. -/
theorem walk_exhausted_fold {α : Type w} {valid : Atoms α → Prop} {V : Type u}
    (r : Reduction V) (value : Atoms α → V) {c : Cube α} (tree : CoverageTree valid c)
    {emitted : List (Atoms α)} (walk : Pending.Walk [⟨c, tree⟩] [] [] emitted) :
    r.fold value emitted = r.fold value tree.outputs :=
  r.fold_perm value (Pending.Walk.exhausted tree walk).1

/-- **A complete world view decides an idempotent fold.** Any two lists that
represent a program's world view, its set of answer sets, fold alike, whatever
their order and repetitions: the value is the world view's, not the
enumeration's. -/
theorem world_view_fold_complete {A : Type w} {V : Type u} (r : Reduction V)
    (value : Atoms A → V) (idempotent : r.Idempotent) (T : Ferraris.Theory A)
    {first second : List (Atoms A)}
    (firstExact : WorldViews.Represents T first) (secondExact : WorldViews.Represents T second) :
    r.fold value first = r.fold value second :=
  r.fold_same_members value idempotent fun M => (firstExact M).trans (secondExact M).symm

/-- **An absorbed prefix decides an idempotent fold of the world view.** If the
answer sets seen so far fold to the absorbing element, the fold of the whole
world view is that element, whatever answer sets remain unseen. Only the seen
members' membership is needed, not coverage: an interrupted enumeration whose
folded members are answer sets decides the question once it is absorbed.

Proof outline: the seen answer sets are members of the world view, so the seen
list followed by any list representing the world view has exactly the world
view's members; for an idempotent reduction that concatenation folds like the
world view, and its fold is the absorbing element by early exit. -/
theorem world_view_fold_decided {A : Type w} {V : Type u} (r : Reduction V)
    (value : Atoms A → V) (idempotent : r.Idempotent) {absorber : V}
    (absorbing : r.Absorbing absorber) (T : Ferraris.Theory A)
    {seen family : List (Atoms A)} (answers : ∀ M, M ∈ seen → Ferraris.Stable M T)
    (decided : r.fold value seen = absorber) (exact : WorldViews.Represents T family) :
    r.fold value family = absorber := by
  have sameMembers : ∀ M, M ∈ seen ++ family ↔ M ∈ family := by
    intro M
    constructor
    · intro member
      rcases List.mem_append.mp member with inSeen | inFamily
      · exact (exact M).mpr (answers M inSeen)
      · exact inFamily
    · intro member
      exact List.mem_append.mpr (Or.inr member)
  calc r.fold value family
      = r.fold value (seen ++ family) :=
        (r.fold_same_members value idempotent sameMembers).symm
    _ = absorber := r.fold_decided_prefix value absorbing decided family

end Zetesis.AxisFolds
