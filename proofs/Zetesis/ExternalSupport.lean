import Zetesis.DisjunctiveSupport
import Zetesis.Search

/-!
# External support of a set of atoms

`DisjunctiveSupport` shows that an answer-set atom needs a producer that
supports it. Some enabled producers cannot be that support. A rule whose body
requires its own head adds nothing the head does not already assume, and
neither does a rule `a ← b` when every producer of `b` requires `a`: the two
atoms would then hold one another up. This module states the general fact both
follow from, and the support restriction it gives candidate generation.

A formula requires an atom that is one of its conjuncts. A producer supports a
set of atoms from outside when it is enabled, at least one of its heads is true,
all its true heads lie in the set, and its body requires no atom of the set. An
answer set that meets a set has such a producer for it; otherwise deleting the
whole set leaves a proper-subset model of the frozen reduct. Bodies may be
arbitrary formulas, since a body false in the candidate freezes to falsum and
a true body keeps each required atom as a conjunct of its reduct.

For one atom `a`, take the set of `a` and of every atom all of whose producers
require `a`. Outside support of that set is ordinary sole-head or choice support
of `a` by a producer that requires neither `a` nor any such atom. Producers that
do are *circular* for `a`: no answer set rests on them.

The last three laws carry that support into a region: an atom is absent from
every stable model of a region when each of its producers is circular for it
or supports it nowhere there, and a held atom with one producer left forces
that producer. They are what a support scan that passes over circular
producers concludes.

These are necessary candidate restrictions. They do not rewrite the theory,
establish membership, or state that removing a circular producer preserves the
answer sets. The laws do not establish Rust DAG extraction, the recognition of
required atoms and dependent atoms, resource accounting or the component test
that reads the same restriction.
-/
namespace Zetesis.ExternalSupport

universe u
variable {α : Type u}
open Ferraris DisjunctiveSupport

/-- A formula requires an atom that is one of its conjuncts. Disjunctions,
implications and falsum require nothing, whatever they mention. -/
def Requires : Formula α → α → Prop
  | .atom b, a => a = b
  | .conj F G, a => Requires F a ∨ Requires G a
  | .bot, _ => False
  | .disj _ _, _ => False
  | .imp _ _, _ => False

/-- A required atom is true wherever the frozen formula is satisfied.

Proof. By induction on the formula. A required atom's reduct is the atom itself
or falsum. A conjunction's reduct is satisfied only through both parts'
reducts, one of which requires the atom. -/
theorem required_of_reduct (M J : Atoms α) :
    ∀ (F : Formula α) (a : α), Requires F a → Satisfies J (Reduct M F) → J a
  | .atom b, a, required, satisfied => by
    have same : a = b := required
    rw [same]
    exact ((atom_reduct M J b).mp satisfied).2
  | .conj F G, a, required, satisfied => by
    have parts : Satisfies J (Reduct M F) ∧ Satisfies J (Reduct M G) :=
      (RuleFactorization.reduct_conj M J F G).mp satisfied
    rcases required with left | right
    · exact required_of_reduct M J F a left parts.1
    · exact required_of_reduct M J G a right parts.2
  | .bot, _, required, _ => False.elim required
  | .disj _ _, _, required, _ => False.elim required
  | .imp _ _, _, required, _ => False.elim required

/-- The atoms an ordinary producer's body requires; a fact requires none. -/
def RuleRequires : Producer α → α → Prop
  | .fact _, _ => False
  | .rule B _, a => Requires B a

/-- The atoms an atomic choice's body requires; an unconditional choice
requires none. -/
def ChoiceRequires : ChoiceProducer α → α → Prop
  | .fact _, _ => False
  | .reversedFact _, _ => False
  | .rule B _, a => Requires B a
  | .reversedRule B _, a => Requires B a

/-- Delete a set of atoms; all other memberships remain unchanged. -/
def without (M U : Atoms α) : Atoms α := fun b => M b ∧ ¬ U b

/-- An ordinary producer supports a set from outside: it is enabled, some head
is true, every true head lies in the set, and its body requires no atom of it. -/
def RuleSupportsSet (M U : Atoms α) (r : Producer α) : Prop :=
  r.Enabled M ∧ (∃ b, r.head.Contains b ∧ M b) ∧
    (∀ b, r.head.Contains b → M b → U b) ∧ ∀ u, U u → ¬ RuleRequires r u

/-- An atomic choice supports a set from outside: it is enabled, its head is
true and in the set, and its body requires no atom of the set. -/
def ChoiceSupportsSet (M U : Atoms α) (c : ChoiceProducer α) : Prop :=
  c.Enabled M ∧ M c.head ∧ U c.head ∧ ∀ u, U u → ¬ ChoiceRequires c u

/-- Deleting a set preserves the frozen reduct of every ordinary producer that
does not support the set from outside.

Proof. If a true head lies outside the set it survives the deletion and
satisfies the frozen head. Otherwise every true head lies in the set. A fact,
or a rule whose frozen body the smaller interpretation satisfies, is enabled
and has a true head, so by assumption its body requires an atom of the set.
The frozen body would make that atom true after its deletion. -/
theorem producer_withdrawal (M U : Atoms α) (r : Producer α)
    (model : Satisfies M r.formula) (internal : ¬ RuleSupportsSet M U r) :
    Satisfies (without M U) (Reduct M r.formula) := by
  classical
  cases r with
  | fact H =>
    have trueHead : ∃ b, H.Contains b ∧ M b := (head_satisfies M H).mp model
    have survivor : ∃ b, H.Contains b ∧ M b ∧ ¬ U b := by
      apply Classical.byContradiction
      intro none
      have inside : ∀ b, H.Contains b → M b → U b := fun b occurs present =>
        Classical.byContradiction (fun out => none ⟨b, occurs, present, out⟩)
      exact internal ⟨trivial, trueHead, inside, fun _ _ required => required⟩
    obtain ⟨b, occurs, present, out⟩ := survivor
    exact (head_reduct M (without M U) H).mpr ⟨b, occurs, present, present, out⟩
  | rule B H =>
    have rootTrue : Satisfies M (.imp B H.formula) := model
    change Satisfies (without M U) (Reduct M (.imp B H.formula))
    rw [Reduct, if_pos rootTrue]
    intro body
    have enabled : Satisfies M B := TightPlans.original_of_reduct M (without M U) B body
    have trueHead : ∃ b, H.Contains b ∧ M b := (head_satisfies M H).mp (rootTrue enabled)
    have survivor : ∃ b, H.Contains b ∧ M b ∧ ¬ U b := by
      apply Classical.byContradiction
      intro none
      have inside : ∀ b, H.Contains b → M b → U b := fun b occurs present =>
        Classical.byContradiction (fun out => none ⟨b, occurs, present, out⟩)
      have independent : ∀ u, U u → ¬ RuleRequires (.rule B H) u := by
        intro u member required
        have kept : without M U u := required_of_reduct M (without M U) B u required body
        exact kept.2 member
      exact internal ⟨enabled, trueHead, inside, independent⟩
    obtain ⟨b, occurs, present, out⟩ := survivor
    exact (head_reduct M (without M U) H).mpr ⟨b, occurs, present, present, out⟩

/-- The guard of a frozen atomic choice survives the deletion of a set unless
the choice supports the set from outside.

Proof. The guard asks for the head after deletion only when the head is true.
A true head outside the set survives. A true head in the set would make an
enabled choice an outside support unless its body requires an atom of the set,
and the frozen body would make that atom true after its deletion. -/
theorem choice_withdrawal (M U : Atoms α) (c : ChoiceProducer α)
    (internal : ¬ ChoiceSupportsSet M U c) :
    Satisfies (without M U) (Reduct M c.formula) := by
  classical
  -- The guard of every shape: a true head survives once the choice is enabled
  -- and its body requires no atom of the set.
  have guard : c.Enabled M → (∀ u, U u → ¬ ChoiceRequires c u) →
      M c.head → without M U c.head := by
    intro enabled independent present
    refine ⟨present, ?_⟩
    intro member
    exact internal ⟨enabled, present, member, independent⟩
  cases c with
  | fact a =>
    exact (choice_reduct_guard M (without M U) a).mpr
      (guard trivial (fun _ _ required => required))
  | reversedFact a =>
    exact (TightPlans.reversed_choice_reduct_guard M (without M U) a).mpr
      (guard trivial (fun _ _ required => required))
  | rule B a =>
    apply (RuleFactorization.reduct_imp M (without M U) B
      (.disj (.atom a) (Ferraris.Neg (.atom a)))).mpr
    refine ⟨choice_satisfied M (.rule B a), ?_⟩
    intro body
    have enabled : Satisfies M B := TightPlans.original_of_reduct M (without M U) B body
    have independent : ∀ u, U u → ¬ ChoiceRequires (.rule B a) u := by
      intro u member required
      have kept : without M U u := required_of_reduct M (without M U) B u required body
      exact kept.2 member
    exact (choice_reduct_guard M (without M U) a).mpr (guard enabled independent)
  | reversedRule B a =>
    apply (RuleFactorization.reduct_imp M (without M U) B
      (.disj (Ferraris.Neg (.atom a)) (.atom a))).mpr
    refine ⟨choice_satisfied M (.reversedRule B a), ?_⟩
    intro body
    have enabled : Satisfies M B := TightPlans.original_of_reduct M (without M U) B body
    have independent : ∀ u, U u → ¬ ChoiceRequires (.reversedRule B a) u := by
      intro u member required
      have kept : without M U u := required_of_reduct M (without M U) B u required body
      exact kept.2 member
    exact (TightPlans.reversed_choice_reduct_guard M (without M U) a).mpr
      (guard enabled independent)

/-- An answer set that meets a set of atoms has a producer supporting the set
from outside.

Proof. Assume no producer does. Deleting the set preserves every ordinary
producer's reduct and every choice's reduct by the two withdrawal laws, and
every asserted negation because it is frozen. Complete root coverage makes the
smaller interpretation a model of the whole reduct. It is a proper subset
because the answer set meets the set, contradicting stability. -/
theorem answer_set_externally_supported (M : Atoms α) (T : Theory α)
    (rules : List (Producer α)) (choices : List (ChoiceProducer α))
    (covered : CoveredWithChoices T rules choices) (answer : Stable M T)
    (U : Atoms α) (a : α) (member : U a) (present : M a) :
    (∃ r ∈ rules, RuleSupportsSet M U r) ∨ (∃ c ∈ choices, ChoiceSupportsSet M U c) := by
  classical
  apply Classical.byContradiction
  intro unsupported
  have reductModel : Models (without M U) (ReductTheory M T) := by
    intro reduced inside
    obtain ⟨F, asserted, rfl⟩ := List.mem_map.mp inside
    rcases covered F asserted with
      ⟨r, inRules, rfl⟩ | ⟨c, inChoices, rfl⟩ | ⟨G, rfl⟩ | rfl
    · apply producer_withdrawal M U r (answer.1 r.formula asserted)
      exact fun support => unsupported (Or.inl ⟨r, inRules, support⟩)
    · apply choice_withdrawal M U c
      exact fun support => unsupported (Or.inr ⟨c, inChoices, support⟩)
    · apply (TightPlans.negation_frozen M (without M U) G).mpr
      exact answer.1 (Ferraris.Neg G) asserted
    · exact False.elim (answer.1 .bot asserted)
  have proper : ProperSub (without M U) M := by
    refine ⟨fun _ kept => kept.1, ?_⟩
    intro reverse
    exact (reverse a present).2 member
  exact answer.2 ⟨without M U, proper, reductModel⟩

/-- Every producer of the atom requires `a`: the atom is derived only through
`a`. An atom without producers satisfies this vacuously. -/
def DependsOn (rules : List (Producer α)) (choices : List (ChoiceProducer α))
    (a b : α) : Prop :=
  (∀ r ∈ rules, r.head.Contains b → RuleRequires r a) ∧
    ∀ c ∈ choices, c.head = b → ChoiceRequires c a

/-- An atom together with the atoms derived only through it. -/
def Circle (rules : List (Producer α)) (choices : List (ChoiceProducer α))
    (a : α) : Atoms α :=
  fun b => b = a ∨ DependsOn rules choices a b

/-- Every answer-set atom has ordinary sole-head support or enabled choice
support from a producer that requires neither the atom nor any atom derived
only through it.

Proof. The answer set meets the atom's circle, so some producer supports the
circle from outside. Suppose it is an ordinary producer and one of its true
heads is another atom of the circle. That atom is derived only through `a`, so
this producer requires `a`, an atom of the circle, which outside support
excludes. Hence its only true head is `a`, and it has one: it supports `a`.
Suppose it is a choice. Its head is in the circle; were it another atom, the
choice would require `a` in the same way. Hence its head is `a`. -/
theorem answer_set_noncircular_support (M : Atoms α) (T : Theory α)
    (rules : List (Producer α)) (choices : List (ChoiceProducer α))
    (covered : CoveredWithChoices T rules choices) (answer : Stable M T) :
    ∀ a, M a →
      (∃ r ∈ rules, r.Supports M a ∧
        ∀ u, Circle rules choices a u → ¬ RuleRequires r u) ∨
      (∃ c ∈ choices, c.Supports M a ∧
        ∀ u, Circle rules choices a u → ¬ ChoiceRequires c u) := by
  classical
  intro a present
  have own : Circle rules choices a a := Or.inl rfl
  rcases answer_set_externally_supported M T rules choices covered answer
      (Circle rules choices a) a own present with
    ⟨r, inRules, enabled, trueHead, inside, independent⟩ |
    ⟨c, inChoices, enabled, chosen, inside, independent⟩
  · have onlyHead : ∀ b, r.head.Contains b → M b → b = a := by
      intro b occurs member
      rcases inside b occurs member with same | dependent
      · exact same
      · exact False.elim (independent a own (dependent.1 r inRules occurs))
    obtain ⟨b, occurs, member⟩ := trueHead
    have contained : r.head.Contains a := onlyHead b occurs member ▸ occurs
    exact Or.inl ⟨r, inRules, ⟨enabled, contained, onlyHead⟩, independent⟩
  · have head : c.head = a := by
      rcases inside with same | dependent
      · exact same
      · exact False.elim (independent a own (dependent.2 c inChoices rfl))
    exact Or.inr ⟨c, inChoices, ⟨enabled, head⟩, independent⟩

/-- An atom is in no stable model of a region when each of its producers is
circular for it or supports it nowhere in the region. Atomic choices count
among the producers.

Proof. A stable model holding the atom has a supporting producer that requires
no atom of the atom's circle. It is not circular, so by assumption it supports
the atom nowhere in the region, which the model contradicts. -/
theorem noncircular_unsupported_cut (T : Theory α) (rules : List (Producer α))
    (choices : List (ChoiceProducer α)) (covered : CoveredWithChoices T rules choices)
    (c : Region α) (a : α)
    (ordinary : ∀ r ∈ rules, r.head.Contains a →
      (∃ u, Circle rules choices a u ∧ RuleRequires r u) ∨
        ∀ z, c.Contains z → Stable z T → ¬ r.Supports z a)
    (chosen : ∀ r ∈ choices, r.head = a →
      (∃ u, Circle rules choices a u ∧ ChoiceRequires r u) ∨
        ∀ z, c.Contains z → Stable z T → ¬ r.Supports z a) :
    ∀ z, c.Contains z → Stable z T → ¬ z a := by
  intro z inside stable present
  rcases answer_set_noncircular_support z T rules choices covered stable a present with
    ⟨r, member, support, independent⟩ | ⟨r, member, support, independent⟩
  · rcases ordinary r member support.2.1 with ⟨u, inCircle, required⟩ | nowhere
    · exact independent u inCircle required
    · exact nowhere z inside stable support
  · rcases chosen r member support.2 with ⟨u, inCircle, required⟩ | nowhere
    · exact independent u inCircle required
    · exact nowhere z inside stable support

/-- An atom held in a stable model of a region, each producer of which but one
ordinary producer is circular for it or supports it nowhere in the region, is
supported by that producer, so its body holds there.

Proof. The model has a supporting producer that is not circular. Were it
another producer than the one left, it would support the atom nowhere in the
region. -/
theorem noncircular_sole_rule_forces (T : Theory α) (rules : List (Producer α))
    (choices : List (ChoiceProducer α)) (covered : CoveredWithChoices T rules choices)
    (c : Region α) (a : α) (r : Producer α)
    (ordinary : ∀ r' ∈ rules, r'.head.Contains a → r' ≠ r →
      (∃ u, Circle rules choices a u ∧ RuleRequires r' u) ∨
        ∀ z, c.Contains z → Stable z T → ¬ r'.Supports z a)
    (chosen : ∀ r' ∈ choices, r'.head = a →
      (∃ u, Circle rules choices a u ∧ ChoiceRequires r' u) ∨
        ∀ z, c.Contains z → Stable z T → ¬ r'.Supports z a) :
    ∀ z, c.Contains z → Stable z T → z a → r.Enabled z := by
  intro z inside stable present
  rcases answer_set_noncircular_support z T rules choices covered stable a present with
    ⟨r', member, support, independent⟩ | ⟨r', member, support, independent⟩
  · by_cases same : r' = r
    · subst same
      exact support.1
    · rcases ordinary r' member support.2.1 same with ⟨u, inCircle, required⟩ | nowhere
      · exact (independent u inCircle required).elim
      · exact (nowhere z inside stable support).elim
  · rcases chosen r' member support.2 with ⟨u, inCircle, required⟩ | nowhere
    · exact (independent u inCircle required).elim
    · exact (nowhere z inside stable support).elim

/-- The same with the one producer left an atomic choice: it is enabled in
every stable model of the region holding the atom. -/
theorem noncircular_sole_choice_forces (T : Theory α) (rules : List (Producer α))
    (choices : List (ChoiceProducer α)) (covered : CoveredWithChoices T rules choices)
    (c : Region α) (a : α) (r : ChoiceProducer α)
    (ordinary : ∀ r' ∈ rules, r'.head.Contains a →
      (∃ u, Circle rules choices a u ∧ RuleRequires r' u) ∨
        ∀ z, c.Contains z → Stable z T → ¬ r'.Supports z a)
    (chosen : ∀ r' ∈ choices, r'.head = a → r' ≠ r →
      (∃ u, Circle rules choices a u ∧ ChoiceRequires r' u) ∨
        ∀ z, c.Contains z → Stable z T → ¬ r'.Supports z a) :
    ∀ z, c.Contains z → Stable z T → z a → r.Enabled z := by
  intro z inside stable present
  rcases answer_set_noncircular_support z T rules choices covered stable a present with
    ⟨r', member, support, independent⟩ | ⟨r', member, support, independent⟩
  · rcases ordinary r' member support.2.1 with ⟨u, inCircle, required⟩ | nowhere
    · exact (independent u inCircle required).elim
    · exact (nowhere z inside stable support).elim
  · by_cases same : r' = r
    · subst same
      exact support.1
    · rcases chosen r' member support.2 same with ⟨u, inCircle, required⟩ | nowhere
      · exact (independent u inCircle required).elim
      · exact (nowhere z inside stable support).elim

end Zetesis.ExternalSupport
