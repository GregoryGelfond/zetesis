import Zetesis.PossibleFoundedness

/-!
# Erasing unsupported atoms after a completed positive closure

The central law concerns a least closed set, not answer-set membership. Erasing
atoms outside that set from both inputs and outputs of a monotone operator
preserves its least closed set. A concrete positive formula relaxation and the
existing possible-foundedness producer definitions instantiate that law.

The Rust implementation must still establish complete least closure, exact
producer/occurrence coverage, cyclic versus boundary roles, direct publication's
mask effect, complete publication before rearming, and uninterrupted false-event
history. No declaration here assumes a Rust-refinement predicate or proves those
implementation obligations by naming them.
-/

namespace Zetesis.PossibleFoundedness.PublicationErasure

universe u
variable {α : Type u}
open Ferraris DisjunctiveSupport

/-- Remove a set of atoms from an interpretation. -/
def erase (cut X : Atoms α) : Atoms α := fun a => X a ∧ ¬ cut a

/-- An operator can only enable more outputs when its inputs grow. -/
def MonotoneStep (step : Atoms α → Atoms α) : Prop :=
  ∀ X Y, Sub X Y → Sub (step X) (step Y)

/-- A closed set contains every output of its operator. -/
def Closed (step : Atoms α → Atoms α) (X : Atoms α) : Prop := Sub (step X) X

/-- Leastness is among all closed sets, not merely a completed execution prefix. -/
def LeastClosed (step : Atoms α → Atoms α) (P : Atoms α) : Prop :=
  Closed step P ∧ ∀ X, Closed step X → Sub P X

/-- Direct erasure changes only atom inputs and permitted atom outputs. -/
def erasedStep (step : Atoms α → Atoms α) (cut : Atoms α) (X : Atoms α) : Atoms α :=
  erase cut (step (erase cut X))

/-- Erasing only atoms absent from a least closed set preserves that least set.

For minimality, intersect any closed set of the erased operator with the old
least set. Monotonicity places every output inside the old least set and hence
outside the cut. The erased closure therefore contains those outputs too. The
intersection is closed for the original operator, so original leastness applies.
No finiteness, enumeration order, chosen iteration count, or second least-set
existence assumption is needed.
-/
theorem erasure_preserves_least_closed (step : Atoms α → Atoms α)
    (cut P : Atoms α) (monotone : MonotoneStep step)
    (least : LeastClosed step P) (absent : ∀ a, P a → ¬ cut a) :
    LeastClosed (erasedStep step cut) P := by
  have remainsClosed : Closed (erasedStep step cut) P := by
    intro a produced
    have inputBelow : Sub (erase cut P) P := fun _ present => present.1
    exact least.1 a (monotone (erase cut P) P inputBelow a produced.1)
  refine ⟨remainsClosed, ?_⟩
  intro X closed
  have intersectionClosed : Closed step (intersection X P) := by
    intro a produced
    have inputBelow : Sub (intersection X P) P := fun _ present => present.2
    have possible : P a := least.1 a
      (monotone (intersection X P) P inputBelow a produced)
    have erasedInput : Sub (intersection X P) (erase cut X) := by
      intro b present
      exact ⟨present.1, absent b present.2⟩
    have erasedOutput : erasedStep step cut X a :=
      ⟨monotone (intersection X P) (erase cut X) erasedInput a produced,
        absent a possible⟩
    exact ⟨closed a erasedOutput, possible⟩
  exact fun a possible => (least.2 (intersection X P) intersectionClosed a possible).1

/-- The positive body relaxation. False information is frozen during this
operator: atoms, conjunctions and disjunctions retain positive dependencies;
other implications are opaque possible seeds unless already disabled.
This is a formula-level model, not the indexed Rust DAG or aggregate refinement.
-/
def positive (disabled : Formula α → Prop) (X : Atoms α) : Formula α → Prop
  | .atom a => X a ∧ ¬ disabled (.atom a)
  | .bot => False
  | .conj F G => positive disabled X F ∧ positive disabled X G ∧ ¬ disabled (.conj F G)
  | .disj F G => (positive disabled X F ∨ positive disabled X G) ∧ ¬ disabled (.disj F G)
  | .imp F G => ¬ disabled (.imp F G)

/-- Every retained positive dependency is monotone; opaque seeds are unchanged. -/
theorem positive_monotone (disabled : Formula α → Prop) (X Y : Atoms α)
    (below : Sub X Y) (F : Formula α) :
    positive disabled X F → positive disabled Y F := by
  induction F with
  | atom a => exact fun present => ⟨below a present.1, present.2⟩
  | bot => exact fun impossible => impossible
  | conj F G left right =>
    exact fun present => ⟨left present.1, right present.2.1, present.2.2⟩
  | disj F G left right =>
    intro present
    rcases present.1 with first | second
    · exact ⟨Or.inl (left first), present.2⟩
    · exact ⟨Or.inr (right second), present.2⟩
  | imp F G _ _ => exact fun present => present

/-- Publication immediately disables occurrences of cut atoms. It does not yet
change opaque body masks or propagate through Boolean parents. -/
def directDisabled (disabled : Formula α → Prop) (cut : Atoms α) : Formula α → Prop :=
  fun F => disabled F ∨ match F with
    | .atom a => cut a
    | _ => False

/-- Direct atom-occurrence disabling is exactly input erasure for this relaxation. -/
theorem positive_direct_erasure (disabled : Formula α → Prop) (cut X : Atoms α)
    (F : Formula α) :
    positive (directDisabled disabled cut) X F ↔ positive disabled (erase cut X) F := by
  induction F with
  | atom a =>
    constructor
    · intro present
      exact ⟨⟨present.1, fun removed => present.2 (Or.inr removed)⟩,
        fun blocked => present.2 (Or.inl blocked)⟩
    · intro present
      refine ⟨present.1.1, ?_⟩
      intro blocked
      rcases blocked with previous | removed
      · exact present.2 previous
      · exact present.1.2 removed
  | bot => exact Iff.rfl
  | conj F G left right =>
    simp only [positive, directDisabled, or_false, left, right]
  | disj F G left right =>
    simp only [positive, directDisabled, or_false, left, right]
  | imp F G _ _ => simp only [positive, directDisabled, or_false]

/-- One upper-closure step: permitted boundary seeds plus every enabled ordinary
or choice head. It reuses PossibleFoundedness's exact producer definitions.
For the cyclic implementation, boundary denotes the noncyclic conceptual seeds.
-/
def producerStep (allowed boundary : Atoms α) (upper : Atoms α → Formula α → Prop)
    (rules : List (Producer α)) (choices : List (ChoiceProducer α))
    (X : Atoms α) : Atoms α := fun a =>
  allowed a ∧ (boundary a ∨
    (∃ r ∈ rules, producerEnabled (upper X) r ∧ r.head.Contains a) ∨
    (∃ c ∈ choices, choiceEnabled (upper X) c ∧ c.head = a))

/-- Closure here is precisely permitted boundary inclusion and the two existing
possible-foundedness producer-closure conditions. -/
theorem producer_step_closed (allowed boundary P : Atoms α)
    (upper : Atoms α → Formula α → Prop)
    (rules : List (Producer α)) (choices : List (ChoiceProducer α)) :
    Closed (producerStep allowed boundary upper rules choices) P ↔
      (Sub (intersection allowed boundary) P ∧
        ProducersClosed P allowed (upper P) rules ∧
        ChoicesClosed P allowed (upper P) choices) := by
  constructor
  · intro closed
    refine ⟨?_, ?_, ?_⟩
    · intro a present
      exact closed a ⟨present.1, Or.inl present.2⟩
    · intro r member enabled a occurs permitted
      exact closed a ⟨permitted, Or.inr (Or.inl ⟨r, member, enabled, occurs⟩)⟩
    · intro c member enabled permitted
      exact closed c.head ⟨permitted, Or.inr (Or.inr ⟨c, member, enabled, rfl⟩)⟩
  · intro components a produced
    rcases produced.2 with boundarySeed | ordinary | choice
    · exact components.1 a ⟨produced.1, boundarySeed⟩
    · obtain ⟨r, member, enabled, occurs⟩ := ordinary
      exact components.2.1 r member enabled a occurs produced.1
    · obtain ⟨c, member, enabled, same⟩ := choice
      subst a
      exact components.2.2 c member enabled produced.1

/-- A positive producer step is monotone, including all head occurrences and
both orders of atomic choice. No Rust monotonicity predicate is assumed. -/
theorem positive_step_monotone (allowed boundary : Atoms α)
    (disabled : Formula α → Prop)
    (rules : List (Producer α)) (choices : List (ChoiceProducer α)) :
    MonotoneStep (producerStep allowed boundary (positive disabled) rules choices) := by
  intro X Y below a produced
  refine ⟨produced.1, ?_⟩
  rcases produced.2 with seed | ordinary | choice
  · exact Or.inl seed
  · obtain ⟨r, member, enabled, occurs⟩ := ordinary
    have staysEnabled : producerEnabled (positive disabled Y) r := by
      cases r with
      | fact H => exact trivial
      | rule B H => exact positive_monotone disabled X Y below B enabled
    exact Or.inr (Or.inl ⟨r, member, staysEnabled, occurs⟩)
  · obtain ⟨c, member, enabled, same⟩ := choice
    have staysEnabled : choiceEnabled (positive disabled Y) c := by
      cases c with
      | fact b => exact trivial
      | reversedFact b => exact trivial
      | rule B b => exact positive_monotone disabled X Y below B enabled
      | reversedRule B b => exact positive_monotone disabled X Y below B enabled
    exact Or.inr (Or.inr ⟨c, member, staysEnabled, same⟩)

/-- The concrete post-publication operator is the erasure of the old operator.
The boundary inventory is fixed; output permission removes any cut seed too.
-/
theorem direct_step_erasure (allowed boundary cut X : Atoms α)
    (disabled : Formula α → Prop)
    (rules : List (Producer α)) (choices : List (ChoiceProducer α)) (a : α) :
    producerStep (erase cut allowed) boundary (positive (directDisabled disabled cut))
      rules choices X a ↔
    erasedStep (producerStep allowed boundary (positive disabled) rules choices) cut X a := by
  have upperEqual : positive (directDisabled disabled cut) X =
      positive disabled (erase cut X) := by
    funext F
    exact propext (positive_direct_erasure disabled cut X F)
  unfold producerStep erasedStep erase
  rw [upperEqual]
  constructor
  · intro produced
    exact ⟨⟨produced.1.1, produced.2⟩, produced.1.2⟩
  · intro produced
    exact ⟨⟨produced.1.1, produced.2⟩, produced.1.2⟩

/-- A completed positive least closure remains exactly the least closure after
all newly excluded atoms are published, before dependent ordinary propagation.

This conclusion is stronger than safe exclusion: it justifies the mathematical
baseline from which a subsequent delta can be collected. The statement does not
say that a partially published prefix, an interrupted queue, a foreign owner, or
an arbitrary closed overapproximation supplies the Rust replay precondition.
-/
theorem completed_publication_preserves_least (allowed boundary cut P : Atoms α)
    (disabled : Formula α → Prop)
    (rules : List (Producer α)) (choices : List (ChoiceProducer α))
    (least : LeastClosed (producerStep allowed boundary (positive disabled) rules choices) P)
    (absent : ∀ a, P a → ¬ cut a) :
    LeastClosed
      (producerStep (erase cut allowed) boundary (positive (directDisabled disabled cut))
        rules choices) P := by
  have operatorEqual :
      producerStep (erase cut allowed) boundary (positive (directDisabled disabled cut))
        rules choices =
      erasedStep (producerStep allowed boundary (positive disabled) rules choices) cut := by
    funext X a
    exact propext (direct_step_erasure allowed boundary cut X disabled rules choices a)
  rw [operatorEqual]
  exact erasure_preserves_least_closed _ cut P
    (positive_step_monotone allowed boundary disabled rules choices) least absent

/-- Publication removes every permitted cyclic head outside the completed set.
This complete inventory is stronger than the disjointness needed by erasure. -/
def missing (cyclic allowed P : Atoms α) : Atoms α :=
  fun a => cyclic a ∧ allowed a ∧ ¬ P a

/-- Every cyclic head surviving the whole missing-head publication belongs to
the completed closure. Publishing only a proper prefix does not supply this
premise, even though erasing that prefix also preserves the least closure. -/
theorem surviving_cyclic_heads_possible (cyclic allowed P : Atoms α) (a : α)
    (isCyclic : cyclic a) (survives : erase (missing cyclic allowed P) allowed a) :
    P a := by
  classical
  by_cases possible : P a
  · exact possible
  · exact False.elim (survives.2 ⟨isCyclic, survives.1, possible⟩)

/-- Whole publication provides both parts of the next replay baseline: the
same least closure and membership of every remaining permitted cyclic head.
No ordinary propagation is included in this instantaneous boundary. -/
theorem whole_publication_baseline (allowed boundary cyclic P : Atoms α)
    (disabled : Formula α → Prop)
    (rules : List (Producer α)) (choices : List (ChoiceProducer α))
    (least : LeastClosed (producerStep allowed boundary (positive disabled) rules choices) P) :
    LeastClosed
        (producerStep (erase (missing cyclic allowed P) allowed) boundary
          (positive (directDisabled disabled (missing cyclic allowed P))) rules choices) P ∧
      ∀ a, cyclic a → erase (missing cyclic allowed P) allowed a → P a := by
  have absent : ∀ a, P a → ¬ missing cyclic allowed P a := by
    intro a possible removed
    exact removed.2.2 possible
  exact ⟨completed_publication_preserves_least allowed boundary
    (missing cyclic allowed P) P disabled rules choices least absent,
    surviving_cyclic_heads_possible cyclic allowed P⟩

end Zetesis.PossibleFoundedness.PublicationErasure
