import Zetesis.Lifted

/-!
# Heads recorded as positions, and a block of rows taken whole

A closure round derives heads and publishes those its relations do not hold.
Two representation choices stand under that step, and each has its law here.

*Marks.* A relation may record a derived head as a position rather than an
atom: a mark is set for a head that is derived and not held, and the marks
are joined into the relation after the round. `marks_are_new_atoms` says
that, under a position function injective on a carrier holding every derived
head, a carrier atom's position is marked exactly when the atom is new, so
joining the marks publishes the new atoms and nothing else;
`marks_distinct` says distinct new atoms have distinct marks, so counting
marks counts new atoms, which is what a derived-atom limit bounds.

*Row steps.* Fix the bindings of every occurrence of a rule but one, whose
last argument alone is free. The bindings through that occurrence are then
indexed by that argument's value. Suppose no filter or gate reads the value,
every other positive atom is the same whatever the value, and the head is
determined by the value. `block_heads` says the heads the rule derives from
those bindings are exactly the heads of the values whose row the relation
holds: the block of rows may be taken whole, without binding each row.
`block_places` adds that when both relations place a value by the same rank
after a base, a row's offset in its block is its head's offset in the head's
block, so the block of rows can be joined into the block of heads place for
place.

That the concrete position of a tuple is injective, that a rule meets the
row step's conditions, that the two relations rank the value alike, and the
word arithmetic of the join are Rust obligations.
-/

namespace Zetesis.RowSteps

open Lifted

universe u v w
variable {α : Type u} {β : Type v} {V : Type w}

/-- The positions a round marks: those of the heads derived and not held. -/
def Marks (pos : α → Nat) (derived held : Atoms α) : Nat → Prop :=
  fun n => ∃ a, derived a ∧ ¬ held a ∧ pos a = n

/-- A carrier atom's position is marked exactly when the atom is derived and
not held.

A mark at the atom's position comes from some new atom with that position;
both lie in the carrier, the derived one by coverage, so injectivity makes
them the same atom. The converse is the definition. -/
theorem marks_are_new_atoms (pos : α → Nat) (carrier derived held : Atoms α)
    (injective : ∀ a b, carrier a → carrier b → pos a = pos b → a = b)
    (covers : ∀ a, derived a → carrier a) (a : α) (inCarrier : carrier a) :
    Marks pos derived held (pos a) ↔ (derived a ∧ ¬ held a) := by
  constructor
  · rintro ⟨b, isDerived, notHeld, samePosition⟩
    have same : b = a := injective b a (covers b isDerived) inCarrier samePosition
    exact same ▸ ⟨isDerived, notHeld⟩
  · rintro ⟨isDerived, notHeld⟩
    exact ⟨a, isDerived, notHeld, rfl⟩

/-- Joining the marks into a relation publishes, among the carrier's atoms,
exactly the held atoms and the derived ones. -/
theorem absorbed_is_union (pos : α → Nat) (carrier derived held : Atoms α)
    (injective : ∀ a b, carrier a → carrier b → pos a = pos b → a = b)
    (covers : ∀ a, derived a → carrier a) (a : α) (inCarrier : carrier a) :
    (held a ∨ Marks pos derived held (pos a)) ↔ (held a ∨ derived a) := by
  classical
  rw [marks_are_new_atoms pos carrier derived held injective covers a inCarrier]
  constructor
  · rintro (isHeld | ⟨isDerived, _⟩)
    · exact Or.inl isHeld
    · exact Or.inr isDerived
  · rintro (isHeld | isDerived)
    · exact Or.inl isHeld
    · by_cases isHeld : held a
      · exact Or.inl isHeld
      · exact Or.inr ⟨isDerived, isHeld⟩

/-- Distinct new atoms have distinct marks: a list of carrier atoms without
repetition has positions without repetition. Counting the marks of a round
therefore counts its new atoms. -/
theorem marks_distinct (pos : α → Nat) (carrier : Atoms α)
    (injective : ∀ a b, carrier a → carrier b → pos a = pos b → a = b)
    (news : List α) (inCarrier : ∀ a ∈ news, carrier a) (distinct : news.Nodup) :
    (news.map pos).Nodup := by
  induction news with
  | nil => exact List.nodup_nil
  | cons a rest ih =>
    have restCarrier : ∀ b ∈ rest, carrier b := fun b member =>
      inCarrier b (List.mem_cons_of_mem a member)
    have parts := List.nodup_cons.mp distinct
    refine List.nodup_cons.mpr ⟨?_, ih restCarrier parts.2⟩
    intro repeated
    obtain ⟨b, inRest, samePosition⟩ := List.mem_map.mp repeated
    have same : b = a := injective b a (restCarrier b inRest)
      (inCarrier a List.mem_cons_self) samePosition
    exact parts.1 (same ▸ inRest)

/-- The bindings of a rule through one occurrence, every other occurrence
fixed: indexed by the value of the occurrence's free last argument. No filter
or gate reads the value, every other positive atom is the same whatever the
value, and the row and the head are functions of the value. -/
structure Block (t : Template α β) (z : Atoms α) where
  /-- The binding that gives the free argument this value. -/
  extend : V → β
  /-- The occurrence's atom under that binding: a row of its relation. -/
  row : V → α
  /-- The head under that binding. -/
  headOf : V → α
  /-- The positive atoms of the other occurrences. -/
  rest : List α
  positive : ∀ x a, a ∈ t.positive (extend x) ↔ (a ∈ rest ∨ a = row x)
  head : ∀ x, t.head (extend x) = headOf x
  filterFixed : ∀ x y, t.filter (extend x) ↔ t.filter (extend y)
  gatesFixed : ∀ x y, Enabled t z (extend x) ↔ Enabled t z (extend y)

/-- The bindings a row step selects: those of the block. -/
def Block.selected {t : Template α β} {z : Atoms α} (block : Block (V := V) t z) : Bindings β :=
  fun b => ∃ x, b = block.extend x

/-- When the fixed part of the rule is live, the heads derived from a block's
bindings are exactly the heads of the values whose row the relation holds.

The fixed part is live when the other positive atoms hold and the filters and
gates hold at some value, hence at every value since none reads it. A binding
of the block then joins exactly when its row holds, passes the filters and
gates because the witness value does, and derives its value's head. So the
row step, which takes the held rows of the block and marks their heads,
selects what binding each row would. -/
theorem block_heads {t : Template α β} {z : Atoms α} (block : Block (V := V) t z)
    (X : Atoms α) (restHolds : ∀ a, a ∈ block.rest → X a) (witness : V)
    (filterHolds : t.filter (block.extend witness))
    (gatesHold : Enabled t z (block.extend witness)) (a : α) :
    MaterializedTransform t z X block.selected a ↔ ∃ x, X (block.row x) ∧ block.headOf x = a := by
  constructor
  · rintro ⟨b, ⟨⟨⟨joins, x, rfl⟩, _⟩, _⟩, isHead⟩
    have rowHolds : X (block.row x) :=
      joins (block.row x) ((block.positive x _).mpr (Or.inr rfl))
    exact ⟨x, rowHolds, (block.head x).symm.trans isHead⟩
  · rintro ⟨x, rowHolds, isHead⟩
    have joins : Bind t X (block.extend x) := by
      intro p member
      rcases (block.positive x p).mp member with inRest | isRow
      · exact restHolds p inRest
      · exact isRow ▸ rowHolds
    have filtered : t.filter (block.extend x) := (block.filterFixed x witness).mpr filterHolds
    have enabled : Enabled t z (block.extend x) := (block.gatesFixed x witness).mpr gatesHold
    exact ⟨block.extend x, ⟨⟨⟨joins, x, rfl⟩, filtered⟩, enabled⟩, (block.head x).trans isHead⟩

/-- When both relations place a value by the same rank after a base, a row's
offset in its block is its head's offset in the head's block. -/
theorem block_places {t : Template α β} {z : Atoms α} (block : Block (V := V) t z)
    (rank : V → Nat) (rowPos headPos : α → Nat) (rowBase headBase : Nat)
    (rows : ∀ x, rowPos (block.row x) = rowBase + rank x)
    (heads : ∀ x, headPos (block.headOf x) = headBase + rank x) (x : V) :
    headPos (block.headOf x) - headBase = rowPos (block.row x) - rowBase := by
  rw [rows x, heads x]
  omega

end Zetesis.RowSteps
