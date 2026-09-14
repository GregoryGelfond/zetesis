import Zetesis.NormalFerraris
import Zetesis.SourceSupport

/-!
# Possible support for normalized rules

A possible-head producer retains every filtered rule head whose positive body
lies in the carrier. It ignores the candidate's gates. Its proposals therefore
cover positive reduct consequences without claiming that they are realizable.

The central result discharges `SourceSupport.ProjectionCompatible` for this
producer. Intersecting an original model with a producer-closed carrier retains
every required reduct head: original satisfaction supplies membership in the
model, and producer closure supplies membership in the carrier. Constraints
remain satisfied when positive atoms are removed. Reduct minimality then places
every answer set inside the carrier.

The program here is the mathematical normalized-rule representation. Ground
filters and the normal/Ferraris translation are those of `NormalFerraris`.
Complete source bindings, checked arithmetic, richer head/aggregate producers,
Rust catalog construction and device execution remain separate obligations.
`projection_compatible_of_coverage` permits a producer with additional proposals;
it does not establish that any concrete source compiler supplies that coverage.
-/

namespace Zetesis.NormalSupport

universe u
variable {A : Type u}

/-- Propose every filtered atomic head with a possible positive body. Gates are
ignored; a headless constraint proposes no atom. -/
def propose (P : Semantics.Program A) (carrier : Atoms A) : Atoms A :=
  fun atom => ∃ rule ∈ P,
    rule.head = some atom ∧ rule.filter ∧ Semantics.Body rule carrier

/-- Changing only a normalized rule's candidate gates preserves its possible
heads at every carrier. The positive inputs, ground filter and head are fixed.

Each mapped rule has exactly the original head/filter/body witness, in both
directions. This is producer invariance, not equivalence of the gated programs:
their reducts and answer sets can differ. Source bindings, argument evaluation
and the concrete distinction between inputs and non-inputs remain obligations. -/
theorem propose_gate_independent (P : Semantics.Program A)
    (trueGates falseGates : Semantics.Rule A → List A) (carrier : Atoms A) :
    propose (P.map (fun rule =>
      { rule with gateTrue := trueGates rule, gateFalse := falseGates rule })) carrier =
      propose P carrier := by
  apply atoms_ext
  intro atom
  constructor
  · intro ⟨mapped, member, head, valid, body⟩
    obtain ⟨original, member, rfl⟩ := List.mem_map.mp member
    exact ⟨original, member, head, valid, body⟩
  · intro ⟨original, member, head, valid, body⟩
    exact ⟨{ original with gateTrue := trueGates original,
      gateFalse := falseGates original },
      List.mem_map.mpr ⟨original, member, rfl⟩, head, valid, body⟩

/-- Adding possible body atoms cannot remove a proposed head. The same rule
witness remains applicable by positive-body monotonicity. -/
theorem propose_monotone (P : Semantics.Program A) :
    SourceSupport.Monotone (propose P) := by
  intro left right subset atom proposed
  obtain ⟨rule, member, head, filter, body⟩ := proposed
  exact ⟨rule, member, head, filter, Semantics.body_mono subset body⟩

/-- Restricting an original normalized model to a closed possible carrier still
models its frozen reduct. A required head belongs to both sets; constraints
are downward closed for the same frozen gates. -/
theorem restricted_reduct_model (P : Semantics.Program A) (carrier model : Atoms A)
    (closed : SourceSupport.Closed (propose P) carrier)
    (original : Semantics.ReductModel P model model) :
    Semantics.ReductModel P model (SourceSupport.restrict model carrier) := by
  have inModel : Sub (SourceSupport.restrict model carrier) model := by
    intro atom retained
    exact retained.1
  have inCarrier : Sub (SourceSupport.restrict model carrier) carrier := by
    intro atom retained
    exact retained.2
  have heads : Closed (Semantics.Consequence P model)
      (SourceSupport.restrict model carrier) := by
    intro atom consequence
    obtain ⟨rule, member, head, filter, gate, body⟩ := consequence
    have originalHead : model atom :=
      original.1 atom
        ⟨rule, member, head, filter, gate, Semantics.body_mono inModel body⟩
    have possibleHead : carrier atom :=
      closed atom ⟨rule, member, head, filter, Semantics.body_mono inCarrier body⟩
    exact ⟨originalHead, possibleHead⟩
  have constraints : Semantics.ConstraintsOK P model
      (SourceSupport.restrict model carrier) :=
    Semantics.constraints_down inModel original.2
  exact ⟨heads, constraints⟩

/-- Normal possible-head closure establishes the frozen-projection premise.
The normal/Ferraris bridge transfers the restricted normalized model to the
original model's formula reduct, including its constraints. -/
theorem projection_compatible (P : Semantics.Program A) :
    SourceSupport.ProjectionCompatible (propose P) (NormalFerraris.translate P) := by
  intro carrier model closed original
  have subset : Sub (SourceSupport.restrict model carrier) model := by
    intro atom retained
    exact retained.1
  have normalModel : Semantics.ReductModel P model model :=
    (NormalFerraris.models_translate P model).mp original
  have restricted : Semantics.ReductModel P model
      (SourceSupport.restrict model carrier) :=
    restricted_reduct_model P carrier model closed normalModel
  exact (NormalFerraris.models_frozen_translate P model
    (SourceSupport.restrict model carrier) subset).mpr ⟨normalModel, restricted⟩

/-- Additional possible proposals preserve projection compatibility when every
normal proposal remains covered at every carrier. Closure of the larger
producer implies closure of the normal producer. -/
theorem projection_compatible_of_coverage (P : Semantics.Program A)
    (producer : Atoms A → Atoms A)
    (covers : ∀ carrier, Sub (propose P carrier) (producer carrier)) :
    SourceSupport.ProjectionCompatible producer (NormalFerraris.translate P) := by
  intro carrier model closed original
  have normalClosed : SourceSupport.Closed (propose P) carrier := by
    intro atom proposed
    exact closed atom (covers carrier atom proposed)
  exact projection_compatible P carrier model normalClosed original

/-- Every answer set lies in any closed carrier of a producer covering the
normal possible heads. Projection gives a subset reduct model; answer-set
minimality forces every original atom to remain. -/
theorem answer_set_inside_closed (P : Semantics.Program A)
    (producer : Atoms A → Atoms A)
    (covers : ∀ carrier, Sub (propose P carrier) (producer carrier))
    (carrier model : Atoms A) (closed : SourceSupport.Closed producer carrier)
    (answer : Ferraris.Stable model (NormalFerraris.translate P)) :
    Sub model carrier := by
  have compatible :
      SourceSupport.ProjectionCompatible producer (NormalFerraris.translate P) :=
    projection_compatible_of_coverage P producer covers
  exact SourceSupport.stable_inside_closed producer (NormalFerraris.translate P)
    compatible carrier model closed answer

end Zetesis.NormalSupport
