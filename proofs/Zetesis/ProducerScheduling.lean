import Zetesis.NormalSupport

/-!
# Affected possible-head producers

Each mathematical ground rule belongs to an original producer occurrence. A
producer's registered input predicates cover every positive atom of each rule
it can instantiate. If a current positive body was not already old, at least
one registered predicate changed, so that producer must be selected.

The laws concern possible heads, not truth in an answer set. Completed history
must already contain every old proposal. In particular, zero-input producers
need a complete bootstrap. Constraints have no proposed head; final original
constraint evaluation remains separate. Source instantiation, signed signature
identity, exact reverse postings, packed masks, binding partitions, work/storage
admission and successful round publication are Rust correspondence obligations.
-/

namespace Zetesis.ProducerScheduling

universe u v w
variable {Atom : Type u} {Producer : Type v} {Predicate : Type w}

/-- A producer is affected when an atom newly present in the carrier has one of
its registered input predicates. Registration may conservatively contain more
predicates than a particular ground rule needs. -/
def Affected (signature : Atom → Predicate) (inputs : Producer → Predicate → Prop)
    (old current : Atoms Atom) (producer : Producer) : Prop :=
  ∃ atom, current atom ∧ ¬ old atom ∧ inputs producer (signature atom)

/-- Selected occurrences propose their ordinary filtered positive heads. This
does not test candidate gates or establish original constraint satisfaction. -/
def proposeSelected (family : List (Producer × Semantics.Rule Atom))
    (selected : Producer → Prop) (carrier : Atoms Atom) : Atoms Atom :=
  fun atom => ∃ entry ∈ family, selected entry.1 ∧
    entry.2.head = some atom ∧ entry.2.filter ∧ Semantics.Body entry.2 carrier

/-- An unaffected producer cannot acquire a new positive body under complete
input registration. Every current body atom absent from the old carrier would
itself witness an affected input predicate. -/
theorem unaffected_body_old (rule : Semantics.Rule Atom) (producer : Producer)
    (signature : Atom → Predicate) (inputs : Producer → Predicate → Prop)
    (old current : Atoms Atom)
    (covered : ∀ atom ∈ rule.positive, inputs producer (signature atom))
    (unaffected : ¬ Affected signature inputs old current producer)
    (body : Semantics.Body rule current) : Semantics.Body rule old := by
  classical
  intro atom member
  have retained : old atom := by
    apply Classical.byContradiction
    intro absent
    have changed : Affected signature inputs old current producer :=
      ⟨atom, body atom member, absent, covered atom member⟩
    exact unaffected changed
  exact retained

/-- Selecting every affected producer preserves the full inflationary step
when all old heads were already published.

A full proposal either belongs to a selected producer, or its producer is
unaffected. In the latter case its positive body was old, so completed history
already retains the head. Conversely, selecting fewer producers creates no
proposal outside the original program. Repeated original occurrences may have
distinct owners even when their ground rules are logically equal. -/
theorem selective_step_exact (family : List (Producer × Semantics.Rule Atom))
    (signature : Atom → Predicate) (inputs : Producer → Predicate → Prop)
    (selected : Producer → Prop) (old current : Atoms Atom)
    (covered : ∀ entry ∈ family, ∀ atom ∈ entry.2.positive,
      inputs entry.1 (signature atom))
    (woken : ∀ producer, Affected signature inputs old current producer → selected producer)
    (oldHeads : Sub (NormalSupport.propose (family.map Prod.snd) old) current) :
    Union current (proposeSelected family selected current) =
      Union current (NormalSupport.propose (family.map Prod.snd) current) := by
  classical
  apply atoms_ext
  intro atom
  constructor
  · rintro (retained | proposed)
    · exact Or.inl retained
    · obtain ⟨entry, member, _, head, valid, body⟩ := proposed
      have original : entry.2 ∈ family.map Prod.snd :=
        List.mem_map.mpr ⟨entry, member, rfl⟩
      exact Or.inr ⟨entry.2, original, head, valid, body⟩
  · rintro (retained | proposed)
    · exact Or.inl retained
    · obtain ⟨rule, member, head, valid, body⟩ := proposed
      obtain ⟨entry, original, rfl⟩ := List.mem_map.mp member
      by_cases chosen : selected entry.1
      · exact Or.inr ⟨entry, original, chosen, head, valid, body⟩
      · have unaffected : ¬ Affected signature inputs old current entry.1 := by
          intro changed
          exact chosen (woken entry.1 changed)
        have oldBody : Semantics.Body entry.2 old :=
          unaffected_body_old entry.2 entry.1 signature inputs old current
            (covered entry original) unaffected body
        have member : entry.2 ∈ family.map Prod.snd :=
          List.mem_map.mpr ⟨entry, original, rfl⟩
        have retained : current atom := oldHeads atom ⟨entry.2, member, head, valid, oldBody⟩
        exact Or.inl retained

/-- An empty wake set establishes possible-head closure under the same complete
registration and old-head history premises. It does not discharge constraints
or establish that the carrier is an answer set. -/
theorem empty_selection_closed (family : List (Producer × Semantics.Rule Atom))
    (signature : Atom → Predicate) (inputs : Producer → Predicate → Prop)
    (selected : Producer → Prop) (old current : Atoms Atom)
    (covered : ∀ entry ∈ family, ∀ atom ∈ entry.2.positive,
      inputs entry.1 (signature atom))
    (woken : ∀ producer, Affected signature inputs old current producer → selected producer)
    (oldHeads : Sub (NormalSupport.propose (family.map Prod.snd) old) current)
    (empty : ∀ producer, ¬ selected producer) :
    SourceSupport.Closed (NormalSupport.propose (family.map Prod.snd)) current := by
  have sameStep : Union current (proposeSelected family selected current) =
      Union current (NormalSupport.propose (family.map Prod.snd) current) :=
    selective_step_exact family signature inputs selected
    old current covered woken oldHeads
  intro atom proposed
  have full : Union current (NormalSupport.propose (family.map Prod.snd) current) atom := Or.inr proposed
  have restricted : Union current (proposeSelected family selected current) atom := by
    rw [sameStep]
    exact full
  rcases restricted with retained | emitted
  · exact retained
  · obtain ⟨entry, _, chosen, _⟩ := emitted
    exact False.elim (empty entry.1 chosen)

end Zetesis.ProducerScheduling
