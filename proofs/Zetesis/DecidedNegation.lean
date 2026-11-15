import Zetesis.CertainAtoms

/-!
# Decided negation

A grounder may know that an atom holds in no answer set before it emits a
formula that reads the atom: no formula it emits can derive it. This module
states when reading such an atom as false changes no answer set. With
`CertainAtoms`, which states when an atom may be read as true, it covers a
default negation whose atom the grounder has decided either way: `not a` is
the implication from `a` to falsum, so it becomes false where `a` is read as
true (`CertainAtoms.Fixed` replaces an atom at any occurrence) and true where
`a` is read as false.

`Headless C F` says no atom of `C` occurs in `F` outside every antecedent: an
occurrence in the antecedent of an implication, at any depth, is unrestricted,
and any other occurrence is forbidden. A rule's body is an antecedent and its
head is not, so a theory of rules is headless in the atoms no rule has in its
head. Removing the atoms of `C` from an interpretation leaves the frozen
reduct of a headless formula satisfied (`headless_reduct`), so an answer set of
a headless theory holds no atom of `C` (`absent_false`): otherwise the
interpretation without them would be a proper subset satisfying the reduct.

`Emptied C F G` says `G` is `F` with some occurrences of atoms of `C` replaced
by falsum. An interpretation that holds no atom of `C` satisfies the two
alike, and every interpretation satisfies their reducts frozen at it alike
(`emptied_satisfies`, `emptied_reduct`). Emptying keeps a formula headless
(`emptied_headless`). A headless theory and its emptying therefore have the
same answer sets (`absent_stable`).

These laws do not establish that the theory a grounder emits is headless in
the atoms outside its completed support, which rests on that support being a
fixed point of the rules; nor which occurrences the Rust emission replaces,
the classification of predicates, or the treatment of aggregates, which are
outside this formula language.
-/
namespace Zetesis.DecidedNegation

universe u
variable {α : Type u}
open Ferraris CertainAtoms

/-- No atom of `C` occurs in the formula outside every antecedent. -/
inductive Headless (C : Atoms α) : Formula α → Prop
  | atom {a : α} : ¬ C a → Headless C (.atom a)
  | bot : Headless C .bot
  | conj {F G : Formula α} : Headless C F → Headless C G → Headless C (.conj F G)
  | disj {F G : Formula α} : Headless C F → Headless C G → Headless C (.disj F G)
  | imp (F : Formula α) {G : Formula α} : Headless C G → Headless C (.imp F G)

/-- The interpretation `M` without the atoms of `C`. -/
def Without (M C : Atoms α) : Atoms α := fun a => M a ∧ ¬ C a

/-- An interpretation without some atoms is inside the interpretation. -/
theorem without_sub (M C : Atoms α) : Sub (Without M C) M := fun _ inside => inside.1

/-- A formula whose frozen reduct some interpretation satisfies is satisfied by
the frozen interpretation.

Proof. Otherwise the reduct is falsum (`reduct_unsatisfied`). -/
theorem reduct_holds {M J : Atoms α} {F : Formula α} (reduct : Satisfies J (Reduct M F)) :
    Satisfies M F := by
  classical
  by_cases holds : Satisfies M F
  · exact holds
  · exact (reduct_unsatisfied holds J reduct).elim

/-- Removing the atoms of `C` from the frozen interpretation leaves the frozen
reduct of a headless formula satisfied.

Proof. By induction on the formula. An atom outside `C` that the
interpretation holds stays; a formula the frozen interpretation does not
satisfy has falsum for its reduct, which nothing satisfies. A conjunction and
a disjunction combine their parts. For an implication the frozen interpretation
satisfies, suppose the smaller interpretation satisfies the antecedent's
reduct. Then the frozen interpretation satisfies the antecedent
(`reduct_holds`), hence the consequent and its reduct (`reduct_self`), and the
consequent is headless, so the smaller interpretation satisfies its reduct. -/
theorem headless_reduct {C M : Atoms α} {F : Formula α} (headless : Headless C F)
    (holds : Satisfies M (Reduct M F)) : Satisfies (Without M C) (Reduct M F) := by
  induction headless with
  | @atom a outside =>
    have inside : M a := reduct_holds holds
    rw [reduct_atom inside]
    exact ⟨inside, outside⟩
  | bot => exact False.elim holds
  | @conj F G _ _ left right =>
    have whole : Satisfies M (.conj F G) := reduct_holds holds
    rw [reduct_conj whole] at holds ⊢
    exact ⟨left holds.1, right holds.2⟩
  | @disj F G _ _ left right =>
    have whole : Satisfies M (.disj F G) := reduct_holds holds
    rw [reduct_disj whole] at holds ⊢
    exact holds.elim (fun first => Or.inl (left first)) (fun second => Or.inr (right second))
  | @imp F G _ consequent =>
    have whole : Satisfies M (.imp F G) := reduct_holds holds
    rw [reduct_imp whole]
    intro antecedent
    exact consequent ((reduct_self M G).mpr (whole (reduct_holds antecedent)))

/-- An answer set of a headless theory holds no atom of `C`.

Proof. Suppose it held one. The answer set without the atoms of `C` is a proper
subset, and it satisfies the frozen reduct of every formula of the theory
(`headless_reduct`), which no proper subset of an answer set does. -/
theorem absent_false {C : Atoms α} {T : Theory α}
    (headless : ∀ F, F ∈ T → Headless C F) {M : Atoms α} (stable : Stable M T)
    {a : α} (member : C a) : ¬ M a := by
  intro inside
  apply stable.2
  refine ⟨Without M C, ⟨without_sub M C, fun back => (back a inside).2 member⟩, ?_⟩
  intro G listed
  obtain ⟨F, original, rfl⟩ := List.mem_map.mp listed
  exact headless_reduct (headless F original) ((reduct_self M F).mpr (stable.1 F original))

/-- `G` is `F` with some occurrences of atoms of `C` replaced by falsum. -/
inductive Emptied (C : Atoms α) : Formula α → Formula α → Prop
  | same (F : Formula α) : Emptied C F F
  | atom {a : α} : C a → Emptied C (.atom a) .bot
  | conj {F F' G G' : Formula α} :
      Emptied C F F' → Emptied C G G' → Emptied C (.conj F G) (.conj F' G')
  | disj {F F' G G' : Formula α} :
      Emptied C F F' → Emptied C G G' → Emptied C (.disj F G) (.disj F' G')
  | imp {F F' G G' : Formula α} :
      Emptied C F F' → Emptied C G G' → Emptied C (.imp F G) (.imp F' G')

/-- An interpretation that holds no atom of `C` satisfies a formula and its
emptying alike.

Proof. By induction on the emptying. A replaced atom is false in the
interpretation, as falsum is; each connective combines its parts' truth. -/
theorem emptied_satisfies {C M : Atoms α} (outside : ∀ a, C a → ¬ M a) {F G : Formula α}
    (emptied : Emptied C F G) : Satisfies M G ↔ Satisfies M F := by
  induction emptied with
  | same F => exact Iff.rfl
  | atom member => exact ⟨fun impossible => impossible.elim, fun inside => outside _ member inside⟩
  | conj _ _ left right => simp only [Satisfies, left, right]
  | disj _ _ left right => simp only [Satisfies, left, right]
  | imp _ _ left right => simp only [Satisfies, left, right]

/-- When the frozen interpretation holds no atom of `C`, every interpretation
satisfies the frozen reducts of a formula and of its emptying alike.

Proof. By induction on the emptying. The reduct of a replaced atom is falsum,
because the frozen interpretation does not hold it, and so is the reduct of
falsum. A connective and its emptying are satisfied alike by the frozen
interpretation (`emptied_satisfies`), so their reducts are both falsum or both
the connective of the parts' reducts. -/
theorem emptied_reduct {C M : Atoms α} (outside : ∀ a, C a → ¬ M a) (J : Atoms α)
    {F G : Formula α} (emptied : Emptied C F G) :
    Satisfies J (Reduct M G) ↔ Satisfies J (Reduct M F) := by
  induction emptied with
  | same F => exact Iff.rfl
  | @atom a member =>
    exact ⟨fun h => False.elim h,
      fun h => (reduct_unsatisfied (F := .atom a) (outside a member) J h).elim⟩
  | @conj F F' G G' hF hG left right =>
    have alike : Satisfies M (.conj F' G') ↔ Satisfies M (.conj F G) :=
      emptied_satisfies outside (Emptied.conj hF hG)
    by_cases whole : Satisfies M (.conj F G)
    · rw [reduct_conj whole, reduct_conj (alike.mpr whole)]
      simp only [Satisfies, left, right]
    · exact ⟨fun h => (reduct_unsatisfied (fun g => whole (alike.mp g)) J h).elim,
        fun h => (reduct_unsatisfied whole J h).elim⟩
  | @disj F F' G G' hF hG left right =>
    have alike : Satisfies M (.disj F' G') ↔ Satisfies M (.disj F G) :=
      emptied_satisfies outside (Emptied.disj hF hG)
    by_cases whole : Satisfies M (.disj F G)
    · rw [reduct_disj whole, reduct_disj (alike.mpr whole)]
      simp only [Satisfies, left, right]
    · exact ⟨fun h => (reduct_unsatisfied (fun g => whole (alike.mp g)) J h).elim,
        fun h => (reduct_unsatisfied whole J h).elim⟩
  | @imp F F' G G' hF hG left right =>
    have alike : Satisfies M (.imp F' G') ↔ Satisfies M (.imp F G) :=
      emptied_satisfies outside (Emptied.imp hF hG)
    by_cases whole : Satisfies M (.imp F G)
    · rw [reduct_imp whole, reduct_imp (alike.mpr whole)]
      simp only [Satisfies, left, right]
    · exact ⟨fun h => (reduct_unsatisfied (fun g => whole (alike.mp g)) J h).elim,
        fun h => (reduct_unsatisfied whole J h).elim⟩

/-- Emptying keeps a formula headless.

Proof. By induction on the emptying; falsum is headless, and an antecedent is
unrestricted. -/
theorem emptied_headless {C : Atoms α} {F G : Formula α} (emptied : Emptied C F G)
    (headless : Headless C F) : Headless C G := by
  induction emptied with
  | same F => exact headless
  | atom _ => exact Headless.bot
  | conj _ _ left right =>
    cases headless with
    | conj first second => exact Headless.conj (left first) (right second)
  | disj _ _ left right =>
    cases headless with
    | disj first second => exact Headless.disj (left first) (right second)
  | imp _ _ _ right =>
    cases headless with
    | imp _ consequent => exact Headless.imp _ (right consequent)

/-- A theory emptied formula by formula. -/
inductive EmptiedTheory (C : Atoms α) : Theory α → Theory α → Prop
  | nil : EmptiedTheory C [] []
  | cons {F G : Formula α} {T T' : Theory α} :
      Emptied C F G → EmptiedTheory C T T' → EmptiedTheory C (F :: T) (G :: T')

/-- An interpretation that holds no atom of `C` models a theory and its
emptying alike. -/
theorem emptiedTheory_models {C M : Atoms α} (outside : ∀ a, C a → ¬ M a) {T T' : Theory α}
    (emptied : EmptiedTheory C T T') : Models M T' ↔ Models M T := by
  induction emptied with
  | nil => exact Iff.rfl
  | cons first _ rest =>
    simp only [models_cons, emptied_satisfies outside first, rest]

/-- When the frozen interpretation holds no atom of `C`, every interpretation
models the frozen reducts of a theory and of its emptying alike. -/
theorem emptiedTheory_reduct {C M : Atoms α} (outside : ∀ a, C a → ¬ M a) (J : Atoms α)
    {T T' : Theory α} (emptied : EmptiedTheory C T T') :
    Models J (ReductTheory M T') ↔ Models J (ReductTheory M T) := by
  induction emptied with
  | nil => exact Iff.rfl
  | cons first _ rest =>
    simp only [ReductTheory, List.map_cons, models_cons] at rest ⊢
    simp only [emptied_reduct outside J first, rest]

/-- The emptying of a headless theory is headless. -/
theorem emptiedTheory_headless {C : Atoms α} {T T' : Theory α}
    (emptied : EmptiedTheory C T T') (headless : ∀ F, F ∈ T → Headless C F) :
    ∀ G, G ∈ T' → Headless C G := by
  induction emptied with
  | nil => intro G member; exact (List.not_mem_nil member).elim
  | @cons F G T T' first _ rest =>
    intro H member
    rcases List.mem_cons.mp member with rfl | member
    · exact emptied_headless first (headless F (List.mem_cons.mpr (Or.inl rfl)))
    · exact rest (fun K listed => headless K (List.mem_cons.mpr (Or.inr listed))) H member

/-- A headless theory and its emptying have the same answer sets.

Proof. Both theories are headless (`emptiedTheory_headless`), so an answer set
of either holds no atom of `C` (`absent_false`). It then models the other
theory (`emptiedTheory_models`), and a proper subset modelling the other
theory's frozen reduct would model the first theory's
(`emptiedTheory_reduct`), which no proper subset of an answer set does. -/
theorem absent_stable {C : Atoms α} {T T' : Theory α} (emptied : EmptiedTheory C T T')
    (headless : ∀ F, F ∈ T → Headless C F) (M : Atoms α) :
    Stable M T' ↔ Stable M T := by
  have headless' := emptiedTheory_headless emptied headless
  constructor
  · intro stable
    have outside : ∀ a, C a → ¬ M a := fun _ member => absent_false headless' stable member
    refine ⟨(emptiedTheory_models outside emptied).mp stable.1, ?_⟩
    rintro ⟨J, proper, reduct⟩
    exact stable.2 ⟨J, proper, (emptiedTheory_reduct outside J emptied).mpr reduct⟩
  · intro stable
    have outside : ∀ a, C a → ¬ M a := fun _ member => absent_false headless stable member
    refine ⟨(emptiedTheory_models outside emptied).mpr stable.1, ?_⟩
    rintro ⟨J, proper, reduct⟩
    exact stable.2 ⟨J, proper, (emptiedTheory_reduct outside J emptied).mp reduct⟩

end Zetesis.DecidedNegation
