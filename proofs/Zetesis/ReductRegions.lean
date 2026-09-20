import Zetesis.Ferraris
import Zetesis.Search
import Zetesis.FormulaBounds

/-!
# The proper-subset query as a region tree

Membership of a classical model `M` of a theory is decided by its frozen
reduct: `M` is stable exactly when no proper subset of `M` models the
reduct (`Ferraris.Stable`). That query is a coverage tree over the cube
whose lower bound is empty and whose upper bound is `M`, with the frozen
reduct as the theory: a leaf other than `M` that models the reduct is a
proper-subset model and refutes stability, and a tree with no such leaf
proves it. `FormulaBounds.known_sound` narrows the query's regions over the
reduct theory as it narrows the candidate tree over the original one, and
`decided_leaf_models` makes every surviving leaf a model of the reduct.
The Rust query reads the original DAG under the candidate's frozen truth
mask rather than building the reduct. That evaluation under the mask is the
reduct's is `FerrarisMask`'s law; that the narrowing's two readings under the
mask are the reduct's is `masked_read_eq_reduct`, and that propagation under
the mask, where a masked node has no rules of its own and still informs the
nodes above it, knows only what propagation over the reduct theory does is
`masked_known_sound`. Here the mask is the truth of each formula in the
candidate; that the stored mask is that truth, and that the Rust propagates
by these rules over the shared DAG, are Rust obligations.
-/

namespace Zetesis.ReductRegions

open Ferraris

universe u

variable {α : Type u}

/-- The query's root: every atom of `M` open, every other atom cut. -/
def query (M : Atoms α) : Cube α := ⟨fun _ => False, M⟩

theorem query_contains (M J : Atoms α) : (query M).Contains J ↔ Sub J M := by
  constructor
  · exact fun h => h.2
  · exact fun h => ⟨fun _ f => f.elim, h⟩

/-- A proper subset of `M` that models the frozen reduct refutes stability. -/
theorem leaf_refutes (M J : Atoms α) (T : Theory α) (sub : ProperSub J M)
    (models : Models J (ReductTheory M T)) : ¬ Stable M T :=
  fun stable => stable.2 ⟨J, sub, models⟩

/-- A classical model of the theory whose query tree has no proper-subset
leaf is stable: the tree's coverage says no proper subset models the reduct. -/
theorem exhausted_stable (M : Atoms α) (T : Theory α) (original : Models M T)
    (none : ∀ J, ProperSub J M → ¬ Models J (ReductTheory M T)) : Stable M T :=
  ⟨original, fun ⟨J, sub, models⟩ => none J sub models⟩

/-- The valid seeds of the query tree: proper subsets of `M` modelling the
reduct. A coverage tree over the query root with this validity lists
exactly the countermodels (`CoverageTree.mem_outputs_iff`). -/
def Countermodel (M : Atoms α) (T : Theory α) (J : Atoms α) : Prop :=
  ProperSub J M ∧ Models J (ReductTheory M T)

theorem countermodels_exact (M : Atoms α) (T : Theory α)
    (tree : CoverageTree (Countermodel M T) (query M)) (J : Atoms α) :
    J ∈ tree.outputs ↔ Countermodel M T J := by
  rw [CoverageTree.mem_outputs_iff]
  constructor
  · exact fun h => h.2
  · exact fun h => ⟨(query_contains M J).2 h.1.1, h⟩

/-- Stability from an empty query tree, and its refutation from a nonempty one. -/
theorem stable_iff_no_countermodel (M : Atoms α) (T : Theory α) (original : Models M T)
    (tree : CoverageTree (Countermodel M T) (query M)) :
    Stable M T ↔ tree.outputs = [] := by
  constructor
  · intro stable
    cases h : tree.outputs with
    | nil => rfl
    | cons J rest =>
      have mem : J ∈ tree.outputs := by rw [h]; exact List.mem_cons_self ..
      exact (leaf_refutes M J T ((countermodels_exact M T tree J).1 mem).1
        ((countermodels_exact M T tree J).1 mem).2 stable).elim
  · intro empty
    refine exhausted_stable M T original fun J sub models => ?_
    have mem : J ∈ tree.outputs := (countermodels_exact M T tree J).2 ⟨sub, models⟩
    rw [empty] at mem
    exact nomatch mem

/-! ## The readings and the knowledge under the frozen mask

The Rust query does not build the reduct. It reads the original formulas
under the candidate's frozen mask, the truth of each formula in the candidate.
A masked formula, one false in the candidate, is falsum in the reduct. So it
has no rules of its own: its reading is falsum's whatever its operands read,
and no knowledge passes between it and them. It still informs the formulas
above it, which combine falsum's reading, and know it to fail, as they would
any operand's. The two laws below say that reading and propagating this way
over the original formulas is reading and propagating over the reduct. -/

open FormulaBounds

/-- A formula false in the candidate is falsum in the reduct. -/
theorem reduct_masked (M : Atoms α) {F : Formula α} (masked : ¬ Satisfies M F) :
    Reduct M F = .bot := by
  cases F with
  | atom a =>
    have absent : ¬ M a := masked
    simp [Reduct, absent]
  | bot => rfl
  | conj F G => simp [Reduct, masked]
  | disj F G => simp [Reduct, masked]
  | imp F G => simp [Reduct, masked]

/-- The reduct of a formula true in the candidate keeps its connective. -/
theorem reduct_conj (M : Atoms α) {F G : Formula α} (h : Satisfies M (.conj F G)) :
    Reduct M (.conj F G) = .conj (Reduct M F) (Reduct M G) := by simp [Reduct, h]

theorem reduct_disj (M : Atoms α) {F G : Formula α} (h : Satisfies M (.disj F G)) :
    Reduct M (.disj F G) = .disj (Reduct M F) (Reduct M G) := by simp [Reduct, h]

theorem reduct_imp (M : Atoms α) {F G : Formula α} (h : Satisfies M (.imp F G)) :
    Reduct M (.imp F G) = .imp (Reduct M F) (Reduct M G) := by simp [Reduct, h]

theorem reduct_atom (M : Atoms α) {a : α} (h : M a) : Reduct M (.atom a) = .atom a := by
  simp [Reduct, h]

/-- The two readings of an original formula under the candidate's mask: a
masked formula reads as falsum, never sure and always impossible, and an
unmasked one combines the masked readings of its operands as `read` does. -/
noncomputable def maskedRead (M : Atoms α) (c : Cube α) : Formula α → Prop × Prop
  | .atom a => open Classical in
      if M a then (c.lower a, ¬ c.upper a) else (False, True)
  | .bot => (False, True)
  | .conj F G => open Classical in
      if Satisfies M (.conj F G)
      then ((maskedRead M c F).1 ∧ (maskedRead M c G).1,
        (maskedRead M c F).2 ∨ (maskedRead M c G).2)
      else (False, True)
  | .disj F G => open Classical in
      if Satisfies M (.disj F G)
      then ((maskedRead M c F).1 ∨ (maskedRead M c G).1,
        (maskedRead M c F).2 ∧ (maskedRead M c G).2)
      else (False, True)
  | .imp F G => open Classical in
      if Satisfies M (.imp F G)
      then ((maskedRead M c F).2 ∨ (maskedRead M c G).1,
        (maskedRead M c F).1 ∧ (maskedRead M c G).2)
      else (False, True)

/-- Reading the original formula under the mask is reading its reduct.
Induction on the formula: a masked formula and falsum both read as falsum,
and an unmasked connective is the reduct's connective over the operands'
reducts, whose readings agree by the induction hypotheses. -/
theorem masked_read_eq_reduct (M : Atoms α) (c : Cube α) (F : Formula α) :
    maskedRead M c F = read c (Reduct M F) := by
  classical
  induction F with
  | atom a =>
    by_cases h : M a <;> simp [maskedRead, Reduct, FormulaBounds.read, h]
  | bot => rfl
  | conj F G ihF ihG =>
    by_cases h : Satisfies M (.conj F G)
    · simp only [maskedRead, if_pos h, reduct_conj M h, FormulaBounds.read, ihF, ihG]
    · simp only [maskedRead, if_neg h, reduct_masked M h, FormulaBounds.read]
  | disj F G ihF ihG =>
    by_cases h : Satisfies M (.disj F G)
    · simp only [maskedRead, if_pos h, reduct_disj M h, FormulaBounds.read, ihF, ihG]
    · simp only [maskedRead, if_neg h, reduct_masked M h, FormulaBounds.read]
  | imp F G ihF ihG =>
    by_cases h : Satisfies M (.imp F G)
    · simp only [maskedRead, if_pos h, reduct_imp M h, FormulaBounds.read, ihF, ihG]
    · simp only [maskedRead, if_neg h, reduct_masked M h, FormulaBounds.read]

/-- A masked formula reads as falsum whatever its operands read: it has no
rules of its own. -/
theorem masked_reads_falsum (M : Atoms α) (c : Cube α) {F : Formula α}
    (masked : ¬ Satisfies M F) : maskedRead M c F = (False, True) := by
  rw [masked_read_eq_reduct, reduct_masked M masked]
  rfl

/-- What propagation over the original formulas under the mask establishes.
A masked formula is known to fail, and that is all that is known of it from
itself: every rule that relates a connective to its operands, upward or
downward, asks that the connective be unmasked. A root is unmasked, the
candidate being a model, and is known to hold. -/
inductive MaskedKnown (M : Atoms α) (T : Theory α) (c : Cube α) : Formula α → Bool → Prop
  | masked {F} (h : ¬ Satisfies M F) : MaskedKnown M T c F false
  | held {a} (m : M a) (h : c.lower a) : MaskedKnown M T c (.atom a) true
  | cut {a} (h : ¬ c.upper a) : MaskedKnown M T c (.atom a) false
  | root {F} (h : F ∈ T) : MaskedKnown M T c F true
  | conj_up {F G} (m : Satisfies M (.conj F G)) (hF : MaskedKnown M T c F true)
      (hG : MaskedKnown M T c G true) : MaskedKnown M T c (.conj F G) true
  | conj_up_left {F G} (m : Satisfies M (.conj F G)) (hF : MaskedKnown M T c F false) :
      MaskedKnown M T c (.conj F G) false
  | conj_up_right {F G} (m : Satisfies M (.conj F G)) (hG : MaskedKnown M T c G false) :
      MaskedKnown M T c (.conj F G) false
  | disj_up_left {F G} (m : Satisfies M (.disj F G)) (hF : MaskedKnown M T c F true) :
      MaskedKnown M T c (.disj F G) true
  | disj_up_right {F G} (m : Satisfies M (.disj F G)) (hG : MaskedKnown M T c G true) :
      MaskedKnown M T c (.disj F G) true
  | disj_up {F G} (m : Satisfies M (.disj F G)) (hF : MaskedKnown M T c F false)
      (hG : MaskedKnown M T c G false) : MaskedKnown M T c (.disj F G) false
  | imp_up_left {F G} (m : Satisfies M (.imp F G)) (hF : MaskedKnown M T c F false) :
      MaskedKnown M T c (.imp F G) true
  | imp_up_right {F G} (m : Satisfies M (.imp F G)) (hG : MaskedKnown M T c G true) :
      MaskedKnown M T c (.imp F G) true
  | imp_up {F G} (m : Satisfies M (.imp F G)) (hF : MaskedKnown M T c F true)
      (hG : MaskedKnown M T c G false) : MaskedKnown M T c (.imp F G) false
  | conj_left {F G} (m : Satisfies M (.conj F G))
      (h : MaskedKnown M T c (.conj F G) true) : MaskedKnown M T c F true
  | conj_right {F G} (m : Satisfies M (.conj F G))
      (h : MaskedKnown M T c (.conj F G) true) : MaskedKnown M T c G true
  | disj_left {F G} (m : Satisfies M (.disj F G))
      (h : MaskedKnown M T c (.disj F G) true) (hG : MaskedKnown M T c G false) :
      MaskedKnown M T c F true
  | disj_right {F G} (m : Satisfies M (.disj F G))
      (h : MaskedKnown M T c (.disj F G) true) (hF : MaskedKnown M T c F false) :
      MaskedKnown M T c G true
  | imp_consequent {F G} (m : Satisfies M (.imp F G))
      (h : MaskedKnown M T c (.imp F G) true) (hF : MaskedKnown M T c F true) :
      MaskedKnown M T c G true
  | imp_antecedent {F G} (m : Satisfies M (.imp F G))
      (h : MaskedKnown M T c (.imp F G) true) (hG : MaskedKnown M T c G false) :
      MaskedKnown M T c F false
  | not_conj_left {F G} (m : Satisfies M (.conj F G))
      (h : MaskedKnown M T c (.conj F G) false) (hG : MaskedKnown M T c G true) :
      MaskedKnown M T c F false
  | not_conj_right {F G} (m : Satisfies M (.conj F G))
      (h : MaskedKnown M T c (.conj F G) false) (hF : MaskedKnown M T c F true) :
      MaskedKnown M T c G false
  | not_disj_left {F G} (m : Satisfies M (.disj F G))
      (h : MaskedKnown M T c (.disj F G) false) : MaskedKnown M T c F false
  | not_disj_right {F G} (m : Satisfies M (.disj F G))
      (h : MaskedKnown M T c (.disj F G) false) : MaskedKnown M T c G false
  | not_imp_antecedent {F G} (m : Satisfies M (.imp F G))
      (h : MaskedKnown M T c (.imp F G) false) : MaskedKnown M T c F true
  | not_imp_consequent {F G} (m : Satisfies M (.imp F G))
      (h : MaskedKnown M T c (.imp F G) false) : MaskedKnown M T c G false

/-- Propagation under the mask establishes only what propagation over the
reduct theory does: what is known of an original formula under the mask is
known of its reduct. Induction on the derivation. A masked formula's reduct
is falsum, known to fail; a root's reduct is a root of the reduct theory; and
each rule at an unmasked connective is the same rule at the reduct's
connective, which `reduct_conj`, `reduct_disj` and `reduct_imp` expose. -/
theorem masked_known_sound (M : Atoms α) (T : Theory α) (c : Cube α)
    {F : Formula α} {b : Bool} (h : MaskedKnown M T c F b) :
    Known (ReductTheory M T) c (Reduct M F) b := by
  induction h with
  | masked h => rw [reduct_masked M h]; exact .bot
  | held m h => rw [reduct_atom M m]; exact .held h
  | @cut a h =>
    by_cases m : M a
    · rw [reduct_atom M m]; exact .cut h
    · rw [reduct_masked M (F := .atom a) m]; exact .bot
  | root h => exact .root (List.mem_map.mpr ⟨_, h, rfl⟩)
  | conj_up m _ _ ihF ihG => rw [reduct_conj M m]; exact .conj_up ihF ihG
  | conj_up_left m _ ihF => rw [reduct_conj M m]; exact .conj_up_left ihF
  | conj_up_right m _ ihG => rw [reduct_conj M m]; exact .conj_up_right ihG
  | disj_up_left m _ ihF => rw [reduct_disj M m]; exact .disj_up_left ihF
  | disj_up_right m _ ihG => rw [reduct_disj M m]; exact .disj_up_right ihG
  | disj_up m _ _ ihF ihG => rw [reduct_disj M m]; exact .disj_up ihF ihG
  | imp_up_left m _ ihF => rw [reduct_imp M m]; exact .imp_up_left ihF
  | imp_up_right m _ ihG => rw [reduct_imp M m]; exact .imp_up_right ihG
  | imp_up m _ _ ihF ihG => rw [reduct_imp M m]; exact .imp_up ihF ihG
  | conj_left m _ ih => rw [reduct_conj M m] at ih; exact .conj_left ih
  | conj_right m _ ih => rw [reduct_conj M m] at ih; exact .conj_right ih
  | disj_left m _ _ ih ihG => rw [reduct_disj M m] at ih; exact .disj_left ih ihG
  | disj_right m _ _ ih ihF => rw [reduct_disj M m] at ih; exact .disj_right ih ihF
  | imp_consequent m _ _ ih ihF => rw [reduct_imp M m] at ih; exact .imp_consequent ih ihF
  | imp_antecedent m _ _ ih ihG => rw [reduct_imp M m] at ih; exact .imp_antecedent ih ihG
  | not_conj_left m _ _ ih ihG => rw [reduct_conj M m] at ih; exact .not_conj_left ih ihG
  | not_conj_right m _ _ ih ihF => rw [reduct_conj M m] at ih; exact .not_conj_right ih ihF
  | not_disj_left m _ ih => rw [reduct_disj M m] at ih; exact .not_disj_left ih
  | not_disj_right m _ ih => rw [reduct_disj M m] at ih; exact .not_disj_right ih
  | not_imp_antecedent m _ ih => rw [reduct_imp M m] at ih; exact .not_imp_antecedent ih
  | not_imp_consequent m _ ih => rw [reduct_imp M m] at ih; exact .not_imp_consequent ih

/-- What propagation under the mask knows is sound in every subset of the
candidate that models the reduct inside the region: the query may narrow by
it. Composition of `masked_known_sound` with `known_sound` over the reduct
theory. -/
theorem masked_known_narrows (M : Atoms α) (T : Theory α) (c : Cube α) {J : Atoms α}
    (hJ : c.Contains J) (models : Models J (ReductTheory M T))
    {F : Formula α} {b : Bool} (h : MaskedKnown M T c F b) :
    (b = true → Satisfies J (Reduct M F)) ∧ (b = false → ¬ Satisfies J (Reduct M F)) :=
  known_sound (z := J) (ReductTheory M T) c hJ models (masked_known_sound M T c h)

end Zetesis.ReductRegions
