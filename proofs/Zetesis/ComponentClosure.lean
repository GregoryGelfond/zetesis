import Zetesis.ProducerScheduling

/-!
# Least closures by dependency components

This module uses the library's existing predicate sets, monotone transformers,
and `Least`, the intersection of all pre-fixed points. Bekić's law decomposes
the least closure of two coupled monotone components. Its dependency-ordered
case computes the lower component first and the upper component with that
lower result fixed. Neither law assumes finite carriers or terminating loops.

`completed_ordered_rounds` connects the ordered case to existing finite
inflationary iteration: completed, closed component scans give the same least
closure. `completed_producers_closed` separately captures the scheduling
history obligation: each producer ran completely after its registered inputs'
last change. It uses `ProducerScheduling.ReadsOnly`, not a claim that merely
having an SCC label establishes independence.

These laws concern possible-head closure. They do not identify that carrier
with an answer set, prove a source producer monotone, validate a dependency
graph, or establish the truth of a Rust completion flag. Partial/resource-
limited attempts supply no completed scan or publication premise.
-/

namespace Zetesis.ComponentClosure

universe u v w
variable {A : Type u} {B : Type v} {C : Type w}

/-- Combine two disjoint semantic components without identifying their atoms. -/
def combine (lower : Atoms A) (upper : Atoms B) : Atoms (Sum A B)
  | .inl atom => lower atom
  | .inr atom => upper atom

/-- The lower component of a combined carrier. -/
def lower (carrier : Atoms (Sum A B)) : Atoms A := fun atom => carrier (.inl atom)

/-- The upper component of a combined carrier. -/
def upper (carrier : Atoms (Sum A B)) : Atoms B := fun atom => carrier (.inr atom)

/-- Increasing either component cannot remove a proposed result. -/
def MonotoneInBoth (propose : Atoms A → Atoms B → Atoms C) : Prop :=
  ∀ {left nextLeft right nextRight}, Sub left nextLeft → Sub right nextRight →
    Sub (propose left right) (propose nextLeft nextRight)

/-- Simultaneous proposals from the two component operators. -/
def coupled (first : Atoms A → Atoms B → Atoms A)
    (second : Atoms A → Atoms B → Atoms B) : Transformer (Sum A B) :=
  fun carrier => combine (first (lower carrier) (upper carrier))
    (second (lower carrier) (upper carrier))

/-- Close the first component while holding the second component fixed. -/
def lowerClosure (first : Atoms A → Atoms B → Atoms A)
    (parameter : Atoms B) : Atoms A := Least (fun current => first current parameter)

/-- The second component reads the least first-component closure at each input. -/
def upperTransform (first : Atoms A → Atoms B → Atoms A)
    (second : Atoms A → Atoms B → Atoms B) : Transformer B :=
  fun current => second (lowerClosure first current) current

/-- A larger parameter gives a larger first-component least closure. This is
the existing least-closure refinement law applied pointwise. -/
theorem lowerClosure_monotone (first : Atoms A → Atoms B → Atoms A)
    (monotone : MonotoneInBoth first) {before after : Atoms B}
    (included : Sub before after) :
    Sub (lowerClosure first before) (lowerClosure first after) := by
  apply least_mono
  intro current
  exact monotone (sub_refl current) included

/-- The substituted second-component transformer is monotone. Both its
parameter and the least first-component closure grow with its input. -/
theorem upperTransform_monotone (first : Atoms A → Atoms B → Atoms A)
    (second : Atoms A → Atoms B → Atoms B)
    (firstMonotone : MonotoneInBoth first)
    (secondMonotone : MonotoneInBoth second) :
    MonotoneT (upperTransform first second) := by
  intro before after included
  exact secondMonotone (lowerClosure_monotone first firstMonotone included) included

/-- Bekić's law for the existing predicate-set least closure.

Close the second component after substituting the first component's least
closure; then close the first at that resulting parameter. The pair equals the
least simultaneous closure.

The argument has two obligations. First, this constructed pair is closed.
Second, every simultaneous closed carrier contains it: its lower projection
contains the first closure at its upper projection; that makes its upper
projection closed for the substituted operator; leastness bounds the selected
upper result, and monotonicity then bounds the selected lower result. These two
facts identify the pair with `Least`, without postulating a fixed-point oracle. -/
theorem bekic (first : Atoms A → Atoms B → Atoms A)
    (second : Atoms A → Atoms B → Atoms B)
    (firstMonotone : MonotoneInBoth first)
    (secondMonotone : MonotoneInBoth second) :
    Least (coupled first second) =
      combine (lowerClosure first (Least (upperTransform first second)))
        (Least (upperTransform first second)) := by
  let chosenUpper := Least (upperTransform first second)
  let chosenLower := lowerClosure first chosenUpper
  have chosenClosed : Closed (coupled first second) (combine chosenLower chosenUpper) := by
    intro atom proposed
    cases atom with
    | inl atom =>
      have fixedParameter : MonotoneT (fun current => first current chosenUpper) := by
        intro before after included
        exact firstMonotone included (sub_refl chosenUpper)
      exact least_closed fixedParameter atom proposed
    | inr atom =>
      exact least_closed (upperTransform_monotone first second firstMonotone secondMonotone)
        atom proposed
  have chosenLeast : ∀ carrier, Closed (coupled first second) carrier →
      Sub (combine chosenLower chosenUpper) carrier := by
    intro carrier carrierClosed
    have lowerClosed : Closed (fun current => first current (upper carrier)) (lower carrier) := by
      intro atom proposed
      exact carrierClosed (.inl atom) proposed
    have lowerBound : Sub (lowerClosure first (upper carrier)) (lower carrier) :=
      least_le lowerClosed
    have upperClosed : Closed (upperTransform first second) (upper carrier) := by
      intro atom proposed
      have expanded : second (lower carrier) (upper carrier) atom :=
        secondMonotone lowerBound (sub_refl (upper carrier)) atom proposed
      exact carrierClosed (.inr atom) expanded
    have chosenUpperBound : Sub chosenUpper (upper carrier) := least_le upperClosed
    have chosenLowerBound : Sub chosenLower (lower carrier) :=
      sub_trans (lowerClosure_monotone first firstMonotone chosenUpperBound) lowerBound
    intro atom present
    cases atom with
    | inl atom => exact chosenLowerBound atom present
    | inr atom => exact chosenUpperBound atom present
  apply sub_antisymm
  · exact least_le chosenClosed
  · intro atom present carrier carrierClosed
    exact chosenLeast carrier carrierClosed atom present

/-- Dependency-ordered Bekić: the lower component does not read the upper
component. Close it once, then close the upper component with those inputs
fixed. This is the component-independence premise used by delayed producers;
it is stronger than merely naming a lower component. -/
theorem ordered_components (first : Transformer A)
    (second : Atoms A → Atoms B → Atoms B)
    (firstMonotone : MonotoneT first)
    (secondMonotone : MonotoneInBoth second) :
    Least (coupled (fun current _ => first current) second) =
      combine (Least first) (Least (second (Least first))) := by
  have independentMonotone : MonotoneInBoth (fun current (_ : Atoms B) => first current) := by
    intro before after oldUpper newUpper included _upperIncluded
    exact firstMonotone included
  exact bekic (fun current _ => first current) second independentMonotone secondMonotone

/-- Observed completion connects finite component scans to the ordered law.
Each finite scan is sound by the existing `FiniteIter` law. Its separate
closedness premise identifies it with its least closure. There is no claim
that an arbitrary fuel bound completes either component. -/
theorem completed_ordered_rounds (first : Transformer A)
    (second : Atoms A → Atoms B → Atoms B)
    (firstMonotone : MonotoneT first)
    (secondMonotone : MonotoneInBoth second) (lowerRounds upperRounds : Nat)
    (lowerComplete : Closed first (FiniteIter first lowerRounds))
    (upperComplete : Closed (second (FiniteIter first lowerRounds))
      (FiniteIter (second (FiniteIter first lowerRounds)) upperRounds)) :
    combine (FiniteIter first lowerRounds)
      (FiniteIter (second (FiniteIter first lowerRounds)) upperRounds) =
      Least (coupled (fun current _ => first current) second) := by
  have lowerExact : FiniteIter first lowerRounds = Least first :=
    finiteIter_exact_if_closed firstMonotone lowerRounds lowerComplete
  have upperMonotone : MonotoneT (second (FiniteIter first lowerRounds)) := by
    intro before after included
    exact secondMonotone (sub_refl (FiniteIter first lowerRounds)) included
  have upperExact : FiniteIter (second (FiniteIter first lowerRounds)) upperRounds =
      Least (second (FiniteIter first lowerRounds)) :=
    finiteIter_exact_if_closed upperMonotone upperRounds upperComplete
  calc
    combine (FiniteIter first lowerRounds)
        (FiniteIter (second (FiniteIter first lowerRounds)) upperRounds) =
        combine (FiniteIter first lowerRounds) (Least (second (FiniteIter first lowerRounds))) := by
      rw [upperExact]
    _ = combine (Least first) (Least (second (Least first))) := by rw [lowerExact]
    _ = Least (coupled (fun current _ => first current) second) :=
      (ordered_components first second firstMonotone secondMonotone).symm

/-- A completed traversal need not run again after its inputs settle. Complete
read registration makes the final proposals equal those at the last snapshot;
successful publication already contains those proposals. No monotonicity is
needed for this equality-based scheduling fact. -/
theorem settled_producer_covered {Producer : Type v} {Predicate : Type w}
    (signature : A → Predicate) (inputs : Producer → Predicate → Prop)
    (propose : Producer → Atoms A → Atoms A) (producer : Producer)
    (snapshot final : Atoms A)
    (reads : ProducerScheduling.ReadsOnly signature inputs propose producer)
    (settled : ∀ atom, inputs producer (signature atom) → (snapshot atom ↔ final atom))
    (published : Sub (propose producer snapshot) final) :
    Sub (propose producer final) final := by
  have sameProposals : propose producer snapshot = propose producer final :=
    reads snapshot final settled
  rw [← sameProposals]
  exact published

/-- A completed producer schedule establishes possible-head closure when every
producer's final read relations equal its last completed snapshot and that
traversal's proposals were published. Each producer may have a different last
snapshot. A waiting producer supplies no such publication premise.

This closes the history obligation behind one-time component-scheduled scans.
The result is closedness only; leastness also needs a sound construction, and
answer-set coverage still needs the library's reduct-projection premise. -/
theorem completed_producers_closed {Producer : Type v} {Predicate : Type w}
    (producers : List Producer) (signature : A → Predicate)
    (inputs : Producer → Predicate → Prop)
    (propose : Producer → Atoms A → Atoms A)
    (last : Producer → Atoms A) (final : Atoms A)
    (reads : ∀ producer ∈ producers,
      ProducerScheduling.ReadsOnly signature inputs propose producer)
    (settled : ∀ producer ∈ producers, ∀ atom,
      inputs producer (signature atom) → (last producer atom ↔ final atom))
    (published : ∀ producer ∈ producers, Sub (propose producer (last producer)) final) :
    SourceSupport.Closed (ProducerScheduling.proposeAll producers propose) final := by
  intro atom proposed
  obtain ⟨producer, member, atFinal⟩ := proposed
  have covered : Sub (propose producer final) final :=
    settled_producer_covered signature inputs propose producer (last producer) final
      (reads producer member) (settled producer member) (published producer member)
  exact covered atom atFinal

end Zetesis.ComponentClosure
