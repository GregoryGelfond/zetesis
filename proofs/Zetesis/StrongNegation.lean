import Zetesis.ChoiceIntervals

/-!
# Strong negation as coherent signed atom identities

Positive and strongly negative occurrences are distinct atomic identities.
The base type denotes complete ground atoms, including predicates and argument
tuples; coherence is not a restriction on predicate names alone.
Default negation remains implication to bottom. An injective carrier encoding
preserves formula truth, every frozen reduct, and subset-minimal stability.
Finite coherence constraints rule out candidates containing both polarities of
the same complete base atom, without deriving or supporting either polarity.

The finite coherence registry and its coverage are explicit. These laws do not
verify source recognition, themelios raising, signed predicate/tuple interning,
Rust carrier construction, generated constraints, output spelling, optimization,
resource accounting, or a backend implementation.
-/

namespace Zetesis.StrongNegation

open Ferraris

universe u v
variable {α : Type u} {β : Type v}

/-- An executable atom renaming preserves every connective, including default
    negation and arbitrary recursive/disjunctive Ferraris formulas. -/
def rename (encode : α → β) : Formula α → Formula β
  | .atom atom => .atom (encode atom)
  | .bot => .bot
  | .conj left right => .conj (rename encode left) (rename encode right)
  | .disj left right => .disj (rename encode left) (rename encode right)
  | .imp left right => .imp (rename encode left) (rename encode right)

def renameTheory (encode : α → β) (theory : Theory α) : Theory β :=
  theory.map (rename encode)

def Pull (encode : α → β) (target : Atoms β) : Atoms α := fun atom => target (encode atom)

def Push (encode : α → β) (source : Atoms α) : Atoms β :=
  fun atom => ∃ original, source original ∧ encode original = atom

theorem satisfies_rename (encode : α → β) (target : Atoms β) (F : Formula α) :
    Satisfies target (rename encode F) ↔ Satisfies (Pull encode target) F := by
  induction F with
  | atom atom => rfl
  | bot => rfl
  | conj F G ihF ihG => exact and_congr ihF ihG
  | disj F G ihF ihG => exact or_congr ihF ihG
  | imp F G ihF ihG => exact imp_congr ihF ihG

/-- Renaming commutes with the actual syntax-tree reduct, not just its classical
    truth; this does not require a subset relation between interpretations. -/
theorem reduct_rename (encode : α → β) (target : Atoms β) (F : Formula α) :
    Reduct target (rename encode F) = rename encode (Reduct (Pull encode target) F) := by
  classical
  induction F with
  | atom atom =>
    by_cases present : target (encode atom) <;> simp [rename, Reduct, Pull, present]
  | bot => rfl
  | conj F G ihF ihG =>
    by_cases truth : Satisfies (Pull encode target) (.conj F G)
    · simp only [Reduct, rename, ← satisfies_rename encode target, ihF, ihG] at *
      simp_all [rename]
    · simp only [Reduct, rename, ← satisfies_rename encode target, ihF, ihG] at *
      simp_all [rename]
  | disj F G ihF ihG =>
    by_cases truth : Satisfies (Pull encode target) (.disj F G)
    · simp only [Reduct, rename, ← satisfies_rename encode target, ihF, ihG] at *
      simp_all [rename]
    · simp only [Reduct, rename, ← satisfies_rename encode target, ihF, ihG] at *
      simp_all [rename]
  | imp F G ihF ihG =>
    by_cases truth : Satisfies (Pull encode target) (.imp F G)
    · simp only [Reduct, rename, ← satisfies_rename encode target, ihF, ihG] at *
      simp_all [rename]
    · simp only [Reduct, rename, ← satisfies_rename encode target, ihF, ihG] at *
      simp_all [rename]

theorem frozen_rename (encode : α → β) (M J : Atoms β) (F : Formula α) :
    Satisfies J (Reduct M (rename encode F)) ↔
      Satisfies (Pull encode J) (Reduct (Pull encode M) F) := by
  rw [reduct_rename, satisfies_rename]

theorem models_rename (encode : α → β) (M : Atoms β) (theory : Theory α) :
    Models M (renameTheory encode theory) ↔ Models (Pull encode M) theory := by
  simp [Models, renameTheory, satisfies_rename]

theorem frozen_theory_rename (encode : α → β) (M J : Atoms β) (theory : Theory α) :
    Models J (ReductTheory M (renameTheory encode theory)) ↔
      Models (Pull encode J) (ReductTheory (Pull encode M) theory) := by
  simp [Models, ReductTheory, renameTheory, frozen_rename]

theorem pull_push (encode : α → β) (injective : Function.Injective encode) (M : Atoms α) :
    Pull encode (Push encode M) = M := by
  apply atoms_ext
  intro atom
  constructor
  · rintro ⟨original, present, same⟩
    exact injective same ▸ present
  · intro present
    exact ⟨atom, present, rfl⟩

theorem push_monotone (encode : α → β) (J M : Atoms α) (subset : Sub J M) :
    Sub (Push encode J) (Push encode M) := by
  rintro atom ⟨original, present, same⟩
  exact ⟨original, subset original present, same⟩

theorem pull_monotone (encode : α → β) (J M : Atoms β) (subset : Sub J M) :
    Sub (Pull encode J) (Pull encode M) := fun atom present => subset (encode atom) present

theorem image_sub (encode : α → β) (M : Atoms β) : Sub (Push encode (Pull encode M)) M := by
  rintro atom ⟨original, present, same⟩
  exact same ▸ present

/-- Any subset of an encoded candidate still consists solely of encoded atoms. -/
theorem subset_image (encode : α → β) (M : Atoms α) (J : Atoms β)
    (subset : Sub J (Push encode M)) : Push encode (Pull encode J) = J := by
  apply sub_antisymm (image_sub encode J)
  intro atom present
  obtain ⟨original, _, same⟩ := subset atom present
  exact ⟨original, (show J (encode original) from same.symm ▸ present), same⟩

/-- Injective identity transport preserves the independent minimal-reduct
    definition, rather than identifying strong negation with default negation. -/
theorem stable_push (encode : α → β) (injective : Function.Injective encode)
    (M : Atoms α) (theory : Theory α) :
    Stable (Push encode M) (renameTheory encode theory) ↔ Stable M theory := by
  constructor
  · intro stable
    refine ⟨?_, ?_⟩
    · simpa only [models_rename, pull_push encode injective] using stable.1
    · rintro ⟨J, ⟨subset, strict⟩, models⟩
      apply stable.2
      refine ⟨Push encode J, ⟨push_monotone encode J M subset, ?_⟩, ?_⟩
      · intro backwards
        apply strict
        simpa only [pull_push encode injective] using pull_monotone encode _ _ backwards
      · simpa only [frozen_theory_rename, pull_push encode injective] using models
  · intro stable
    refine ⟨?_, ?_⟩
    · simpa only [models_rename, pull_push encode injective] using stable.1
    · rintro ⟨J, ⟨subset, strict⟩, models⟩
      apply stable.2
      refine ⟨Pull encode J, ⟨?_, ?_⟩, ?_⟩
      · simpa only [pull_push encode injective] using pull_monotone encode _ _ subset
      · intro backwards
        apply strict
        rw [← subset_image encode M J subset]
        exact push_monotone encode _ _ backwards
      · simpa only [frozen_theory_rename, pull_push encode injective] using models

/-- A stable target interpretation cannot contain atoms outside the encoded
    carrier: deleting them preserves the complete original frozen reduct. -/
theorem stable_target_is_image (encode : α → β) (injective : Function.Injective encode)
    (M : Atoms β) (theory : Theory α) (stable : Stable M (renameTheory encode theory)) :
    Push encode (Pull encode M) = M := by
  have original := (models_reduct_self M (renameTheory encode theory)).mpr stable.1
  have projected : Models (Push encode (Pull encode M))
      (ReductTheory M (renameTheory encode theory)) := by
    rw [frozen_theory_rename, pull_push encode injective]
    exact (frozen_theory_rename encode M M theory).mp original
  exact sub_antisymm (image_sub encode M)
    (((stable_iff_minimal_reduct M _).mp stable).2 _ (image_sub encode M) projected)

theorem stable_target_iff (encode : α → β) (injective : Function.Injective encode)
    (M : Atoms β) (theory : Theory α) :
    Stable M (renameTheory encode theory) ↔
      Push encode (Pull encode M) = M ∧ Stable (Pull encode M) theory := by
  constructor
  · intro stable
    have image := stable_target_is_image encode injective M theory stable
    exact ⟨image, (stable_push encode injective _ theory).mp (image.symm ▸ stable)⟩
  · rintro ⟨image, stable⟩
    rw [← image]
    exact (stable_push encode injective _ theory).mpr stable

/-- Strongly negative atoms are genuine carrier atoms, distinct from the
    positive atom and from a default-negated formula. -/
inductive Signed (α : Type u) where
  | positive : α → Signed α
  | negative : α → Signed α
  deriving DecidableEq

def base : Signed α → α
  | .positive atom => atom
  | .negative atom => atom

theorem polarity_distinct (atom : α) : Signed.positive atom ≠ Signed.negative atom := by
  intro impossible
  cases impossible

theorem encoded_polarity_distinct (encode : Signed α → β)
    (injective : Function.Injective encode) (atom : α) :
    encode (.positive atom) ≠ encode (.negative atom) :=
  fun same => polarity_distinct atom (injective same)

def Coherent (M : Atoms (Signed α)) : Prop :=
  ∀ atom, ¬ (M (.positive atom) ∧ M (.negative atom))

def CoherentOn (bases : List α) (M : Atoms (Signed α)) : Prop :=
  ∀ atom ∈ bases, ¬ (M (.positive atom) ∧ M (.negative atom))

/-- A registry covers a model when every selected signed identity has its
    complete base atom in the registry. It need not enumerate absent tuples. -/
def Covered (bases : List α) (M : Atoms (Signed α)) : Prop :=
  ∀ atom, M atom → base atom ∈ bases

def coherenceConstraint (atom : α) : Formula (Signed α) :=
  Neg (.conj (.atom (.positive atom)) (.atom (.negative atom)))

def coherenceTheory (bases : List α) : Theory (Signed α) :=
  bases.map coherenceConstraint

def CoherentStable (M : Atoms (Signed α)) (theory : Theory (Signed α)) : Prop :=
  Coherent M ∧ Stable M theory

theorem coherent_on_of_coherent (bases : List α) (M : Atoms (Signed α))
    (coherent : Coherent M) : CoherentOn bases M := fun atom _ => coherent atom

theorem coherent_iff_on_covered (bases : List α) (M : Atoms (Signed α))
    (covered : Covered bases M) : Coherent M ↔ CoherentOn bases M := by
  constructor
  · exact coherent_on_of_coherent bases M
  · intro coherent atom both
    exact coherent atom (covered (.positive atom) both.1) both

theorem coherence_classical (bases : List α) (M : Atoms (Signed α)) :
    Models M (coherenceTheory bases) ↔ CoherentOn bases M := by
  simp [Models, coherenceTheory, coherenceConstraint, CoherentOn, Ferraris.Neg, Satisfies]

/-- Coherence forbids a candidate pair but supplies no reduct support for either
    polarity. The tested interpretation J is otherwise unrestricted. -/
theorem coherence_frozen (bases : List α) (M J : Atoms (Signed α)) :
    Models J (ReductTheory M (coherenceTheory bases)) ↔ CoherentOn bases M := by
  simp [Models, ReductTheory, coherenceTheory, coherenceConstraint,
    ChoiceIntervals.negation_frozen, Satisfies, CoherentOn]

theorem coherence_stable_filter (bases : List α) (M : Atoms (Signed α))
    (theory : Theory (Signed α)) :
    Stable M (coherenceTheory bases ++ theory) ↔ Stable M theory ∧ CoherentOn bases M := by
  classical
  have frozen : ∀ J, Models J (ReductTheory M (coherenceTheory bases ++ theory)) ↔
      CoherentOn bases M ∧ Models J (ReductTheory M theory) := by
    intro J
    simp only [ReductTheory, List.map_append, RuleFactorization.models_append]
    rw [← ReductTheory, coherence_frozen]
  by_cases coherent : CoherentOn bases M
  · simp only [Stable, RuleFactorization.models_append, coherence_classical,
      frozen, coherent, true_and, and_true]
  · simp only [Stable, RuleFactorization.models_append, coherence_classical,
      coherent, false_and, and_false]

/-- Compile ordinary finite formulas over signed identities and append one
    candidate-only coherence constraint per complete base atom registry entry. -/
def compile (encode : Signed α → β) (bases : List α) (theory : Theory (Signed α)) : Theory β :=
  renameTheory encode (coherenceTheory bases ++ theory)

theorem compiled_classical (encode : Signed α → β) (M : Atoms β) (bases : List α)
    (theory : Theory (Signed α)) :
    Models M (compile encode bases theory) ↔
      CoherentOn bases (Pull encode M) ∧ Models (Pull encode M) theory := by
  rw [compile, models_rename, RuleFactorization.models_append, coherence_classical]

theorem compiled_frozen (encode : Signed α → β) (M J : Atoms β) (bases : List α)
    (theory : Theory (Signed α)) :
    Models J (ReductTheory M (compile encode bases theory)) ↔
      CoherentOn bases (Pull encode M) ∧
        Models (Pull encode J) (ReductTheory (Pull encode M) theory) := by
  rw [compile, frozen_theory_rename]
  simp only [ReductTheory, List.map_append, RuleFactorization.models_append]
  rw [← ReductTheory, coherence_frozen]

theorem compiled_stable (encode : Signed α → β) (injective : Function.Injective encode)
    (M : Atoms (Signed α)) (bases : List α) (theory : Theory (Signed α))
    (covered : Covered bases M) :
    Stable (Push encode M) (compile encode bases theory) ↔ CoherentStable M theory := by
  rw [compile, stable_push encode injective, coherence_stable_filter]
  rw [← coherent_iff_on_covered bases M covered]
  exact and_comm

theorem compiled_target_stable (encode : Signed α → β) (injective : Function.Injective encode)
    (M : Atoms β) (bases : List α) (theory : Theory (Signed α)) :
    Stable M (compile encode bases theory) ↔
      Push encode (Pull encode M) = M ∧
        Stable (Pull encode M) theory ∧ CoherentOn bases (Pull encode M) := by
  rw [compile, stable_target_iff encode injective, coherence_stable_filter]

/-- Even before reduct minimality is checked, an encoded contradictory pair
    present in the coherence registry is impossible. -/
theorem compiled_rejects_incoherent_candidate (encode : Signed α → β)
    (M : Atoms β) (bases : List α) (theory : Theory (Signed α)) (atom : α)
    (registered : atom ∈ bases) (positive : M (encode (.positive atom)))
    (negative : M (encode (.negative atom))) : ¬ Models M (compile encode bases theory) := by
  intro models
  exact ((compiled_classical encode M bases theory).mp models).1 atom registered
    ⟨positive, negative⟩

/-- Coherence needs a registry entry only when both polarities can coexist in
    the candidate. A carrier with just one polarity needs no invented opposite. -/
def PairCovered (bases : List α) (M : Atoms (Signed α)) : Prop :=
  ∀ atom, M (.positive atom) → M (.negative atom) → atom ∈ bases

theorem pair_covered_of_covered (bases : List α) (M : Atoms (Signed α))
    (covered : Covered bases M) : PairCovered bases M :=
  fun atom positive _ => covered (.positive atom) positive

/-- A complete possible-atom carrier can establish registry coverage for all
    enclosed candidates. Both enclosure and pair coverage remain premises. -/
theorem pair_coverage_from_upper (bases : List α) (M upper : Atoms (Signed α))
    (enclosed : Sub M upper) (complete : PairCovered bases upper) : PairCovered bases M :=
  fun atom positive negative => complete atom
    (enclosed (.positive atom) positive) (enclosed (.negative atom) negative)

theorem coherent_iff_on_pair_covered (bases : List α) (M : Atoms (Signed α))
    (covered : PairCovered bases M) : Coherent M ↔ CoherentOn bases M := by
  constructor
  · exact coherent_on_of_coherent bases M
  · intro coherent atom both
    exact coherent atom (covered atom both.1 both.2) both

theorem compiled_stable_of_pair_coverage (encode : Signed α → β)
    (injective : Function.Injective encode) (M : Atoms (Signed α)) (bases : List α)
    (theory : Theory (Signed α)) (covered : PairCovered bases M) :
    Stable (Push encode M) (compile encode bases theory) ↔ CoherentStable M theory := by
  rw [compile, stable_push encode injective, coherence_stable_filter,
    ← coherent_iff_on_pair_covered bases M covered]
  exact and_comm

/-- The complete target characterization also excludes unencoded atoms.
    Registry coverage is a separate explicit obligation, not inferred from an
    arbitrary partial grounding or from an unchecked candidate. -/
theorem compiled_target_coherent_stable (encode : Signed α → β)
    (injective : Function.Injective encode) (M : Atoms β) (bases : List α)
    (theory : Theory (Signed α)) (covered : PairCovered bases (Pull encode M)) :
    Stable M (compile encode bases theory) ↔
      Push encode (Pull encode M) = M ∧ CoherentStable (Pull encode M) theory := by
  rw [compiled_target_stable encode injective,
    ← coherent_iff_on_pair_covered bases (Pull encode M) covered]
  exact and_congr Iff.rfl and_comm

theorem compiled_push_classical (encode : Signed α → β) (injective : Function.Injective encode)
    (M : Atoms (Signed α)) (bases : List α) (theory : Theory (Signed α))
    (covered : PairCovered bases M) :
    Models (Push encode M) (compile encode bases theory) ↔ Coherent M ∧ Models M theory := by
  rw [compiled_classical, pull_push encode injective,
    ← coherent_iff_on_pair_covered bases M covered]

theorem compiled_push_frozen (encode : Signed α → β) (injective : Function.Injective encode)
    (M J : Atoms (Signed α)) (bases : List α) (theory : Theory (Signed α))
    (covered : PairCovered bases M) :
    Models (Push encode J) (ReductTheory (Push encode M) (compile encode bases theory)) ↔
      Coherent M ∧ Models J (ReductTheory M theory) := by
  rw [compiled_frozen, pull_push encode injective, pull_push encode injective,
    ← coherent_iff_on_pair_covered bases M covered]

/-- Strong negative atoms remain atomic under default negation. -/
theorem rename_default_negative (encode : Signed α → β) (atom : α) :
    rename encode (Neg (.atom (.negative atom))) = Neg (.atom (encode (.negative atom))) := rfl

theorem default_negative_frozen (encode : Signed α → β) (M J : Atoms β) (atom : α) :
    Satisfies J (Reduct M (rename encode (Neg (.atom (.negative atom))))) ↔
      ¬ M (encode (.negative atom)) := by
  rw [rename_default_negative, neg_atom_reduct]

/-- An optional choice over either signed identity has the same frozen guard;
    a strongly negative choice does not mean default-negating the positive atom. -/
theorem signed_choice_frozen (encode : Signed α → β) (M J : Atoms β) (atom : Signed α) :
    Satisfies J (Reduct M (rename encode (.disj (.atom atom) (Neg (.atom atom))))) ↔
      (M (encode atom) → J (encode atom)) := by
  exact choice_reduct_guard M J (encode atom)

/-- The existing whole bounded group is transported intact. In particular the
    constraint-only bounds and head/eligibility conjunctions are not changed. -/
theorem bounded_group_frozen (encode : Signed α → β) (injective : Function.Injective encode)
    (M J : Atoms (Signed α)) (body : Formula (Signed α)) (lower upper : Nat)
    (heads : List (Signed α)) (eligible : Signed α → Formula (Signed α)) :
    Satisfies (Push encode J) (Reduct (Push encode M)
      (rename encode (ChoiceIntervals.group body lower upper heads eligible))) ↔
      Satisfies J (Reduct M (ChoiceIntervals.group body lower upper heads eligible)) := by
  rw [frozen_rename, pull_push encode injective, pull_push encode injective]

/-- Coherence may be combined with any bounded group and unchanged surrounding
    signed theory, preserving exact stable-model identity through the encoding. -/
theorem bounded_group_stable_in_context (encode : Signed α → β)
    (injective : Function.Injective encode) (M : Atoms (Signed α)) (bases : List α)
    (body : Formula (Signed α)) (lower upper : Nat) (heads : List (Signed α))
    (eligible : Signed α → Formula (Signed α)) (context : Theory (Signed α))
    (covered : PairCovered bases M) :
    Stable (Push encode M) (compile encode bases
      (ChoiceIntervals.group body lower upper heads eligible :: context)) ↔
      CoherentStable M (ChoiceIntervals.group body lower upper heads eligible :: context) :=
  compiled_stable_of_pair_coverage encode injective M bases _ covered

/-- Absence of a positive atom does not establish its strong negation. -/
theorem strong_is_not_default (atom : α) :
    Satisfies (Empty : Atoms (Signed α)) (Neg (.atom (.positive atom))) ∧
      ¬ Satisfies (Empty : Atoms (Signed α)) (.atom (.negative atom)) := by
  simp [Ferraris.Neg, Satisfies, Empty]

/-- Both signed facts are impossible once their complete base is registered. -/
theorem conflicting_facts_are_unsatisfiable (encode : Signed α → β) (M : Atoms β)
    (bases : List α) (atom : α) (registered : atom ∈ bases) :
    ¬ Stable M (compile encode bases [.atom (.positive atom), .atom (.negative atom)]) := by
  intro stable
  obtain ⟨coherent, facts⟩ := (compiled_classical encode M bases _).mp stable.1
  exact coherent atom registered
    ⟨facts (.atom (.positive atom)) (by simp), facts (.atom (.negative atom)) (by simp)⟩

/-- An explicit strong-negative fact requires that distinct negative identity. -/
theorem negative_fact_stable (atom : α) :
    CoherentStable (fun value => value = Signed.negative atom) [.atom (.negative atom)] := by
  constructor
  · intro other both
    cases both.1
  · constructor
    · intro F member
      have same : F = Formula.atom (.negative atom) := List.mem_singleton.mp member
      subst F
      rfl
    · rintro ⟨J, ⟨_, strict⟩, models⟩
      apply strict
      intro value present
      subst value
      have selected := models (Reduct (fun value => value = Signed.negative atom)
        (.atom (.negative atom))) (by simp [ReductTheory])
      exact (atom_reduct _ _ _).mp selected |>.2

/-- The registry coverage premise is essential: without the coherence entry,
    the underlying unconstrained Ferraris theory accepts both signed facts. -/
theorem omitted_registry_admits_incoherence :
    Stable (Full : Atoms (Signed Unit))
      (compile id [] [.atom (.positive ()), .atom (.negative ())]) ∧
      ¬ Coherent (Full : Atoms (Signed Unit)) := by
  constructor
  · constructor
    · intro F member
      simp only [compile, coherenceTheory, List.map_nil, List.nil_append, renameTheory,
        List.map_cons, rename, id_eq, List.mem_cons, List.not_mem_nil, or_false] at member
      rcases member with rfl | rfl <;> trivial
    · rintro ⟨J, ⟨_, strict⟩, models⟩
      apply strict
      have positive : J (.positive ()) := by
        have selected := models (Reduct (Full : Atoms (Signed Unit)) (.atom (.positive ())))
          (by simp [ReductTheory, compile, coherenceTheory, renameTheory, rename])
        exact (atom_reduct _ _ _).mp selected |>.2
      have negative : J (.negative ()) := by
        have selected := models (Reduct (Full : Atoms (Signed Unit)) (.atom (.negative ())))
          (by simp [ReductTheory, compile, coherenceTheory, renameTheory, rename])
        exact (atom_reduct _ _ _).mp selected |>.2
      intro value _
      cases value with
      | positive atom => cases atom; exact positive
      | negative atom => cases atom; exact negative
  · intro coherent
    exact coherent () ⟨True.intro, True.intro⟩

/-- Optional choices may select neither sign, or either sign, while coherence
    rejects the fourth (both-polarities) candidate. Default negation does not
    force the negative identity when the positive identity is absent. -/
theorem independent_polarity_choices (M : Atoms (Signed Unit)) :
    Stable M (coherenceTheory [()] ++
      [.disj (.atom (.positive ())) (Neg (.atom (.positive ()))),
       .disj (.atom (.negative ())) (Neg (.atom (.negative ())))]) ↔ Coherent M := by
  classical
  have choices : Stable M
      [.disj (.atom (.positive ())) (Neg (.atom (.positive ()))),
       .disj (.atom (.negative ())) (Neg (.atom (.negative ())))] := by
    constructor
    · intro F member
      rcases List.mem_cons.mp member with rfl | member
      · exact Classical.em (M (.positive ()))
      · have same := List.mem_singleton.mp member
        subst F
        exact Classical.em (M (.negative ()))
    · rintro ⟨J, ⟨_, strict⟩, models⟩
      apply strict
      intro value present
      have selected : Satisfies J (Reduct M (.disj (.atom value) (Neg (.atom value)))) :=
        models _ (by cases value with
          | positive atom => cases atom; simp [ReductTheory]
          | negative atom => cases atom; simp [ReductTheory])
      exact (choice_reduct_guard M J value).mp selected present
  rw [coherence_stable_filter]
  have covered : PairCovered [()] M := by intro atom _ _; cases atom; simp
  rw [← coherent_iff_on_pair_covered [()] M covered]
  simp only [choices, true_and]

end Zetesis.StrongNegation
