import Zetesis.Ferraris
import Zetesis.Search

/-!
# Pruning candidate generation by entailed consequences

This is the outermost law of every narrowing that derives its consequences from
the theory's own formulas. An answer set models its theory, so a property that
every model of the theory has, every answer set has. Pruning candidate
generation by such a property therefore keeps every answer set: a region in
which no interpretation has the property contains no answer set, and a value
that every interpretation of a region with the property shares is the value of
every answer set in the region.

A consequence may come from a sub-collection R of the theory's formulas, as a
recognized constraint group does; every model of the theory models R. Some
consequences rest on stability rather than satisfaction, as the completion of a
definition does; for those the stable-model premise is supplied directly.

The laws say nothing about which consequences a narrowing derives, how it
represents them, or what it costs. Each source of consequences discharges the
entailment premise for its own consequences (`ParityRows`, `CountingFamilies`).
Membership is not involved: a candidate is still accepted only by the reduct's
check, and nothing here enters that check.
-/

namespace Zetesis.EntailedPruning

open Ferraris

universe u
variable {α : Type u}

/-- A property every model of a theory has, every answer set of it has.

Proof. An answer set satisfies every formula of its theory, the first half of
`Stable`'s definition. -/
theorem stable_satisfies_entailed {T : Theory α} {P : Atoms α → Prop}
    (entailed : ∀ M, Models M T → P M) : ∀ M, Stable M T → P M :=
  fun M stable => entailed M stable.1

/-- A model of a theory models every collection of formulas drawn from it,
repetitions and order included. -/
theorem models_of_included {R T : Theory α} (included : ∀ F ∈ R, F ∈ T)
    {M : Atoms α} (model : Models M T) : Models M R :=
  fun F member => model F (included F member)

/-- A property every model of some of a theory's formulas has, every answer set
of the theory has. -/
theorem stable_satisfies_entailed_by_part {R T : Theory α}
    (included : ∀ F ∈ R, F ∈ T) {P : Atoms α → Prop}
    (entailed : ∀ M, Models M R → P M) : ∀ M, Stable M T → P M :=
  stable_satisfies_entailed fun M model => entailed M (models_of_included included model)

/-- A region in which no interpretation has a property every answer set has
contains no answer set. This is the refutation premise of
`CoverageTree.refuted` with the theory's answer sets as the valid predicate. -/
theorem refuted_region_excludes {T : Theory α} {P : Atoms α → Prop}
    (entailed : ∀ M, Stable M T → P M) (c : Cube α)
    (refuted : ∀ z, c.Contains z → ¬ P z) :
    ∀ z, c.Contains z → ¬ Stable z T :=
  fun z inside answer => refuted z inside (entailed z answer)

/-- What every interpretation of a region with a property every answer set has
satisfies, every answer set in the region satisfies. This is the form of
`Cube.narrow_preserves_valid`'s premises with the answer sets as the valid
predicate. -/
theorem forced_in_region {T : Theory α} {P Q : Atoms α → Prop}
    (entailed : ∀ M, Stable M T → P M) (c : Cube α)
    (forced : ∀ z, c.Contains z → P z → Q z) :
    ∀ z, c.Contains z → Stable z T → Q z :=
  fun z inside answer => forced z inside (entailed z answer)

end Zetesis.EntailedPruning
