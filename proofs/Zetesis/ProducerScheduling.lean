import Zetesis.NormalSupport

/-!
# Affected possible-head producers

A producer registers predicates; it is affected when an atom newly present in
the carrier has a registered predicate. The general law concerns any family of
producers with a proposal function: when every unaffected producer is stable,
proposing nothing over the current carrier that it did not propose over the old
one, selecting every affected producer preserves the full inflationary step,
and an empty selection establishes closure. A producer whose proposals depend
only on the carrier's atoms with registered predicates is stable whenever the
carrier grows.

Each mathematical ground rule belongs to an original producer occurrence. A
producer's registered input predicates cover every positive atom of each rule
it can instantiate. If a current positive body was not already old, at least
one registered predicate changed, so that producer must be selected: ordinary
rules are stable without the growth premise, and the rule-level laws below are
corollaries of the general ones.

The laws concern possible heads, not truth in an answer set. Completed history
must already contain every old proposal. In particular, zero-input producers
need a complete bootstrap. Constraints have no proposed head; final original
constraint evaluation remains separate. Source instantiation, signed signature
identity, exact reverse postings, packed masks, binding partitions, read-set
registration, work/storage admission and successful round publication are Rust
correspondence obligations.
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

/-- Every producer of `producers` proposes its heads over `carrier`. -/
def proposeAll (producers : List Producer) (propose : Producer → Atoms Atom → Atoms Atom)
    (carrier : Atoms Atom) : Atoms Atom :=
  fun atom => ∃ producer ∈ producers, propose producer carrier atom

/-- The selected producers of `producers` propose their heads over `carrier`. -/
def proposeSelectedOf (producers : List Producer)
    (propose : Producer → Atoms Atom → Atoms Atom) (selected : Producer → Prop)
    (carrier : Atoms Atom) : Atoms Atom :=
  fun atom => ∃ producer ∈ producers, selected producer ∧ propose producer carrier atom

/-- An unaffected producer proposes nothing over the current carrier that it
did not propose over the old one. -/
def Stable (signature : Atom → Predicate) (inputs : Producer → Predicate → Prop)
    (propose : Producer → Atoms Atom → Atoms Atom) (old current : Atoms Atom)
    (producer : Producer) : Prop :=
  ¬ Affected signature inputs old current producer →
    Sub (propose producer current) (propose producer old)

/-- A producer's proposals depend only on the carrier's atoms whose predicates
it registered. -/
def ReadsOnly (signature : Atom → Predicate) (inputs : Producer → Predicate → Prop)
    (propose : Producer → Atoms Atom → Atoms Atom) (producer : Producer) : Prop :=
  ∀ left right : Atoms Atom,
    (∀ atom, inputs producer (signature atom) → (left atom ↔ right atom)) →
      propose producer left = propose producer right

/-- A producer that reads only its registered predicates is stable when the
carrier grows. Unaffectedness makes every current atom with a registered
predicate old; growth gives the converse; so the two carriers agree on every
atom the producer reads, and so do its proposals. Without growth a lost input
could enable a proposal that was not old. -/
theorem stable_of_reads_only (signature : Atom → Predicate)
    (inputs : Producer → Predicate → Prop) (propose : Producer → Atoms Atom → Atoms Atom)
    (old current : Atoms Atom) (producer : Producer)
    (reads : ReadsOnly signature inputs propose producer)
    (grown : Sub old current) :
    Stable signature inputs propose old current producer := by
  classical
  intro unaffected
  have same : propose producer current = propose producer old := by
    apply reads
    intro atom registered
    constructor
    · intro present
      apply Classical.byContradiction
      intro absent
      exact unaffected ⟨atom, present, absent, registered⟩
    · intro wasOld
      exact grown atom wasOld
  rw [same]
  exact sub_refl _

/-- Selecting every affected producer of a stable family preserves the full
inflationary step when all old heads were already published.

A full proposal either belongs to a selected producer, or its producer is
unaffected. In the latter case stability makes it an old proposal, which
completed history already retains. Conversely, selecting fewer producers creates
no proposal outside the family. -/
theorem selective_step_exact_of_stable (producers : List Producer)
    (propose : Producer → Atoms Atom → Atoms Atom) (signature : Atom → Predicate)
    (inputs : Producer → Predicate → Prop) (selected : Producer → Prop)
    (old current : Atoms Atom)
    (stable : ∀ producer ∈ producers, Stable signature inputs propose old current producer)
    (woken : ∀ producer, Affected signature inputs old current producer → selected producer)
    (oldHeads : Sub (proposeAll producers propose old) current) :
    Union current (proposeSelectedOf producers propose selected current) =
      Union current (proposeAll producers propose current) := by
  classical
  apply atoms_ext
  intro atom
  constructor
  · rintro (retained | ⟨producer, member, _, proposed⟩)
    · exact Or.inl retained
    · exact Or.inr ⟨producer, member, proposed⟩
  · rintro (retained | ⟨producer, member, proposed⟩)
    · exact Or.inl retained
    · by_cases chosen : selected producer
      · exact Or.inr ⟨producer, member, chosen, proposed⟩
      · have unaffected : ¬ Affected signature inputs old current producer := by
          intro changed
          exact chosen (woken producer changed)
        have wasProposed : propose producer old atom :=
          stable producer member unaffected atom proposed
        exact Or.inl (oldHeads atom ⟨producer, member, wasProposed⟩)

/-- An empty wake set of a stable family establishes possible-head closure
under the same old-head history premise. It does not discharge constraints or
establish that the carrier is an answer set. -/
theorem empty_selection_closed_of_stable (producers : List Producer)
    (propose : Producer → Atoms Atom → Atoms Atom) (signature : Atom → Predicate)
    (inputs : Producer → Predicate → Prop) (selected : Producer → Prop)
    (old current : Atoms Atom)
    (stable : ∀ producer ∈ producers, Stable signature inputs propose old current producer)
    (woken : ∀ producer, Affected signature inputs old current producer → selected producer)
    (oldHeads : Sub (proposeAll producers propose old) current)
    (empty : ∀ producer, ¬ selected producer) :
    SourceSupport.Closed (proposeAll producers propose) current := by
  have sameStep : Union current (proposeSelectedOf producers propose selected current) =
      Union current (proposeAll producers propose current) :=
    selective_step_exact_of_stable producers propose signature inputs selected old current
      stable woken oldHeads
  intro atom proposed
  have full : Union current (proposeAll producers propose current) atom := Or.inr proposed
  have restricted : Union current (proposeSelectedOf producers propose selected current) atom := by
    rw [sameStep]
    exact full
  rcases restricted with retained | ⟨producer, _, chosen, _⟩
  · exact retained
  · exact False.elim (empty producer chosen)

/-- A ground rule's own proposal: its ordinary filtered positive head when its
positive body holds in the carrier. -/
def ruleProposal (entry : Producer × Semantics.Rule Atom) (carrier : Atoms Atom) :
    Atoms Atom :=
  fun atom => entry.2.head = some atom ∧ entry.2.filter ∧ Semantics.Body entry.2 carrier

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

/-- The rule entries of a family, each with its own proposal, propose exactly
the normalized program's possible heads. -/
theorem proposeAll_rules (family : List (Producer × Semantics.Rule Atom))
    (carrier : Atoms Atom) :
    proposeAll family ruleProposal carrier =
      NormalSupport.propose (family.map Prod.snd) carrier := by
  apply atoms_ext
  intro atom
  constructor
  · rintro ⟨entry, member, head, valid, body⟩
    exact ⟨entry.2, List.mem_map.mpr ⟨entry, member, rfl⟩, head, valid, body⟩
  · rintro ⟨rule, member, head, valid, body⟩
    obtain ⟨entry, original, rfl⟩ := List.mem_map.mp member
    exact ⟨entry, original, head, valid, body⟩

/-- A rule entry whose producer is unaffected proposes over the current carrier
only what it proposed over the old one, without any growth premise. -/
theorem rule_stable (signature : Atom → Predicate) (inputs : Producer → Predicate → Prop)
    (old current : Atoms Atom) (entry : Producer × Semantics.Rule Atom)
    (covered : ∀ atom ∈ entry.2.positive, inputs entry.1 (signature atom)) :
    Stable signature (fun entry => inputs entry.1) ruleProposal old current entry := by
  intro unaffected atom proposed
  obtain ⟨head, valid, body⟩ := proposed
  exact ⟨head, valid,
    unaffected_body_old entry.2 entry.1 signature inputs old current covered unaffected body⟩

/-- Selecting every affected producer preserves the full inflationary step
when all old heads were already published.

A full proposal either belongs to a selected producer, or its producer is
unaffected. In the latter case its positive body was old, so completed history
already retains the head. Conversely, selecting fewer producers creates no
proposal outside the original program. Repeated original occurrences may have
distinct owners even when their ground rules are logically equal. The rule
entries form a stable family (`rule_stable`), so this is a corollary of
`selective_step_exact_of_stable`. -/
theorem selective_step_exact (family : List (Producer × Semantics.Rule Atom))
    (signature : Atom → Predicate) (inputs : Producer → Predicate → Prop)
    (selected : Producer → Prop) (old current : Atoms Atom)
    (covered : ∀ entry ∈ family, ∀ atom ∈ entry.2.positive,
      inputs entry.1 (signature atom))
    (woken : ∀ producer, Affected signature inputs old current producer → selected producer)
    (oldHeads : Sub (NormalSupport.propose (family.map Prod.snd) old) current) :
    Union current (proposeSelected family selected current) =
      Union current (NormalSupport.propose (family.map Prod.snd) current) := by
  have general :=
    selective_step_exact_of_stable family ruleProposal signature (fun entry => inputs entry.1)
      (fun entry => selected entry.1) old current
      (fun entry member => rule_stable signature inputs old current entry (covered entry member))
      (fun entry changed => woken entry.1 changed)
      (by rw [proposeAll_rules]; exact oldHeads)
  rw [proposeAll_rules] at general
  exact general

/-- An empty wake set establishes possible-head closure under the same complete
registration and old-head history premises. It does not discharge constraints
or establish that the carrier is an answer set. A corollary of
`empty_selection_closed_of_stable` for the rule entries. -/
theorem empty_selection_closed (family : List (Producer × Semantics.Rule Atom))
    (signature : Atom → Predicate) (inputs : Producer → Predicate → Prop)
    (selected : Producer → Prop) (old current : Atoms Atom)
    (covered : ∀ entry ∈ family, ∀ atom ∈ entry.2.positive,
      inputs entry.1 (signature atom))
    (woken : ∀ producer, Affected signature inputs old current producer → selected producer)
    (oldHeads : Sub (NormalSupport.propose (family.map Prod.snd) old) current)
    (empty : ∀ producer, ¬ selected producer) :
    SourceSupport.Closed (NormalSupport.propose (family.map Prod.snd)) current := by
  have general :=
    empty_selection_closed_of_stable family ruleProposal signature (fun entry => inputs entry.1)
      (fun entry => selected entry.1) old current
      (fun entry member => rule_stable signature inputs old current entry (covered entry member))
      (fun entry changed => woken entry.1 changed)
      (by rw [proposeAll_rules]; exact oldHeads)
      (fun entry => empty entry.1)
  have sameStep : proposeAll family ruleProposal = NormalSupport.propose (family.map Prod.snd) :=
    funext (proposeAll_rules family)
  rw [sameStep] at general
  exact general

end Zetesis.ProducerScheduling
