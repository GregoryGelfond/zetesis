import Zetesis.Ferraris
import Zetesis.Search
import Zetesis.DisjunctiveSupport

/-!
# Regions narrowed by the readings of a formula theory

A region of candidates over a formula theory is a cube: the atoms every
seed holds and the atoms some seed may hold. A formula has two readings
under a region, both decided by one pass over its structure: it is *sure*
when every seed of the region satisfies it, and *impossible* when no seed
does; an atom held is sure, an atom cut is impossible, and the connectives
combine the readings as the closure route's definite and possible gates do.
The readings are sound by induction on the formula.

Three narrowing rules follow, each over the acceptance `Ferraris.Stable`. A
root impossible under the region refutes it: no seed is a classical model.
A rule-shaped root whose body is sure forces its atomic head into every
stable model of the region, and a constraint whose body is sure refutes the
region; both are instances of the first rule read through the implication.
`Known` closes such consequences in both directions over the whole theory,
which is what unit propagation over a clause form of the theory decides,
and `known_sound` says every classical model inside the region agrees. In
the producer fragment of `DisjunctiveSupport`, an atom none of whose
producers can support it under the region, because each has an impossible
body or another head held, is cut, and an atom held with one producer
left unblocked has that producer's body: `answer_set_supported` says every
atom of a stable model has a supporting producer.

Each rule supplies a node of the coverage tree of `Search.lean` with the
stable models as the valid seeds, so the leaves of a tree built from them
are exactly the stable models of the root region once each leaf is checked
by the reduct. The Rust readings must agree with `read` on the admitted DAG
and charge one unit per node visited; that agreement and the extraction of
producers from roots are Rust obligations.
-/

namespace Zetesis.FormulaBounds

open Ferraris DisjunctiveSupport

universe u

variable {α : Type u}

/-- The two readings of a formula under a region: sure, and impossible. -/
def read (c : Cube α) : Formula α → Prop × Prop
  | .atom a => (c.lower a, ¬ c.upper a)
  | .bot => (False, True)
  | .conj F G => ((read c F).1 ∧ (read c G).1, (read c F).2 ∨ (read c G).2)
  | .disj F G => ((read c F).1 ∨ (read c G).1, (read c F).2 ∧ (read c G).2)
  | .imp F G => ((read c F).2 ∨ (read c G).1, (read c F).1 ∧ (read c G).2)

/-- Every seed of the region satisfies the formula. -/
abbrev Sure (c : Cube α) (F : Formula α) : Prop := (read c F).1

/-- No seed of the region satisfies the formula. -/
abbrev Never (c : Cube α) (F : Formula α) : Prop := (read c F).2

/-- The readings are sound: a sure formula holds in every seed of the
region and an impossible one in none. Induction on the formula. -/
theorem read_sound (c : Cube α) {z : Atoms α} (hz : c.Contains z) :
    ∀ F : Formula α, (Sure c F → Satisfies z F) ∧ (Never c F → ¬ Satisfies z F) := by
  intro F
  induction F with
  | atom a =>
    exact ⟨fun h => hz.1 a h, fun h hs => h (hz.2 a hs)⟩
  | bot =>
    exact ⟨fun h => h.elim, fun _ hs => hs⟩
  | conj F G hF hG =>
    refine ⟨fun h => ⟨hF.1 h.1, hG.1 h.2⟩, fun h hs => ?_⟩
    rcases h with n | n
    · exact hF.2 n hs.1
    · exact hG.2 n hs.2
  | disj F G hF hG =>
    refine ⟨fun h => ?_, fun h hs => ?_⟩
    · rcases h with s | s
      · exact Or.inl (hF.1 s)
      · exact Or.inr (hG.1 s)
    · rcases hs with s | s
      · exact hF.2 h.1 s
      · exact hG.2 h.2 s
  | imp F G hF hG =>
    refine ⟨fun h hs => ?_, fun h hs => ?_⟩
    · rcases h with n | s
      · exact (hF.2 n hs).elim
      · exact hG.1 s
    · exact hG.2 h.2 (hs (hF.1 h.1))

theorem sure_sound (c : Cube α) {z : Atoms α} (hz : c.Contains z) {F : Formula α}
    (h : Sure c F) : Satisfies z F :=
  (read_sound (z := z) c hz F).1 h

theorem never_sound (c : Cube α) {z : Atoms α} (hz : c.Contains z) {F : Formula α}
    (h : Never c F) : ¬ Satisfies z F :=
  (read_sound (z := z) c hz F).2 h

/-- Under a decided region, one whose two bounds agree, every formula is
sure or impossible: the readings are the truth of the formula in the
region's one seed. Induction on the formula, classically at the atoms. -/
theorem decided_reads (c : Cube α) (hc : ∀ a, c.lower a ↔ c.upper a) :
    ∀ F : Formula α, Sure c F ∨ Never c F := by
  intro F
  induction F with
  | atom a =>
    by_cases h : c.lower a
    · exact Or.inl h
    · exact Or.inr (fun u => h ((hc a).2 u))
  | bot => exact Or.inr trivial
  | conj F G hF hG =>
    rcases hF with sF | nF
    · rcases hG with sG | nG
      · exact Or.inl ⟨sF, sG⟩
      · exact Or.inr (Or.inr nG)
    · exact Or.inr (Or.inl nF)
  | disj F G hF hG =>
    rcases hF with sF | nF
    · exact Or.inl (Or.inl sF)
    · rcases hG with sG | nG
      · exact Or.inl (Or.inr sG)
      · exact Or.inr ⟨nF, nG⟩
  | imp F G hF hG =>
    rcases hF with sF | nF
    · rcases hG with sG | nG
      · exact Or.inl (Or.inr sG)
      · exact Or.inr ⟨sF, nG⟩
    · exact Or.inl (Or.inl nF)

/-- A decided region no root reads as impossible is a classical model of
the theory: its one seed, the lower bound, satisfies every root. This is
why a leaf of the region tree is proposed to the reduct without a classical
check. -/
theorem decided_leaf_models (T : Theory α) (c : Cube α)
    (hc : ∀ a, c.lower a ↔ c.upper a) (h : ∀ F ∈ T, ¬ Never c F) :
    Models c.lower T := by
  intro F hF
  have contains : c.Contains c.lower := ⟨fun _ l => l, fun a l => (hc a).1 l⟩
  rcases decided_reads c hc F with s | n
  · exact sure_sound c contains s
  · exact (h F hF n).elim

/-- A root impossible under the region refutes it. -/
theorem never_root_refutes (T : Theory α) (c : Cube α) {F : Formula α}
    (hF : F ∈ T) (h : Never c F) : ∀ z, c.Contains z → ¬ Stable z T :=
  fun z hz hs => never_sound (z := z) c hz h (hs.1 F hF)

/-- A rule whose body is sure forces its atomic head into every stable model
of the region. -/
theorem sure_body_forces (T : Theory α) (c : Cube α) {B : Formula α} {a : α}
    (hF : Formula.imp B (.atom a) ∈ T) (h : Sure c B) :
    ∀ z, c.Contains z → Stable z T → z a :=
  fun z hz hs => hs.1 _ hF (sure_sound (z := z) c hz h)

/-- A constraint whose body is sure refutes the region. -/
theorem sure_body_refutes (T : Theory α) (c : Cube α) {B : Formula α}
    (hF : Formula.imp B .bot ∈ T) (h : Sure c B) :
    ∀ z, c.Contains z → ¬ Stable z T :=
  never_root_refutes T c hF ⟨h, trivial⟩

/-- What every classical model of the theory inside the region must make of a
formula, closed in both directions: an atom held or cut by the region, falsum,
every root; upward, a formula from its operands as the connective dictates;
downward, an operand from a formula whose truth is known and the other
operand, as unit propagation over a clause form of the theory would decide.
The knowledge of a shared subformula serves every formula containing it. -/
inductive Known (T : Theory α) (c : Cube α) : Formula α → Bool → Prop
  | held {a} (h : c.lower a) : Known T c (.atom a) true
  | cut {a} (h : ¬ c.upper a) : Known T c (.atom a) false
  | bot : Known T c .bot false
  | root {F} (h : F ∈ T) : Known T c F true
  | conj_up {F G} (hF : Known T c F true) (hG : Known T c G true) :
      Known T c (.conj F G) true
  | conj_up_left {F G} (hF : Known T c F false) : Known T c (.conj F G) false
  | conj_up_right {F G} (hG : Known T c G false) : Known T c (.conj F G) false
  | disj_up_left {F G} (hF : Known T c F true) : Known T c (.disj F G) true
  | disj_up_right {F G} (hG : Known T c G true) : Known T c (.disj F G) true
  | disj_up {F G} (hF : Known T c F false) (hG : Known T c G false) :
      Known T c (.disj F G) false
  | imp_up_left {F G} (hF : Known T c F false) : Known T c (.imp F G) true
  | imp_up_right {F G} (hG : Known T c G true) : Known T c (.imp F G) true
  | imp_up {F G} (hF : Known T c F true) (hG : Known T c G false) :
      Known T c (.imp F G) false
  | conj_left {F G} (h : Known T c (.conj F G) true) : Known T c F true
  | conj_right {F G} (h : Known T c (.conj F G) true) : Known T c G true
  | disj_left {F G} (h : Known T c (.disj F G) true) (hG : Known T c G false) :
      Known T c F true
  | disj_right {F G} (h : Known T c (.disj F G) true) (hF : Known T c F false) :
      Known T c G true
  | imp_consequent {F G} (h : Known T c (.imp F G) true) (hF : Known T c F true) :
      Known T c G true
  | imp_antecedent {F G} (h : Known T c (.imp F G) true) (hG : Known T c G false) :
      Known T c F false
  | not_conj_left {F G} (h : Known T c (.conj F G) false) (hG : Known T c G true) :
      Known T c F false
  | not_conj_right {F G} (h : Known T c (.conj F G) false) (hF : Known T c F true) :
      Known T c G false
  | not_disj_left {F G} (h : Known T c (.disj F G) false) : Known T c F false
  | not_disj_right {F G} (h : Known T c (.disj F G) false) : Known T c G false
  | not_imp_antecedent {F G} (h : Known T c (.imp F G) false) : Known T c F true
  | not_imp_consequent {F G} (h : Known T c (.imp F G) false) : Known T c G false

/-- The knowledge is sound in every classical model of the theory inside the
region: what is known to hold does, and what is known to fail does not.
Induction on the derivation; the antecedent of a failing implication needs
the excluded middle. -/
theorem known_sound (T : Theory α) (c : Cube α) {z : Atoms α} (hz : c.Contains z)
    (hm : Models z T) {F : Formula α} {b : Bool} (h : Known T c F b) :
    (b = true → Satisfies z F) ∧ (b = false → ¬ Satisfies z F) := by
  induction h with
  | held ha => exact ⟨(fun _ => hz.1 _ ha), (fun e => nomatch e)⟩
  | cut ha => exact ⟨(fun e => nomatch e), (fun _ sa => ha (hz.2 _ sa))⟩
  | bot => exact ⟨(fun e => nomatch e), (fun _ sb => sb)⟩
  | root hF => exact ⟨(fun _ => hm _ hF), (fun e => nomatch e)⟩
  | conj_up _ _ ihF ihG => exact ⟨(fun _ => ⟨ihF.1 rfl, ihG.1 rfl⟩), (fun e => nomatch e)⟩
  | conj_up_left _ ihF => exact ⟨(fun e => nomatch e), (fun _ s => ihF.2 rfl s.1)⟩
  | conj_up_right _ ihG => exact ⟨(fun e => nomatch e), (fun _ s => ihG.2 rfl s.2)⟩
  | disj_up_left _ ihF => exact ⟨(fun _ => Or.inl (ihF.1 rfl)), (fun e => nomatch e)⟩
  | disj_up_right _ ihG => exact ⟨(fun _ => Or.inr (ihG.1 rfl)), (fun e => nomatch e)⟩
  | disj_up _ _ ihF ihG =>
    refine ⟨(fun e => nomatch e), (fun _ s => ?_)⟩
    rcases s with s | s
    · exact ihF.2 rfl s
    · exact ihG.2 rfl s
  | imp_up_left _ ihF => exact ⟨(fun _ sF => (ihF.2 rfl sF).elim), (fun e => nomatch e)⟩
  | imp_up_right _ ihG => exact ⟨(fun _ _ => ihG.1 rfl), (fun e => nomatch e)⟩
  | imp_up _ _ ihF ihG => exact ⟨(fun e => nomatch e), (fun _ s => ihG.2 rfl (s (ihF.1 rfl)))⟩
  | conj_left _ ih => exact ⟨(fun _ => (ih.1 rfl).1), (fun e => nomatch e)⟩
  | conj_right _ ih => exact ⟨(fun _ => (ih.1 rfl).2), (fun e => nomatch e)⟩
  | disj_left _ _ ih ihG =>
    refine ⟨(fun _ => ?_), (fun e => nomatch e)⟩
    rcases ih.1 rfl with s | s
    · exact s
    · exact (ihG.2 rfl s).elim
  | disj_right _ _ ih ihF =>
    refine ⟨(fun _ => ?_), (fun e => nomatch e)⟩
    rcases ih.1 rfl with s | s
    · exact (ihF.2 rfl s).elim
    · exact s
  | imp_consequent _ _ ih ihF => exact ⟨(fun _ => ih.1 rfl (ihF.1 rfl)), (fun e => nomatch e)⟩
  | imp_antecedent _ _ ih ihG =>
    exact ⟨(fun e => nomatch e), (fun _ sF => ihG.2 rfl (ih.1 rfl sF))⟩
  | not_conj_left _ _ ih ihG =>
    exact ⟨(fun e => nomatch e), (fun _ sF => ih.2 rfl ⟨sF, ihG.1 rfl⟩)⟩
  | not_conj_right _ _ ih ihF =>
    exact ⟨(fun e => nomatch e), (fun _ sG => ih.2 rfl ⟨ihF.1 rfl, sG⟩)⟩
  | not_disj_left _ ih => exact ⟨(fun e => nomatch e), (fun _ sF => ih.2 rfl (Or.inl sF))⟩
  | not_disj_right _ ih => exact ⟨(fun e => nomatch e), (fun _ sG => ih.2 rfl (Or.inr sG))⟩
  | not_imp_antecedent _ ih =>
    refine ⟨(fun _ => ?_), (fun e => nomatch e)⟩
    exact Classical.byContradiction fun nF => ih.2 rfl (fun sF => (nF sF).elim)
  | not_imp_consequent _ ih =>
    exact ⟨(fun e => nomatch e), (fun _ sG => ih.2 rfl (fun _ => sG))⟩

/-- An atom known to hold is in every stable model of the region. -/
theorem known_forces (T : Theory α) (c : Cube α) {a : α}
    (h : Known T c (.atom a) true) : ∀ z, c.Contains z → Stable z T → z a :=
  fun z hz hs => (known_sound (z := z) T c hz hs.1 h).1 rfl

/-- An atom known to fail is in no stable model of the region. -/
theorem known_cuts (T : Theory α) (c : Cube α) {a : α}
    (h : Known T c (.atom a) false) : ∀ z, c.Contains z → Stable z T → ¬ z a :=
  fun z hz hs => (known_sound (z := z) T c hz hs.1 h).2 rfl

/-- A formula known both to hold and to fail refutes the region. -/
theorem known_contradiction_refutes (T : Theory α) (c : Cube α) {F : Formula α}
    (ht : Known T c F true) (hf : Known T c F false) :
    ∀ z, c.Contains z → ¬ Stable z T :=
  fun z hz hs =>
    (known_sound (z := z) T c hz hs.1 hf).2 rfl ((known_sound (z := z) T c hz hs.1 ht).1 rfl)

/-- A producer cannot support `a` under the region: its body is impossible,
or another of its heads is held. -/
def Blocked (c : Cube α) (r : Producer α) (a : α) : Prop :=
  (match r with
    | .fact _ => False
    | .rule B _ => Never c B) ∨
  ∃ b, r.head.Contains b ∧ c.lower b ∧ b ≠ a

theorem blocked_no_support (c : Cube α) (r : Producer α) (a : α) (hb : Blocked c r a) :
    ∀ z, c.Contains z → ¬ r.Supports z a := by
  intro z hz hs
  rcases hb with body | ⟨b, hb, held, ne⟩
  · cases r with
    | fact _ => exact body
    | rule B _ => exact never_sound c hz body hs.1
  · exact ne (hs.2.2 b hb (hz.1 b held))

/-- An atom none of whose producers can support it under the region is in
no stable model of the region. -/
theorem unsupported_cut (T : Theory α) (rules : List (Producer α))
    (covered : Covered T rules) (original : ∀ r ∈ rules, r.formula ∈ T)
    (c : Cube α) (a : α) (h : ∀ r ∈ rules, r.head.Contains a → Blocked c r a) :
    ∀ z, c.Contains z → Stable z T → ¬ z a := by
  intro z hz hs present
  obtain ⟨r, member, support⟩ :=
    answer_set_supported z T rules covered original hs a present
  exact blocked_no_support c r a (h r member support.2.1) z hz support

/-- An atom held in the region whose every producer but one is blocked is
supported by that one in every stable model of the region, so that
producer is enabled there: its body holds. -/
theorem sole_support_forces (T : Theory α) (rules : List (Producer α))
    (covered : Covered T rules) (original : ∀ r ∈ rules, r.formula ∈ T)
    (c : Cube α) (a : α) (r : Producer α)
    (h : ∀ r' ∈ rules, r'.head.Contains a → r' ≠ r → Blocked c r' a) :
    ∀ z, c.Contains z → Stable z T → z a → r.Enabled z := by
  intro z hz hs present
  obtain ⟨r', member, support⟩ :=
    answer_set_supported z T rules covered original hs a present
  by_cases eq : r' = r
  · subst eq
    exact support.1
  · exact (blocked_no_support c r' a (h r' member support.2.1 eq) z hz support).elim

end Zetesis.FormulaBounds
