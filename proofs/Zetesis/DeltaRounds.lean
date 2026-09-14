import Zetesis.DeltaJoins
import Zetesis.Lifted

/-!
# Delta rounds with completed history

The candidate is fixed. Positive truth grows from an old snapshot to the current
one; the same complete binding has the same filter, gates and head throughout.
A delta selection need only cover bindings whose positive combination was not
already available. Old headed consequences must already have been published.
Old constraint triggers instead persist in a separate latch.

`coverage_of_first_delta` connects the existing occurrence partition to this
new-binding coverage. Its row correspondence uses stable occurrence row IDs:
the old IDs form a prefix of the current IDs. A canonical sorted rank which
moves on append cannot supply that premise. Repeated predicates remain separate
source occurrences. A binding may include its complete row-choice witness;
partition disjointness does not imply distinct heads or variable assignments.

`delta_step_exact` preserves the inflationary consequence step, not the complete
list of bindings evaluated in that round. `latched_constraints_exact` preserves
one latch for a whole constraint family. Bootstrap remains a separate duty:
facts and other zero-positive-input rules have no new positive occurrence, so
their heads must first be published and their constraint triggers recorded by a
complete initial scan. The headed history premise does not discharge constraints.

These are mathematical schedule laws. Source registration, tuple matching and
generation, stable IDs, actual prefix boundaries, error-producing expressions,
work/storage admission, interruption and publication remain concrete obligations.
No runtime delta algorithm, finite termination bound or answer-set membership is
established here. Existing leastness laws apply only after soundness and completed
closedness have separately been supplied.
-/

namespace Zetesis.DeltaRounds

open Lifted

universe u v w
variable {Atom : Type u} {Binding : Type v}

/-- Every currently enabled, filter-valid binding with a new positive combination
is selected. Old bindings may be absent even if their heads remain true. -/
def DeltaCoverage (template : Template Atom Binding) (seed old current : Atoms Atom)
    (selected : Bindings Binding) : Prop :=
  ∀ binding, Bind template current binding → template.filter binding →
    Enabled template seed binding → ¬ Bind template old binding → selected binding

/-- Complete first-new-occurrence partitions supply delta binding coverage.

`currentRows` relates a positive binding to bounded current row IDs. `oldRows`
states that the same binding was positive in the old snapshot exactly when all
those IDs were old. These are explicit row-identity/matcher premises, not facts
obtained from tuple counts alone. `partitions` supplies the actual selections.

A binding absent from the old snapshot cannot use only old IDs. Choose a new
occurrence, apply `DeltaJoins.partition_complete`, and use that partition's
selection. `DeltaJoins.partition_disjoint` separately gives pivot uniqueness.
With zero occurrences there is no new combination; bootstrap is not supplied. -/
theorem coverage_of_first_delta (template : Template Atom Binding)
    (seed old current : Atoms Atom) (selected : Bindings Binding)
    (occurrences : Nat) (oldSize currentSize : Nat → Nat)
    (rows : Binding → Nat → Nat)
    (currentRows : ∀ binding, Bind template current binding →
      DeltaJoins.Current occurrences currentSize (rows binding))
    (oldRows : ∀ binding, Bind template current binding →
      (Bind template old binding ↔ DeltaJoins.Current occurrences oldSize (rows binding)))
    (partitions : ∀ binding pivot,
      DeltaJoins.FirstDelta occurrences oldSize currentSize (rows binding) pivot →
      Bind template current binding → template.filter binding →
      Enabled template seed binding → selected binding) :
    DeltaCoverage template seed old current selected := by
  classical
  intro binding positive valid enabled newBinding
  have newRow : ∃ occurrence, occurrence < occurrences ∧
      oldSize occurrence ≤ rows binding occurrence := by
    apply Classical.byContradiction
    intro absent
    have allOld : DeltaJoins.Current occurrences oldSize (rows binding) := by
      intro occurrence inside
      have notNew : ¬ oldSize occurrence ≤ rows binding occurrence := by
        intro newly
        exact absent ⟨occurrence, inside, newly⟩
      omega
    exact newBinding ((oldRows binding positive).mpr allOld)
  obtain ⟨pivot, first⟩ :=
    (DeltaJoins.partition_complete occurrences oldSize currentSize (rows binding)).mp
      ⟨currentRows binding positive, newRow⟩
  exact partitions binding pivot first positive valid enabled

/-- Adding the selected delta has exactly the effect of a complete headed step
when every old consequence is already in the current snapshot.

For a full consequence, split on whether its positive binding was already old.
The history premise retains an old head; delta coverage selects a new binding.
Conversely, every selected consequence is sound by `Lifted.materialized_sound`.
The old snapshot need not itself be closed. For zero-positive-input rules, the
history premise requires their enabled heads to have been handled at bootstrap. -/
theorem delta_step_exact (template : Template Atom Binding)
    (seed old current : Atoms Atom) (selected : Bindings Binding)
    (oldHeads : Sub (DirectConsequence template seed old) current)
    (covered : DeltaCoverage template seed old current selected) :
    Union current (MaterializedTransform template seed current selected) =
      Union current (DirectConsequence template seed current) := by
  classical
  have fullCovered : Sub (DirectConsequence template seed current)
      (Union current (MaterializedTransform template seed current selected)) := by
    intro atom consequence
    obtain ⟨binding, valid, enabled, positive, head⟩ := consequence
    by_cases wasOld : Bind template old binding
    · exact Or.inl (oldHeads atom ⟨binding, valid, enabled, wasOld, head⟩)
    · have chosen : selected binding := covered binding positive valid enabled wasOld
      exact Or.inr ⟨binding, ⟨⟨⟨positive, chosen⟩, valid⟩, enabled⟩, head⟩
  apply atoms_ext
  intro atom
  constructor
  · rintro (retained | emitted)
    · exact Or.inl retained
    · exact Or.inr (materialized_sound template seed current selected atom emitted)
  · rintro (retained | consequence)
    · exact Or.inl retained
    · exact fullCovered atom consequence

/-- A whole family's old constraint latch plus the newly selected triggers equals
the complete constraint-trigger test at the current snapshot.

`history` identifies the latch with the old family's actual triggers; it is not
an arbitrary Boolean success marker. Growing positive truth preserves every old
trigger. A current trigger is either old, and therefore latched, or has a new
positive combination selected by delta coverage. Selected triggers are checked
against the complete current body, filter and fixed candidate gates.

Zero-positive constraints must already be represented in the old latch after
bootstrap, even if no head was derived. The template head is unused. This law
does not permit an implementation to stop required scans after latching failure. -/
theorem latched_constraints_exact {Index : Type w} {BindingsFor : Index → Type v}
    (templates : (index : Index) → Template Atom (BindingsFor index))
    (seed old current : Atoms Atom)
    (selected : (index : Index) → Bindings (BindingsFor index)) (latched : Prop)
    (history : latched ↔ ∃ index, ConstraintTriggered (templates index) seed old)
    (growing : Sub old current)
    (covered : ∀ index, DeltaCoverage (templates index) seed old current (selected index)) :
    (latched ∨ ∃ index, MaterializedConstraint (templates index) seed current (selected index)) ↔
      ∃ index, ConstraintTriggered (templates index) seed current := by
  classical
  constructor
  · rintro (recorded | emitted)
    · obtain ⟨index, binding, valid, enabled, positive⟩ := history.mp recorded
      have retained : Bind (templates index) current binding :=
        bind_monotone (templates index) growing binding positive
      exact ⟨index, binding, valid, enabled, retained⟩
    · obtain ⟨index, binding, ⟨⟨positive, _chosen⟩, valid⟩, enabled⟩ := emitted
      exact ⟨index, binding, valid, enabled, positive⟩
  · rintro ⟨index, binding, valid, enabled, positive⟩
    by_cases wasOld : Bind (templates index) old binding
    · exact Or.inl (history.mpr ⟨index, binding, valid, enabled, wasOld⟩)
    · have chosen : selected index binding :=
        covered index binding positive valid enabled wasOld
      exact Or.inr ⟨index, binding, ⟨⟨⟨positive, chosen⟩, valid⟩, enabled⟩⟩

end Zetesis.DeltaRounds
