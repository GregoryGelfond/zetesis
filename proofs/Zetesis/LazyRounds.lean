import Zetesis.Lifted
import Zetesis.Transformers

/-!
World-isolated consequence rounds over shared source instances.

The source scan uses the union of complete immutable world snapshots. That union
covers each world's enabled bindings, but does not establish positive truth in
any individual world. `MaterializedTransform` keeps that independent world test.
Chunk composition below always uses the same world snapshot and frozen seed.

These laws discharge the semantic coverage and composition obligations of the
relational protocol. Rust source traversal, atom-ID encoding, finite budgets,
WGSL evaluation, readback and publication remain unproved correspondences.
Exhaustion of a previous snapshot is not an assumption about a later snapshot.
-/

namespace Zetesis.LazyRounds

open Lifted

universe u v w

variable {α : Type u} {β : Type v} {ω : Type w}

/-- Shared source relation; membership may originate in a different world. -/
def WorldUnion (worlds : ω → Atoms α) : Atoms α :=
  fun atom => ∃ world, worlds world atom

/-- Every world's positive snapshot is contained in the shared source union. -/
theorem world_below_union (worlds : ω → Atoms α) (world : ω) :
    Sub (worlds world) (WorldUnion worlds) :=
  fun _atom present => ⟨world, present⟩

/-- An exhausted union scan covers each individual world's enabled bindings.
The scan premise is independent of frozen gates, matching the host join stage. -/
theorem union_scan_covers_world (t : Template α β) (worlds : ω → Atoms α)
    (world : ω) (seed : Atoms α) (selected : Bindings β)
    (scan : ∀ binding, Bind t (WorldUnion worlds) binding →
      t.filter binding → selected binding) :
    SourceCoverage t seed (worlds world) selected := by
  intro binding positive filter _gate
  have unionPositive : Bind t (WorldUnion worlds) binding :=
    bind_monotone t (world_below_union worlds world) binding positive
  exact scan binding unionPositive filter

/-- Per-world positive and frozen-gate checks remove cross-world combinations.
The resulting consequence is exact once the union scan has exhausted. -/
theorem world_consequences_exact (t : Template α β) (worlds : ω → Atoms α)
    (world : ω) (seed : Atoms α) (selected : Bindings β)
    (scan : ∀ binding, Bind t (WorldUnion worlds) binding →
      t.filter binding → selected binding) (atom : α) :
    MaterializedTransform t seed (worlds world) selected atom ↔
      DirectConsequence t seed (worlds world) atom := by
  have coverage : SourceCoverage t seed (worlds world) selected :=
    union_scan_covers_world t worlds world seed selected scan
  exact materialized_exact t seed (worlds world) selected coverage atom

/-- Constraint detection requires the same fresh source coverage as derivation. -/
theorem world_constraints_exact (t : Template α β) (worlds : ω → Atoms α)
    (world : ω) (seed : Atoms α) (selected : Bindings β)
    (scan : ∀ binding, Bind t (WorldUnion worlds) binding →
      t.filter binding → selected binding) :
    MaterializedConstraint t seed (worlds world) selected ↔
      ConstraintTriggered t seed (worlds world) := by
  have coverage : SourceCoverage t seed (worlds world) selected :=
    union_scan_covers_world t worlds world seed selected scan
  exact materialized_constraint_exact t seed (worlds world) selected coverage

/-- Chunk boundaries do not change consequences when every chunk reads the
same immutable snapshot and seed. Neither chunk sees the other's new heads. -/
theorem chunk_concatenation (t : Template α β) (seed snapshot : Atoms α)
    (first second : List β) :
    MaterializedTransform t seed snapshot (fun binding => binding ∈ first ++ second) =
      Union
        (MaterializedTransform t seed snapshot (fun binding => binding ∈ first))
        (MaterializedTransform t seed snapshot (fun binding => binding ∈ second)) := by
  apply atoms_ext
  intro atom
  constructor
  · rintro ⟨binding, ⟨⟨⟨positive, selected⟩, filter⟩, gate⟩, head⟩
    rcases List.mem_append.mp selected with inFirst | inSecond
    · exact Or.inl ⟨binding, ⟨⟨⟨positive, inFirst⟩, filter⟩, gate⟩, head⟩
    · exact Or.inr ⟨binding, ⟨⟨⟨positive, inSecond⟩, filter⟩, gate⟩, head⟩
  · intro consequence
    rcases consequence with fromFirst | fromSecond
    · rcases fromFirst with ⟨binding, ⟨⟨⟨positive, selected⟩, filter⟩, gate⟩, head⟩
      exact ⟨binding, ⟨⟨⟨positive, List.mem_append.mpr (Or.inl selected)⟩, filter⟩, gate⟩, head⟩
    · rcases fromSecond with ⟨binding, ⟨⟨⟨positive, selected⟩, filter⟩, gate⟩, head⟩
      exact ⟨binding, ⟨⟨⟨positive, List.mem_append.mpr (Or.inr selected)⟩, filter⟩, gate⟩, head⟩

/-- A complete no-growth scan establishes closedness. A partial scan cannot
supply `coverage`; absence of a pending delta alone is insufficient. -/
theorem completed_round_closed (t : Template α β) (seed snapshot : Atoms α)
    (selected : Bindings β) (coverage : SourceCoverage t seed snapshot selected)
    (noGrowth : Sub (MaterializedTransform t seed snapshot selected) snapshot) :
    Closed (DirectConsequence t seed) snapshot := by
  intro atom consequence
  have emitted : MaterializedTransform t seed snapshot selected atom :=
    materialized_complete t seed snapshot selected coverage atom consequence
  exact noGrowth atom emitted

/-- Sound derivation from the empty interpretation plus a complete final round
establishes the least frozen-reduct closure, independent of chunk scheduling. -/
theorem completed_round_exact (t : Template α β) (seed snapshot : Atoms α)
    (selected : Bindings β) (coverage : SourceCoverage t seed snapshot selected)
    (sound : Sub snapshot (Least (DirectConsequence t seed)))
    (noGrowth : Sub (MaterializedTransform t seed snapshot selected) snapshot) :
    snapshot = Least (DirectConsequence t seed) := by
  have closed : Closed (DirectConsequence t seed) snapshot :=
    completed_round_closed t seed snapshot selected coverage noGrowth
  exact exact_of_sound_and_closed sound closed

end Zetesis.LazyRounds
