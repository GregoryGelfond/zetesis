import Zetesis.CountermodelReplay
import Zetesis.AggregateReduct
import Zetesis.OrdinalExtrema
import Zetesis.FormulaBounds

/-!
# Replaying original nodes backed by numeric rows

A native threshold or extremum node names a descriptor and its owner's row.
Both columns of the row use the same span. Its positions remain distinct when
two positions reference the same formula node or carry equal numbers.

This model starts at an admitted numeric view. It uses natural indices, exact
integer sums and a validated ordinal carrier. It relates its original and frozen
node values to finite aggregate formulas. It does not model allocation, cache
lifetime, machine overflow, source tuple coalescing or an executable ledger.
-/

namespace Zetesis.Refinement.NumericWitnessReplay

open Ferraris AggregateReduct LearnedRegions

universe u
variable {α : Type u}

/-- A span borrowed from one immutable pair of condition and value columns. -/
structure Row where
  size : Nat
  condition : Fin size → Nat
  value : Fin size → Nat

/-- Read the entire range from both columns, even when a weight is zero. -/
def read (conditions values : List Nat) (start size : Nat) : Option Row :=
  if fits : start + size ≤ conditions.length ∧ start + size ≤ values.length then
    some ⟨size, fun i => conditions[start + i.val]'(by have := i.isLt; omega),
      fun i => values[start + i.val]'(by have := i.isLt; omega)⟩
  else none

/-- A successful borrow fixes both identities of every position, including
repetitions. No membership or source-key inference is made from the node ID. -/
theorem read_occurrence {conditions values : List Nat} {start size : Nat} {row : Row}
    (found : read conditions values start size = some row) (i : Fin row.size) :
    conditions[start + i.val]? = some (row.condition i) ∧
    values[start + i.val]? = some (row.value i) := by
  unfold read at found
  split at found
  · cases found
    constructor <;> apply List.getElem?_eq_getElem
  · contradiction

/-- The signed bound and the two cuts retain their native comparisons.
For a cut, greatest is its admitted rank count minus one. -/
inductive Guard where
  | threshold (bound : Int)
  | prefix (greatest cut : Nat)
  | suffix (greatest cut : Nat)
  deriving Repr

/-- Apply the same fixed guard to the selected occurrences of this row. -/
def accepts (row : Row) (guard : Guard) (mask : Mask row.size) : Bool :=
  let selected := RankedExtrema.selectedValues row.value mask
  match guard with
  | .threshold bound => decide (bound ≤ (selected.sum : Int))
  | .prefix greatest cut => decide ((OrdinalExtrema.measure greatest selected).1 < cut)
  | .suffix greatest cut => decide (cut ≤ (OrdinalExtrema.measure greatest selected).2)

/-- Interpret each referenced child ID under one common table of meanings. -/
def meaning (row : Row) (guard : Guard) (children : Nat → Formula α) : Formula α :=
  aggregate (fun i => children (row.condition i)) (accepts row guard) (masks row.size)

/-- A successful numeric operator reads both the original node mask and the
guard on the frozen predecessor truth. It applies to arbitrary tested worlds. -/
theorem frozen_read (row : Row) (guard : Guard) (children : Nat → Formula α)
    (M N : Atoms α) :
    Satisfies N (Reduct M (meaning row guard children)) ↔
      Satisfies M (meaning row guard children) ∧
      accepts row guard (active N (fun i => Reduct M (children (row.condition i)))) = true := by
  unfold meaning
  rw [direct_reduct, original M _ _ _ (masks_complete _)]

/-- A false original guard stops replay at that node. For a true guard,
replaying the selected nodes of each actual predecessor preserves the complete
tuple mask and hence the fixed guard, including a signed or empty-endpoint cut. -/
theorem selected_replay (row : Row) (guard : Guard) (children : Nat → Formula α)
    {M M' N : Atoms α} (below : Sub N M) (below' : Sub N M')
    (original : Satisfies M (meaning row guard children) ↔ Satisfies M' (meaning row guard children))
    (selected : Satisfies M (meaning row guard children) → ∀ i,
      NodeAgreement M M' (replayNodes M N (children (row.condition i)))) :
    Satisfies N (Reduct M (meaning row guard children)) ↔
      Satisfies N (Reduct M' (meaning row guard children)) := by
  classical
  rw [frozen_read, frozen_read]
  by_cases live : Satisfies M (meaning row guard children)
  · have equal : active N (fun i => Reduct M (children (row.condition i))) =
        active N (fun i => Reduct M' (children (row.condition i))) := by
      funext i
      have := replay_nodes_agree below below' _ (selected live i)
      simp only [active, this]
    rw [equal]
    exact and_congr_left' original
  · exact ⟨fun h => (live h.1).elim, fun h => (live (original.mpr h.1)).elim⟩

end Zetesis.Refinement.NumericWitnessReplay

namespace Zetesis.LearnedRegions

open Ferraris
universe u
variable {α : Type u}

open Classical in
/-- In a region holding the witness and an extra atom, a stable completion
would make every asserted root true. For the other selected conditions,
the original theory's signed knowledge in that region supplies agreement.
This particular bridge uses FormulaBounds.Known; an implementation fact
proved only for stable completions needs its own soundness argument. -/
theorem known_countermodel_refutes_region {T : Theory α} {M N : Atoms α} (r : Region α)
    (model : Models M T) (witness : ReductRegions.Countermodel M T N)
    (held : Sub N r.lower) {a : α} (extra : r.lower a) (outside : ¬ N a)
    (knowledge : ∀ F ∈ T, ∀ G ∈ replayNodes M N F, G ∉ T →
      FormulaBounds.Known T r G (decide (Satisfies M G))) :
    ∀ z, r.Contains z → ¬ Stable z T := by
  intro z inside stable
  apply countermodel_row_refutes witness _ (sub_trans held inside.1) (inside.1 a extra) outside stable
  intro F root G picked
  by_cases asserted : G ∈ T
  · exact ⟨fun _ => stable.1 G asserted, fun _ => model G asserted⟩
  · have sound := FormulaBounds.known_sound T r inside stable.1 (knowledge F root G picked asserted)
    by_cases old : Satisfies M G
    · exact ⟨fun _ => sound.1 (by simp [old]), fun _ => old⟩
    · exact ⟨fun h => (old h).elim, fun h => (sound.2 (by simp [old]) h).elim⟩

end Zetesis.LearnedRegions
