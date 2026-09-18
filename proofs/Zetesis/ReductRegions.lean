import Zetesis.Ferraris
import Zetesis.Search
import Zetesis.FormulaBounds

/-!
# The proper-subset query as a region tree

Membership of a classical model `M` of a theory is decided by its frozen
reduct: `M` is stable exactly when no proper subset of `M` models the
reduct (`Ferraris.Stable`). That query is a coverage tree over the cube
whose lower bound is empty and whose upper bound is `M`, with the frozen
reduct as the theory: a leaf other than `M` that models the reduct is a
proper-subset model and refutes stability, and a tree with no such leaf
proves it. `FormulaBounds.known_sound` narrows the query's regions over the
reduct theory as it narrows the candidate tree over the original one, and
`decided_leaf_models` makes every surviving leaf a model of the reduct.
The Rust query reads the original DAG under the candidate's frozen truth
mask rather than building the reduct; that the masked reading is the
reduct's is `FerrarisMask`'s law, and the mask's correctness a Rust
obligation.
-/

namespace Zetesis.ReductRegions

open Ferraris

universe u

variable {α : Type u}

/-- The query's root: every atom of `M` open, every other atom cut. -/
def query (M : Atoms α) : Cube α := ⟨fun _ => False, M⟩

theorem query_contains (M J : Atoms α) : (query M).Contains J ↔ Sub J M := by
  constructor
  · exact fun h => h.2
  · exact fun h => ⟨fun _ f => f.elim, h⟩

/-- A proper subset of `M` that models the frozen reduct refutes stability. -/
theorem leaf_refutes (M J : Atoms α) (T : Theory α) (sub : ProperSub J M)
    (models : Models J (ReductTheory M T)) : ¬ Stable M T :=
  fun stable => stable.2 ⟨J, sub, models⟩

/-- A classical model of the theory whose query tree has no proper-subset
leaf is stable: the tree's coverage says no proper subset models the reduct. -/
theorem exhausted_stable (M : Atoms α) (T : Theory α) (original : Models M T)
    (none : ∀ J, ProperSub J M → ¬ Models J (ReductTheory M T)) : Stable M T :=
  ⟨original, fun ⟨J, sub, models⟩ => none J sub models⟩

/-- The valid seeds of the query tree: proper subsets of `M` modelling the
reduct. A coverage tree over the query root with this validity lists
exactly the countermodels (`CoverageTree.mem_outputs_iff`). -/
def Countermodel (M : Atoms α) (T : Theory α) (J : Atoms α) : Prop :=
  ProperSub J M ∧ Models J (ReductTheory M T)

theorem countermodels_exact (M : Atoms α) (T : Theory α)
    (tree : CoverageTree (Countermodel M T) (query M)) (J : Atoms α) :
    J ∈ tree.outputs ↔ Countermodel M T J := by
  rw [CoverageTree.mem_outputs_iff]
  constructor
  · exact fun h => h.2
  · exact fun h => ⟨(query_contains M J).2 h.1.1, h⟩

/-- Stability from an empty query tree, and its refutation from a nonempty one. -/
theorem stable_iff_no_countermodel (M : Atoms α) (T : Theory α) (original : Models M T)
    (tree : CoverageTree (Countermodel M T) (query M)) :
    Stable M T ↔ tree.outputs = [] := by
  constructor
  · intro stable
    cases h : tree.outputs with
    | nil => rfl
    | cons J rest =>
      have mem : J ∈ tree.outputs := by rw [h]; exact List.mem_cons_self ..
      exact (leaf_refutes M J T ((countermodels_exact M T tree J).1 mem).1
        ((countermodels_exact M T tree J).1 mem).2 stable).elim
  · intro empty
    refine exhausted_stable M T original fun J sub models => ?_
    have mem : J ∈ tree.outputs := (countermodels_exact M T tree J).2 ⟨sub, models⟩
    rw [empty] at mem
    exact nomatch mem

end Zetesis.ReductRegions
