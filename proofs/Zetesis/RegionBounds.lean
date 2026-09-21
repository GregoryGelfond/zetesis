import Zetesis.Bounds
import Zetesis.SeedSelections

/-!
# Root coordinates and borrowed region bounds

A completed root retains fixed-held atoms and finitely many remaining atom
coordinates. A descendant holds, cuts or leaves each coordinate open. Reading
those decisions through an exact partial lookup denotes the same cube as
materializing the held and uncut atom selections.

`materialization_exact` reuses the shared-selection law for each bound;
`gate_readings_exact` and `narrowing_exact` transfer that equality to the
existing must/may semantics and its completed narrowing operation. They add no
candidate-acceptance rule: `Bounds.acceptance_survives_narrowing` still supplies
the preservation argument for an accepted seed contained in the original cube.

Exact lookup includes coverage and unique coordinate identity. Rust additionally
maintains canonical root order, fixed/root disjointness and matching region
length. Disjointness is not needed for this union/set equality and is therefore
not an artificial theorem premise. The sorting permutations, binary search,
program identity, completed-root publication, immutable pre-pass borrow and
transactional decision update remain executable correspondence obligations.
-/

namespace Zetesis.RegionBounds

universe u v
variable {Index : Type u} {Atom : Type v}

/-- The finite coordinates whose atoms every seed of the descendant holds. -/
def held (decision : Index → Option Bool) : Index → Bool :=
  fun coordinate => decision coordinate == some true

/-- Held and open coordinates remain possible; only a cut excludes an atom. -/
def possible (decision : Index → Option Bool) : Index → Bool :=
  fun coordinate => decision coordinate != some false

/-- Read a selected coordinate through a partial inverse of its denotation.
A missing key is absent; whether a completed root justifies that absence for
accepted seeds is a separate semantic coverage premise. -/
def read (locate : Atom → Option Index) (choose : Index → Bool) : Atoms Atom :=
  fun atom => ∃ coordinate, locate atom = some coordinate ∧ choose coordinate = true

/-- Exact key lookup and a selected coordinate list denote the same atom set.
In each direction, lookup soundness/completeness supplies the coordinate's
membership and immutable atom identity; filtering supplies its selected state. -/
theorem lookup_selection_exact
    (indices : List Index) (denote : Index → Atom) (locate : Atom → Option Index)
    (exact_lookup : ∀ atom coordinate,
      locate atom = some coordinate ↔ coordinate ∈ indices ∧ denote coordinate = atom)
    (choose : Index → Bool) :
    read locate choose = SeedSelections.selected denote (indices.filter choose) := by
  apply atoms_ext
  intro atom
  have sound : read locate choose atom →
      SeedSelections.selected denote (indices.filter choose) atom := by
    intro selected
    obtain ⟨coordinate, found, chosen⟩ := selected
    have identity : coordinate ∈ indices ∧ denote coordinate = atom :=
      (exact_lookup atom coordinate).mp found
    exact ⟨coordinate, List.mem_filter.mpr ⟨identity.1, chosen⟩, identity.2⟩
  have complete : SeedSelections.selected denote (indices.filter choose) atom →
      read locate choose atom := by
    intro selected
    obtain ⟨coordinate, filtered, identity⟩ := selected
    have choice : coordinate ∈ indices ∧ choose coordinate = true :=
      List.mem_filter.mp filtered
    exact ⟨coordinate, (exact_lookup atom coordinate).mpr ⟨choice.1, identity⟩,
      choice.2⟩
  exact ⟨sound, complete⟩

/-- The owned cube after ordering and coalescing both selected atom lists. -/
def materialized [BEq Atom] (fixed : Atoms Atom) (lower upper : List Atom) : Cube Atom :=
  ⟨Union fixed (SeedSelections.owned lower), Union fixed (SeedSelections.owned upper)⟩

/-- The same fixed atoms, with lower and upper membership read from coordinates. -/
def borrowed (fixed : Atoms Atom) (locate : Atom → Option Index)
    (decision : Index → Option Bool) : Cube Atom :=
  ⟨Union fixed (read locate (held decision)),
    Union fixed (read locate (possible decision))⟩

section Correspondence

variable [BEq Atom] [LawfulBEq Atom]
variable (fixed : Atoms Atom) (indices : List Index) (denote : Index → Atom)
variable (locate : Atom → Option Index) (decision : Index → Option Bool)
variable (lower upper : List Atom)
variable (exact_lookup : ∀ atom coordinate,
  locate atom = some coordinate ↔ coordinate ∈ indices ∧ denote coordinate = atom)
variable (lower_values : lower.Perm ((indices.filter (held decision)).map denote))
variable (upper_values : upper.Perm ((indices.filter (possible decision)).map denote))

include exact_lookup lower_values upper_values

/-- Materializing the two finite selections preserves the entire borrowed cube.
First apply selection materialization to each ordered list; exact lookup then
identifies that selection with the borrowed reading. Adding the same fixed-held
atoms preserves each equality. -/
theorem materialization_exact :
    materialized fixed lower upper = borrowed fixed locate decision := by
  have lower_same : SeedSelections.owned lower = read locate (held decision) := by
    calc
      SeedSelections.owned lower =
          SeedSelections.selected denote (indices.filter (held decision)) :=
        SeedSelections.materialization_exact denote (indices.filter (held decision))
          lower lower_values
      _ = read locate (held decision) :=
        (lookup_selection_exact indices denote locate exact_lookup (held decision)).symm
  have upper_same : SeedSelections.owned upper = read locate (possible decision) := by
    calc
      SeedSelections.owned upper =
          SeedSelections.selected denote (indices.filter (possible decision)) :=
        SeedSelections.materialization_exact denote (indices.filter (possible decision))
          upper upper_values
      _ = read locate (possible decision) :=
        (lookup_selection_exact indices denote locate exact_lookup (possible decision)).symm
  unfold materialized borrowed
  rw [lower_same, upper_same]

/-- Both polarities of both must/may gate readings are unchanged. The original
rule, including its gate lists, is identical on the two sides. -/
theorem gate_readings_exact (rule : Semantics.Rule Atom) :
    (Bounds.MustGate rule (materialized fixed lower upper) ↔
      Bounds.MustGate rule (borrowed fixed locate decision)) ∧
    (Bounds.MayGate rule (materialized fixed lower upper) ↔
      Bounds.MayGate rule (borrowed fixed locate decision)) := by
  have same : materialized fixed lower upper = borrowed fixed locate decision :=
    materialization_exact (fixed := fixed) (indices := indices) (denote := denote)
      (locate := locate) (decision := decision) (lower := lower) (upper := upper)
      exact_lookup lower_values upper_values
  rw [same]
  exact ⟨Iff.rfl, Iff.rfl⟩

/-- The existing complete narrowing operation is identical under this change
of representation. Both closures read the same pre-pass cube; this statement
does not authorize an upper reading after lower decisions have been committed.
It concerns complete semantic closures, not partial operational prefixes. -/
theorem narrowing_exact (program : Semantics.Program Atom) (carrier : Atoms Atom) :
    Bounds.Narrow program carrier (materialized fixed lower upper) =
      Bounds.Narrow program carrier (borrowed fixed locate decision) := by
  have same : materialized fixed lower upper = borrowed fixed locate decision :=
    materialization_exact (fixed := fixed) (indices := indices) (denote := denote)
      (locate := locate) (decision := decision) (lower := lower) (upper := upper)
      exact_lookup lower_values upper_values
  rw [same]

end Correspondence
end Zetesis.RegionBounds
