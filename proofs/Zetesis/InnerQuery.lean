import Zetesis.Frontier
import Zetesis.ReductRegions

/-!
# The proper-subset query walked as one frontier by several workers

A candidate's membership query is a coverage tree over `ReductRegions.query M`.
When the parallel walk publishes a long query, its pending regions sit in
several workers' queues, and each server visits the region it took. That is a
frontier walk of the query tree, whoever holds its regions: `Pending.Walk`
covers it, and a region taken by another worker is a step like any other.

This module composes the frontier law with the query's characterization. A
walk of the query tree from its root that empties the frontier without
emitting a leaf proves the candidate stable, and a leaf emitted by any walk,
emptied or not, refutes stability, so the verdict does not depend on the
schedule. The second half needs no emptied walk: a server that finds a
countermodel may stop the query with regions still queued.

That the cell's count of live regions reaches zero only when the frontier is
empty, that a stop or an undecided leaf prevents the stable verdict, and that a
witness is a leaf of the query tree are the Rust obligations.
-/

namespace Zetesis.InnerQuery

open Ferraris ReductRegions

universe u

variable {α : Type u}

/-- A walk of the query tree from its root that empties the frontier and emits
no countermodel proves the classical model stable. -/
theorem exhausted_walk_stable (M : Atoms α) (T : Theory α) (original : Models M T)
    (tree : CoverageTree (Countermodel M T) (query M))
    (walk : Pending.Walk [⟨query M, tree⟩] [] [] []) : Stable M T := by
  have empty : tree.outputs = [] := (Pending.Walk.exhausted tree walk).1.symm.eq_nil
  exact (stable_iff_no_countermodel M T original tree).2 empty

/-- A countermodel emitted by any walk of the query tree from its root, whether
or not the frontier is empty, refutes stability. -/
theorem emitted_refutes (M : Atoms α) (T : Theory α)
    (tree : CoverageTree (Countermodel M T) (query M))
    {frontier : List (Pending (Countermodel M T))} {emitted : List (Atoms α)}
    (walk : Pending.Walk [⟨query M, tree⟩] [] frontier emitted)
    {J : Atoms α} (mem : J ∈ emitted) : ¬ Stable M T := by
  have law := walk.perm
  have inTree : J ∈ tree.outputs := by
    have held : J ∈ emitted ++ Pending.outputs frontier := List.mem_append_left _ mem
    have root := law.symm.mem_iff.mp held
    simpa [Pending.outputs] using root
  have countermodel := (countermodels_exact M T tree J).1 inTree
  exact leaf_refutes M J T countermodel.1 countermodel.2

/-- The verdict of an emptied walk is its emitted leaves' emptiness, whatever the
schedule that produced them. -/
theorem exhausted_walk_decides (M : Atoms α) (T : Theory α) (original : Models M T)
    (tree : CoverageTree (Countermodel M T) (query M)) {emitted : List (Atoms α)}
    (walk : Pending.Walk [⟨query M, tree⟩] [] [] emitted) :
    Stable M T ↔ emitted = [] := by
  constructor
  · intro stable
    cases h : emitted with
    | nil => rfl
    | cons J rest =>
      have mem : J ∈ emitted := by rw [h]; exact List.mem_cons_self ..
      exact (emitted_refutes M T tree walk mem stable).elim
  · intro empty
    subst empty
    exact exhausted_walk_stable M T original tree walk

end Zetesis.InnerQuery
