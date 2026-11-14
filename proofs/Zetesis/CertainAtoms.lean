import Zetesis.Ferraris

/-!
# Certain atoms

A grounder may know that an atom holds in every answer set before it emits a
formula that reads the atom. A fact is the plain case. An atom derived from
facts by rules with one head and a body of such atoms is another: each of its
derivations ends in facts. This module states when reading such an atom as true
changes no answer set.

`Forces T C` says the theory `T` forces the atoms of `C`: every model of `T`
holds them, and so does every model, inside that model, of its frozen reduct.
`Fixed C F G` says `G` is `F` with some occurrences of atoms of `C` replaced by
the canonical true formula, falsum implying falsum. When an interpretation
holds `C`, the two formulas are satisfied alike, and when a second one inside
it holds `C` too, their frozen reducts are satisfied alike. A theory and its
formula-by-formula fixing that both force `C` therefore have the same answer
sets (`fixed_stable`).

The hypothesis that the fixed theory still forces `C` is not automatic. Fixing
may replace the very occurrence that forced an atom. `derived_forced` gives the
case a grounder uses: an atom with a derivation from the theory's atom formulas
through implications whose antecedent is a conjunction of atoms already derived
is forced. An emitted theory that keeps every atom formula, and for every such
implication one with the same consequent whose antecedent is a conjunction of
some of the original's atoms, keeps every derivation (`derived_kept`). So a
theory that derives `C` and such a fixing of it have the same answer sets
(`derived_stable`).

A true conjunct may also be left out of a conjunction (`top_conj`), which is
how an emitter drops it, and a true disjunct settles its disjunction
(`top_disj`).

These laws do not establish that a grounder's supported atoms of a predicate
are the derived ones, the classification of predicates, the treatment of
aggregates, which are outside this formula language, or the Rust emission.
-/
namespace Zetesis.CertainAtoms

universe u
variable {α : Type u}
open Ferraris

/-- The canonical true formula: falsum implying falsum. -/
def Top : Formula α := .imp .bot .bot

/-- `Top` is satisfied by every interpretation. -/
theorem satisfies_top (M : Atoms α) : Satisfies M (Top : Formula α) :=
  fun impossible => impossible

/-- The frozen reduct of `Top` is `Top`, so every interpretation satisfies it. -/
theorem reduct_top (M J : Atoms α) : Satisfies J (Reduct M (Top : Formula α)) := by
  classical
  have holds : Satisfies M (Formula.imp (.bot : Formula α) .bot) := satisfies_top M
  simp only [Top, Reduct, if_pos holds]
  exact fun impossible => impossible

/-- The frozen reduct of a conjunction the frozen interpretation satisfies. -/
theorem reduct_conj {M : Atoms α} {F G : Formula α} (whole : Satisfies M (.conj F G)) :
    Reduct M (.conj F G) = .conj (Reduct M F) (Reduct M G) := by
  classical
  simp only [Reduct, if_pos whole]

/-- The frozen reduct of a disjunction the frozen interpretation satisfies. -/
theorem reduct_disj {M : Atoms α} {F G : Formula α} (whole : Satisfies M (.disj F G)) :
    Reduct M (.disj F G) = .disj (Reduct M F) (Reduct M G) := by
  classical
  simp only [Reduct, if_pos whole]

/-- The frozen reduct of an implication the frozen interpretation satisfies. -/
theorem reduct_imp {M : Atoms α} {F G : Formula α} (whole : Satisfies M (.imp F G)) :
    Reduct M (.imp F G) = .imp (Reduct M F) (Reduct M G) := by
  classical
  simp only [Reduct, if_pos whole]

/-- The frozen reduct of an atom the frozen interpretation holds. -/
theorem reduct_atom {M : Atoms α} {a : α} (inside : M a) : Reduct M (.atom a) = .atom a := by
  classical
  simp only [Reduct, if_pos inside]

/-- No interpretation satisfies the frozen reduct of a formula the frozen
interpretation does not satisfy: every such reduct is falsum. -/
theorem reduct_unsatisfied {M : Atoms α} {F : Formula α} (out : ¬ Satisfies M F)
    (J : Atoms α) : ¬ Satisfies J (Reduct M F) := by
  classical
  cases F with
  | atom a =>
    have outside : ¬ M a := out
    simp only [Reduct, if_neg outside]
    exact id
  | bot => exact id
  | conj F G =>
    simp only [Reduct, if_neg out]
    exact id
  | disj F G =>
    simp only [Reduct, if_neg out]
    exact id
  | imp F G =>
    simp only [Reduct, if_neg out]
    exact id

/-- `G` is `F` with some occurrences of atoms of `C` replaced by `Top`. -/
inductive Fixed (C : Atoms α) : Formula α → Formula α → Prop
  | same (F : Formula α) : Fixed C F F
  | atom {a : α} : C a → Fixed C (.atom a) Top
  | conj {F F' G G' : Formula α} :
      Fixed C F F' → Fixed C G G' → Fixed C (.conj F G) (.conj F' G')
  | disj {F F' G G' : Formula α} :
      Fixed C F F' → Fixed C G G' → Fixed C (.disj F G) (.disj F' G')
  | imp {F F' G G' : Formula α} :
      Fixed C F F' → Fixed C G G' → Fixed C (.imp F G) (.imp F' G')

/-- An interpretation that holds `C` satisfies a formula and its fixing alike.

Proof. By induction on the fixing. A replaced atom is true in the
interpretation and `Top` is true everywhere; each connective combines its
parts' truth. -/
theorem fixed_satisfies {C M : Atoms α} (holds : Sub C M) {F G : Formula α}
    (fixed : Fixed C F G) : Satisfies M G ↔ Satisfies M F := by
  induction fixed with
  | same F => exact Iff.rfl
  | atom member => exact ⟨fun _ => holds _ member, fun _ => satisfies_top M⟩
  | conj _ _ left right => simp only [Satisfies, left, right]
  | disj _ _ left right => simp only [Satisfies, left, right]
  | imp _ _ left right => simp only [Satisfies, left, right]

/-- When the frozen interpretation and the tested one both hold `C`, the tested
one satisfies the frozen reducts of a formula and of its fixing alike.

Proof. By induction on the fixing. A replaced atom is true in the frozen
interpretation, so its reduct is the atom, which the tested interpretation
holds; the reduct of `Top` is satisfied everywhere. A connective and its fixing
are satisfied alike by the frozen interpretation (`fixed_satisfies`), so their
reducts are both falsum or both the connective of the parts' reducts. -/
theorem fixed_reduct {C M J : Atoms α} (frozen : Sub C M) (tested : Sub C J)
    {F G : Formula α} (fixed : Fixed C F G) :
    Satisfies J (Reduct M G) ↔ Satisfies J (Reduct M F) := by
  induction fixed with
  | same F => exact Iff.rfl
  | @atom a member =>
    rw [reduct_atom (frozen a member)]
    exact ⟨fun _ => tested a member, fun _ => reduct_top M J⟩
  | @conj F F' G G' hF hG left right =>
    have alike : Satisfies M (.conj F' G') ↔ Satisfies M (.conj F G) :=
      fixed_satisfies frozen (Fixed.conj hF hG)
    by_cases whole : Satisfies M (.conj F G)
    · rw [reduct_conj whole, reduct_conj (alike.mpr whole)]
      simp only [Satisfies, left, right]
    · exact ⟨fun h => (reduct_unsatisfied (fun g => whole (alike.mp g)) J h).elim,
        fun h => (reduct_unsatisfied whole J h).elim⟩
  | @disj F F' G G' hF hG left right =>
    have alike : Satisfies M (.disj F' G') ↔ Satisfies M (.disj F G) :=
      fixed_satisfies frozen (Fixed.disj hF hG)
    by_cases whole : Satisfies M (.disj F G)
    · rw [reduct_disj whole, reduct_disj (alike.mpr whole)]
      simp only [Satisfies, left, right]
    · exact ⟨fun h => (reduct_unsatisfied (fun g => whole (alike.mp g)) J h).elim,
        fun h => (reduct_unsatisfied whole J h).elim⟩
  | @imp F F' G G' hF hG left right =>
    have alike : Satisfies M (.imp F' G') ↔ Satisfies M (.imp F G) :=
      fixed_satisfies frozen (Fixed.imp hF hG)
    by_cases whole : Satisfies M (.imp F G)
    · rw [reduct_imp whole, reduct_imp (alike.mpr whole)]
      simp only [Satisfies, left, right]
    · exact ⟨fun h => (reduct_unsatisfied (fun g => whole (alike.mp g)) J h).elim,
        fun h => (reduct_unsatisfied whole J h).elim⟩

/-- A true conjunct may be left out: the frozen reduct of `Top ∧ F` is
satisfied exactly where that of `F` is.

Proof. The frozen interpretation satisfies the conjunction exactly when it
satisfies `F`. If it does, the reduct is the conjunction of the reducts, the
first of which is satisfied everywhere. If not, neither reduct is satisfied by
anything (`reduct_unsatisfied`). -/
theorem top_conj (M J : Atoms α) (F : Formula α) :
    Satisfies J (Reduct M (.conj Top F)) ↔ Satisfies J (Reduct M F) := by
  by_cases whole : Satisfies M F
  · rw [reduct_conj (show Satisfies M (.conj Top F) from ⟨satisfies_top M, whole⟩)]
    exact ⟨fun h => h.2, fun h => ⟨reduct_top M J, h⟩⟩
  · exact ⟨fun h => (reduct_unsatisfied (fun g : Satisfies M (.conj Top F) => whole g.2) J h).elim,
      fun h => (reduct_unsatisfied whole J h).elim⟩

/-- A true disjunct settles a disjunction: the frozen interpretation satisfies
`Top ∨ F`, and every interpretation satisfies its frozen reduct.

Proof. `Top` is satisfied everywhere, so the disjunction is, and its reduct is
the disjunction of the reducts, the first of which is satisfied everywhere. -/
theorem top_disj (M J : Atoms α) (F : Formula α) :
    Satisfies M (.disj Top F) ∧ Satisfies J (Reduct M (.disj Top F)) := by
  have whole : Satisfies M (.disj Top F) := Or.inl (satisfies_top M)
  refine ⟨whole, ?_⟩
  rw [reduct_disj whole]
  exact Or.inl (reduct_top M J)

/-- A theory fixed formula by formula. -/
inductive FixedTheory (C : Atoms α) : Theory α → Theory α → Prop
  | nil : FixedTheory C [] []
  | cons {F G : Formula α} {T T' : Theory α} :
      Fixed C F G → FixedTheory C T T' → FixedTheory C (F :: T) (G :: T')

/-- A theory models its head and its tail. -/
theorem models_cons (M : Atoms α) (F : Formula α) (T : Theory α) :
    Models M (F :: T) ↔ Satisfies M F ∧ Models M T := by
  constructor
  · intro model
    exact ⟨model F (List.mem_cons.mpr (Or.inl rfl)),
      fun G member => model G (List.mem_cons.mpr (Or.inr member))⟩
  · rintro ⟨first, rest⟩ G member
    rcases List.mem_cons.mp member with rfl | member
    · exact first
    · exact rest G member

/-- An interpretation that holds `C` models a theory and its fixing alike. -/
theorem fixedTheory_models {C M : Atoms α} (holds : Sub C M) {T T' : Theory α}
    (fixed : FixedTheory C T T') : Models M T' ↔ Models M T := by
  induction fixed with
  | nil => exact Iff.rfl
  | cons first _ rest =>
    simp only [models_cons, fixed_satisfies holds first, rest]

/-- When the frozen interpretation and the tested one both hold `C`, the tested
one models the frozen reducts of a theory and of its fixing alike. -/
theorem fixedTheory_reduct {C M J : Atoms α} (frozen : Sub C M) (tested : Sub C J)
    {T T' : Theory α} (fixed : FixedTheory C T T') :
    Models J (ReductTheory M T') ↔ Models J (ReductTheory M T) := by
  induction fixed with
  | nil => exact Iff.rfl
  | cons first _ rest =>
    simp only [ReductTheory, List.map_cons, models_cons] at rest ⊢
    simp only [fixed_reduct frozen tested first, rest]

/-- A theory forces a set of atoms when every model of the frozen reduct at
one of its models, inside that model, holds the set. The model is such a model
of its own reduct, so it holds the set too (`forces_model`). -/
def Forces (T : Theory α) (C : Atoms α) : Prop :=
  ∀ M J, Sub J M → Models M T → Models J (ReductTheory M T) → Sub C J

/-- A model of a theory holds what the theory forces. -/
theorem forces_model {T : Theory α} {C : Atoms α} (forces : Forces T C)
    {M : Atoms α} (model : Models M T) : Sub C M :=
  forces M M (sub_refl M) model ((models_reduct_self M T).mpr model)

/-- A theory and its fixing have the same answer sets when both force the
fixed atoms.

Proof. Take an answer set of either theory. It holds `C`, so it models the
other theory (`fixedTheory_models`). A proper subset modelling the other
theory's frozen reduct would hold `C`, being forced there, and so would model
the first theory's frozen reduct (`fixedTheory_reduct`), which no proper subset
of an answer set does. -/
theorem fixed_stable {C : Atoms α} {T T' : Theory α} (fixed : FixedTheory C T T')
    (original : Forces T C) (emitted : Forces T' C) (M : Atoms α) :
    Stable M T' ↔ Stable M T := by
  constructor
  · rintro ⟨model, minimal⟩
    have holds : Sub C M := forces_model emitted model
    have model' : Models M T := (fixedTheory_models holds fixed).mp model
    refine ⟨model', ?_⟩
    rintro ⟨J, proper, reduct⟩
    have inside : Sub C J := original M J proper.1 model' reduct
    exact minimal ⟨J, proper, (fixedTheory_reduct holds inside fixed).mpr reduct⟩
  · rintro ⟨model, minimal⟩
    have holds : Sub C M := forces_model original model
    have model' : Models M T' := (fixedTheory_models holds fixed).mpr model
    refine ⟨model', ?_⟩
    rintro ⟨J, proper, reduct⟩
    have inside : Sub C J := emitted M J proper.1 model' reduct
    exact minimal ⟨J, proper, (fixedTheory_reduct holds inside fixed).mp reduct⟩

/-- A conjunction of atoms, with `Top` for the empty one. -/
inductive Positive : Formula α → Prop
  | top : Positive Top
  | atom (a : α) : Positive (.atom a)
  | conj {F G : Formula α} : Positive F → Positive G → Positive (.conj F G)

/-- A conjunction of atoms the frozen and the tested interpretation both hold
has a frozen reduct the tested one satisfies.

Proof. By induction. The reduct of `Top` is satisfied everywhere. An atom the
frozen interpretation holds is its own reduct. A conjunction of satisfied parts
is satisfied by the frozen interpretation, so its reduct is the conjunction of
the parts' reducts. -/
theorem positive_reduct {M J : Atoms α} {B : Formula α} (positive : Positive B)
    (frozen : ∀ a, Mentions B a → M a) (tested : ∀ a, Mentions B a → J a) :
    Satisfies M B ∧ Satisfies J (Reduct M B) := by
  induction positive with
  | top => exact ⟨satisfies_top M, reduct_top M J⟩
  | atom a =>
    have inside : M a := frozen a rfl
    refine ⟨inside, ?_⟩
    rw [reduct_atom inside]
    exact tested a rfl
  | @conj F G _ _ left right =>
    have first := left (fun a mentioned => frozen a (Or.inl mentioned))
      (fun a mentioned => tested a (Or.inl mentioned))
    have second := right (fun a mentioned => frozen a (Or.inr mentioned))
      (fun a mentioned => tested a (Or.inr mentioned))
    have whole : Satisfies M (.conj F G) := ⟨first.1, second.1⟩
    refine ⟨whole, ?_⟩
    rw [reduct_conj whole]
    exact ⟨first.2, second.2⟩

/-- The atoms a theory derives: an atom formula of the theory, or the
consequent of one of its implications whose antecedent is a conjunction of
derived atoms. -/
inductive Derived (T : Theory α) : α → Prop
  | fact {a : α} : Formula.atom a ∈ T → Derived T a
  | rule {B : Formula α} {h : α} : Formula.imp B (.atom h) ∈ T → Positive B →
      (∀ a, Mentions B a → Derived T a) → Derived T h

/-- A theory forces the atoms it derives.

Proof. By induction on the derivation, for a model `M` of the theory and a
model `J` of its frozen reduct. A fact is an atom formula, which `M` satisfies,
so its reduct is the atom and `J` holds it. For a rule, `M` and `J` hold the
antecedent's atoms by induction, `M` as a model of its own reduct. So `M`
satisfies the antecedent, hence the implication's consequent, and the frozen
implication keeps both sides; `J` satisfies the frozen antecedent
(`positive_reduct`) and therefore holds the consequent. -/
theorem derived_forced (T : Theory α) : Forces T (Derived T) := by
  intro M J _ model reduct a derived
  have both : M a ∧ J a := by
    induction derived with
    | @fact a member =>
      have inside : M a := model _ member
      have kept := reduct _ (List.mem_map.mpr ⟨_, member, rfl⟩)
      rw [reduct_atom inside] at kept
      exact ⟨inside, kept⟩
    | @rule B h member positive _ premises =>
      have body := positive_reduct positive (fun a mentioned => (premises a mentioned).1)
        (fun a mentioned => (premises a mentioned).2)
      have whole : Satisfies M (.imp B (.atom h)) := model _ member
      have inside : M h := whole body.1
      have kept := reduct _ (List.mem_map.mpr ⟨_, member, rfl⟩)
      rw [reduct_imp whole, reduct_atom inside] at kept
      exact ⟨inside, kept body.2⟩
  exact both.2

/-- An emitted theory keeps the derivations of the original when it keeps every
atom formula, and for every implication with an atomic consequent and a
conjunction of atoms as antecedent keeps one with that consequent whose
antecedent is a conjunction of atoms the original antecedent mentions.

Proof. By induction on the derivation. An atom formula is kept. For a rule, the
kept implication's antecedent mentions only atoms of the original antecedent,
each of which the emitted theory derives by induction. -/
theorem derived_kept {T T' : Theory α}
    (facts : ∀ a, Formula.atom a ∈ T → Formula.atom a ∈ T')
    (rules : ∀ B h, Formula.imp B (.atom h) ∈ T → Positive B →
      ∃ B', Formula.imp B' (.atom h) ∈ T' ∧ Positive B' ∧ ∀ a, Mentions B' a → Mentions B a)
    {a : α} (derived : Derived T a) : Derived T' a := by
  induction derived with
  | fact member => exact .fact (facts _ member)
  | @rule B h member positive _ premises =>
    obtain ⟨B', member', positive', inside⟩ := rules B h member positive
    exact .rule member' positive' (fun a mentioned => premises a (inside a mentioned))

/-- A theory that derives the fixed atoms and a fixing of it that keeps its
derivations have the same answer sets.

Proof. Each theory forces the atoms it derives (`derived_forced`), the emitted
one derives what the original does (`derived_kept`), and `fixed_stable`
applies. -/
theorem derived_stable {C : Atoms α} {T T' : Theory α} (fixed : FixedTheory C T T')
    (certain : ∀ a, C a → Derived T a)
    (facts : ∀ a, Formula.atom a ∈ T → Formula.atom a ∈ T')
    (rules : ∀ B h, Formula.imp B (.atom h) ∈ T → Positive B →
      ∃ B', Formula.imp B' (.atom h) ∈ T' ∧ Positive B' ∧ ∀ a, Mentions B' a → Mentions B a)
    (M : Atoms α) : Stable M T' ↔ Stable M T := by
  refine fixed_stable fixed ?_ ?_ M
  · intro M J inside model reduct a member
    exact derived_forced T M J inside model reduct a (certain a member)
  · intro M J inside model reduct a member
    exact derived_forced T' M J inside model reduct a
      (derived_kept facts rules (certain a member))

end Zetesis.CertainAtoms
