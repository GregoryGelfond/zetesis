import Zetesis.LazyRounds

/-!
Positive-prefix world membership within one immutable consequence round.

A membership mask denotes the worlds in which every selected positive row is
true. Extending a prefix intersects this set with one row's world membership.
An empty set justifies pruning all extensions in this snapshot; it says nothing
about later snapshots, different candidates, or the program's possible carrier.

The coverage laws below retain both per-world positive checks and frozen gates.
They discharge the semantic premise needed by `LazyRounds.completed_round_exact`
without requiring the masked scan to exhaust cross-world union combinations.
Concrete packed masks, source traversal, work/byte budgets, Rust and WGSL remain
unproved implementation correspondences. Fresh complete snapshots are explicit
parameters, not a consequence of an unchanged atom catalog.
-/

namespace Zetesis.WorldMasks

open Lifted LazyRounds

universe u v w

variable {α : Type u} {β : Type v} {ω : Type w}

/-- Worlds satisfying the current positive prefix. The empty prefix has every
world; adding a row intersects its membership with the remaining prefix. -/
def Membership (worlds : ω → Atoms α) : List α → ω → Prop
  | [], _ => True
  | atom :: rest, world => worlds world atom ∧ Membership worlds rest world

/-- The recursive membership calculation is exactly positive conjunction. No
candidate seed or frozen gate contributes positive membership. -/
theorem membership_exact (worlds : ω → Atoms α) (rows : List α) (world : ω) :
    Membership worlds rows world ↔ ∀ atom, atom ∈ rows → worlds world atom := by
  induction rows with
  | nil =>
    simp only [Membership, List.not_mem_nil, false_implies, implies_true]
  | cons first rest induction =>
    have sound : Membership worlds (first :: rest) world →
        ∀ atom, atom ∈ first :: rest → worlds world atom := by
      rintro ⟨firstPresent, restPresent⟩ atom member
      rcases List.mem_cons.mp member with same | later
      · exact same ▸ firstPresent
      · exact induction.mp restPresent atom later
    have complete : (∀ atom, atom ∈ first :: rest → worlds world atom) →
        Membership worlds (first :: rest) world := by
      intro present
      have firstPresent : worlds world first :=
        present first List.mem_cons_self
      have restPresent : Membership worlds rest world :=
        induction.mpr (fun atom member => present atom (List.mem_cons_of_mem first member))
      exact ⟨firstPresent, restPresent⟩
    exact ⟨sound, complete⟩

/-- Prefix extension is set intersection in the same immutable snapshots. -/
theorem membership_append (worlds : ω → Atoms α) (rows suffix : List α)
    (world : ω) :
    Membership worlds (rows ++ suffix) world ↔
      Membership worlds rows world ∧ Membership worlds suffix world := by
  have sound : Membership worlds (rows ++ suffix) world →
      Membership worlds rows world ∧ Membership worlds suffix world := by
    intro combined
    have present : ∀ atom, atom ∈ rows ++ suffix → worlds world atom :=
      (membership_exact worlds (rows ++ suffix) world).mp combined
    have prefixPresent : Membership worlds rows world :=
      (membership_exact worlds rows world).mpr
        (fun atom member => present atom (List.mem_append.mpr (Or.inl member)))
    have suffixPresent : Membership worlds suffix world :=
      (membership_exact worlds suffix world).mpr
        (fun atom member => present atom (List.mem_append.mpr (Or.inr member)))
    exact ⟨prefixPresent, suffixPresent⟩
  have complete : Membership worlds rows world ∧ Membership worlds suffix world →
      Membership worlds (rows ++ suffix) world := by
    rintro ⟨prefixPresent, suffixPresent⟩
    apply (membership_exact worlds (rows ++ suffix) world).mpr
    intro atom member
    rcases List.mem_append.mp member with inPrefix | inSuffix
    · exact (membership_exact worlds rows world).mp prefixPresent atom inPrefix
    · exact (membership_exact worlds suffix world).mp suffixPresent atom inSuffix
  exact ⟨sound, complete⟩

/-- A prefix absent from every current world cannot acquire a current-world
witness by extending the same binding with more positive rows. -/
theorem empty_prefix_excludes_extension (worlds : ω → Atoms α)
    (rows suffix : List α) (empty : ¬ ∃ world, Membership worlds rows world) :
    ¬ ∃ world, Membership worlds (rows ++ suffix) world := by
  rintro ⟨world, extended⟩
  have prefixPresent : Membership worlds rows world :=
    ((membership_append worlds rows suffix world).mp extended).1
  exact empty ⟨world, prefixPresent⟩

/-- An exhausted masked union scan still covers every world's enabled binding.
Its selection premise only concerns full positive bodies with a current-world
witness, which every binding needed by that world necessarily supplies. -/
theorem masked_scan_covers_world (t : Template α β) (worlds : ω → Atoms α)
    (world : ω) (seed : Atoms α) (selected : Bindings β)
    (scan : ∀ binding, Bind t (WorldUnion worlds) binding → t.filter binding →
      (∃ witness, Membership worlds (t.positive binding) witness) → selected binding) :
    SourceCoverage t seed (worlds world) selected := by
  intro binding positive filter _gate
  have unionPositive : Bind t (WorldUnion worlds) binding :=
    bind_monotone t (world_below_union worlds world) binding positive
  have witness : Membership worlds (t.positive binding) world :=
    (membership_exact worlds (t.positive binding) world).mpr positive
  exact scan binding unionPositive filter ⟨world, witness⟩

/-- Discarding empty-mask positive bodies preserves every world's consequence
relation once the masked source scan is complete. Gates remain frozen checks. -/
theorem masked_consequences_exact (t : Template α β) (worlds : ω → Atoms α)
    (world : ω) (seed : Atoms α) (selected : Bindings β)
    (scan : ∀ binding, Bind t (WorldUnion worlds) binding → t.filter binding →
      (∃ witness, Membership worlds (t.positive binding) witness) → selected binding)
    (atom : α) :
    MaterializedTransform t seed (worlds world) selected atom ↔
      DirectConsequence t seed (worlds world) atom := by
  have coverage : SourceCoverage t seed (worlds world) selected :=
    masked_scan_covers_world t worlds world seed selected scan
  exact materialized_exact t seed (worlds world) selected coverage atom

/-- Constraint coverage follows the same current-world witness obligation as
positive derivation; an omitted cross-world conjunction cannot violate it. -/
theorem masked_constraints_exact (t : Template α β) (worlds : ω → Atoms α)
    (world : ω) (seed : Atoms α) (selected : Bindings β)
    (scan : ∀ binding, Bind t (WorldUnion worlds) binding → t.filter binding →
      (∃ witness, Membership worlds (t.positive binding) witness) → selected binding) :
    MaterializedConstraint t seed (worlds world) selected ↔
      ConstraintTriggered t seed (worlds world) := by
  have coverage : SourceCoverage t seed (worlds world) selected :=
    masked_scan_covers_world t worlds world seed selected scan
  exact materialized_constraint_exact t seed (worlds world) selected coverage

/-- Membership can grow when snapshots grow, even when every atom identity was
already known. A previous empty mask is therefore not a future omission proof. -/
theorem membership_monotone {before after : ω → Atoms α}
    (grows : ∀ world, Sub (before world) (after world)) (rows : List α) (world : ω) :
    Membership before rows world → Membership after rows world := by
  intro previous
  have present : ∀ atom, atom ∈ rows → after world atom := by
    intro atom member
    have earlier : before world atom :=
      (membership_exact before rows world).mp previous atom member
    exact grows world atom earlier
  exact (membership_exact after rows world).mpr present

/-- Negative control: the union contains both atoms, while no world contains
their conjunction. Union truth alone cannot replace the per-world test. -/
theorem cross_world_union_has_no_common_witness :
    (∀ atom : Bool, atom ∈ [false, true] →
      WorldUnion (fun world atom : Bool => world = atom) atom) ∧
    ¬ ∃ world, Membership (fun world atom : Bool => world = atom) [false, true] world := by
  have unionPresent : ∀ atom : Bool, atom ∈ [false, true] →
      WorldUnion (fun world atom : Bool => world = atom) atom := by
    intro atom _member
    exact ⟨atom, rfl⟩
  have noWitness : ¬ ∃ world,
      Membership (fun world atom : Bool => world = atom) [false, true] world := by
    rintro ⟨world, first, second, _⟩
    have impossible : false = true := first.symm.trans second
    exact Bool.noConfusion impossible
  exact ⟨unionPresent, noWitness⟩

/-- Negative control: the same one-atom catalog can have an empty old mask and
a nonempty current mask. Catalog identity cannot establish snapshot freshness. -/
theorem stale_membership_can_omit_current_truth :
    (¬ ∃ world : Unit, Membership (fun _ _ : Unit => False) [()] world) ∧
    (∃ world : Unit, Membership (fun _ _ : Unit => True) [()] world) := by
  have absentBefore : ¬ ∃ world : Unit,
      Membership (fun _ _ : Unit => False) [()] world := by
    rintro ⟨_world, absent, _⟩
    exact absent
  have presentNow : ∃ world : Unit,
      Membership (fun _ _ : Unit => True) [()] world :=
    ⟨(), True.intro, True.intro⟩
  exact ⟨absentBefore, presentNow⟩

end Zetesis.WorldMasks
