import Zetesis.Ferraris
import Zetesis.Search

/-!
# Regions on the formula route

The formula route proposes candidates by a retained chronological search over
the theory's atoms: each node of the search holds some atoms in and some out,
propagation adds the atoms every classical model of the theory agrees on under
those decisions, a conflict closes the node, and a complete assignment is
handed to the reduct. Read as regions, a node is a cube, the propagated atoms
narrow it, a conflict refutes it and a complete assignment is a leaf; the
reduct decides membership at the leaf and nowhere else, as on the closure
route. This module states why the narrowing is sound for stable models: every
stable model is a classical model, so a consequence of the theory under the
region holds in every stable model of the region, and a region with no
classical model holds no stable model. A restriction every stable model
satisfies, such as the support restriction, extends the theory the
consequences are drawn from. The tree is `CoverageTree` with the stable
models as the valid seeds; its leaves are exactly the stable models of the
root because each leaf is checked by the reduct (`mem_outputs_iff`).

The search decides nothing: it proposes. That the Rust cursor's propagation
returns only classical consequences of the clauses it holds, and that those
clauses are the theory and restrictions every stable model satisfies, are
Rust obligations; this module supplies the semantic step from classical
consequence to stable-model consequence.
-/

namespace Zetesis.FormulaRegions

open Ferraris

universe u

variable {α : Type u}

/-- A classical consequence of the theory under a region holds in every stable
model of the region. -/
theorem classical_consequence_forces (T : Theory α) (c : Cube α) (a : α)
    (h : ∀ z, c.Contains z → Models z T → z a) :
    ∀ z, c.Contains z → Stable z T → z a :=
  fun z hz hs => h z hz hs.1

/-- An atom no classical model of the region holds is in no stable model of
the region. -/
theorem classical_consequence_cuts (T : Theory α) (c : Cube α) (a : α)
    (h : ∀ z, c.Contains z → Models z T → ¬ z a) :
    ∀ z, c.Contains z → Stable z T → ¬ z a :=
  fun z hz hs => h z hz hs.1

/-- A region without a classical model holds no stable model. -/
theorem no_model_refutes (T : Theory α) (c : Cube α)
    (h : ∀ z, c.Contains z → ¬ Models z T) :
    ∀ z, c.Contains z → ¬ Stable z T :=
  fun z hz hs => h z hz hs.1

/-- A restriction every stable model satisfies adds nothing a stable model
lacks, so consequences of the theory with the restriction narrow soundly. -/
theorem restricted_consequence_forces (T U : Theory α) (c : Cube α) (a : α)
    (hU : ∀ z, Stable z T → Models z U)
    (h : ∀ z, c.Contains z → Models z (T ++ U) → z a) :
    ∀ z, c.Contains z → Stable z T → z a := by
  intro z hz hs
  apply h z hz
  intro F hF
  rcases List.mem_append.mp hF with hT | hU'
  · exact hs.1 F hT
  · exact hU z hs F hU'

/-- The narrowing node of the coverage tree, from the atoms one propagation
forced in and cut out. -/
def propagation_narrowed (T : Theory α) (c : Cube α) (lo hi : Atoms α)
    (forced : ∀ z, c.Contains z → Models z T → Sub lo z)
    (cut : ∀ z, c.Contains z → Models z T → Sub z hi)
    (child : CoverageTree (fun z => Stable z T) (c.narrow lo hi)) :
    CoverageTree (fun z => Stable z T) c :=
  .narrowed c lo hi (fun z hz hs => forced z hz hs.1) (fun z hz hs => cut z hz hs.1) child

/-- The closed node of the coverage tree, from a conflict. -/
def conflict_refuted (T : Theory α) (c : Cube α)
    (h : ∀ z, c.Contains z → ¬ Models z T) :
    CoverageTree (fun z => Stable z T) c :=
  .refuted c (no_model_refutes T c h)

end Zetesis.FormulaRegions
