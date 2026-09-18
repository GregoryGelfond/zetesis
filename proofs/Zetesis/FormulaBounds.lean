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
In the producer fragment of `DisjunctiveSupport`, an atom none of whose
producers can support it under the region, because each has an impossible
body or another head held, is cut: `answer_set_supported` says every atom of
a stable model has a supporting producer.

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

end Zetesis.FormulaBounds
