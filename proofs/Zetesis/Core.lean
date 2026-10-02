import Std

/-!
# Predicate sets for answer-set semantics

The semantic universe is symbolic. A set is a membership predicate; these
definitions do not enumerate atoms or assume a physical representation.
-/
namespace Zetesis

universe u

abbrev Atoms (α : Type u) := α → Prop

def Sub {α : Type u} (X Y : Atoms α) : Prop := ∀ a, X a → Y a
def Empty {α : Type u} : Atoms α := fun _ => False
def Full {α : Type u} : Atoms α := fun _ => True
def Union {α : Type u} (X Y : Atoms α) : Atoms α := fun a => X a ∨ Y a
def Inter {α : Type u} (X Y : Atoms α) : Atoms α := fun a => X a ∧ Y a
def EqOn {α : Type u} (S X Y : Atoms α) : Prop := ∀ a, S a → (X a ↔ Y a)

theorem sub_refl {α : Type u} (X : Atoms α) : Sub X X := fun _ h => h

theorem sub_trans {α : Type u} {X Y Z : Atoms α}
    (hXY : Sub X Y) (hYZ : Sub Y Z) : Sub X Z :=
  fun a h => hYZ a (hXY a h)

theorem atoms_ext {α : Type u} {X Y : Atoms α}
    (h : ∀ a, X a ↔ Y a) : X = Y :=
  funext (fun a => propext (h a))

theorem sub_antisymm {α : Type u} {X Y : Atoms α}
    (hXY : Sub X Y) (hYX : Sub Y X) : X = Y :=
  atoms_ext (fun a => ⟨hXY a, hYX a⟩)

theorem inter_sub_left {α : Type u} (X Y : Atoms α) : Sub (Inter X Y) X :=
  fun _ h => h.1

theorem inter_sub_right {α : Type u} (X Y : Atoms α) : Sub (Inter X Y) Y :=
  fun _ h => h.2

end Zetesis
