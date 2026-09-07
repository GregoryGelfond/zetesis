import Zetesis.Transformers

/-!
# Normalized reduct semantics

Rules have finite bodies and frozen true/false gates. Filters are propositions
on already-ground values. The source parser and source-to-rule translation are
outside this module. Stability is independently defined as minimal-model
semantics for the positive reduct, including constraints.
-/
namespace Zetesis
namespace Semantics

universe u
variable {α : Type u}

structure Rule (α : Type u) where
  head : Option α
  positive : List α
  gateTrue : List α
  gateFalse : List α
  filter : Prop

abbrev Program (α : Type u) := List (Rule α)

def Body (r : Rule α) (X : Atoms α) : Prop :=
  ∀ a, a ∈ r.positive → X a

def Gate (r : Rule α) (z : Atoms α) : Prop :=
  (∀ a, a ∈ r.gateTrue → z a) ∧
  (∀ a, a ∈ r.gateFalse → ¬ z a)

def Consequence (P : Program α) (z : Atoms α) : Transformer α :=
  fun X a => ∃ r, r ∈ P ∧ r.head = some a ∧ r.filter ∧ Gate r z ∧ Body r X

def ConstraintsOK (P : Program α) (z X : Atoms α) : Prop :=
  ∀ r, r ∈ P → r.head = none → r.filter → Gate r z → ¬ Body r X

def Gamma (P : Program α) (z : Atoms α) : Atoms α := Least (Consequence P z)

def ReductModel (P : Program α) (z X : Atoms α) : Prop :=
  Closed (Consequence P z) X ∧ ConstraintsOK P z X

/-- Minimality is taken with respect to the model's own frozen reduct. -/
def Stable (P : Program α) (M : Atoms α) : Prop :=
  ReductModel P M M ∧
  ∀ X, ReductModel P M X → Sub X M → Sub M X

def GateCarrier (P : Program α) (S : Atoms α) : Prop :=
  ∀ r, r ∈ P →
    (∀ a, a ∈ r.gateTrue → S a) ∧ (∀ a, a ∈ r.gateFalse → S a)

def Accept (P : Program α) (S z : Atoms α) : Prop :=
  Inter (Gamma P z) S = z ∧ ConstraintsOK P z (Gamma P z)

theorem body_mono {r : Rule α} {X Y : Atoms α}
    (h : Sub X Y) (hb : Body r X) : Body r Y :=
  fun a ha => h a (hb a ha)

theorem consequence_mono (P : Program α) (z : Atoms α) :
    MonotoneT (Consequence P z) := by
  intro X Y h a ha
  obtain ⟨r, hr, hh, hf, hg, hb⟩ := ha
  exact ⟨r, hr, hh, hf, hg, body_mono h hb⟩

theorem constraints_down {P : Program α} {z X Y : Atoms α}
    (hXY : Sub X Y) (hY : ConstraintsOK P z Y) : ConstraintsOK P z X := by
  intro r hr hh hf hg hb
  exact hY r hr hh hf hg (body_mono hXY hb)

theorem gamma_closed (P : Program α) (z : Atoms α) :
    Closed (Consequence P z) (Gamma P z) :=
  least_closed (consequence_mono P z)

theorem gamma_le {P : Program α} {z X : Atoms α}
    (h : Closed (Consequence P z) X) : Sub (Gamma P z) X := least_le h

/-- Positive Horn least closure plus constraints equals reduct minimality. -/
theorem stable_iff_gamma (P : Program α) (M : Atoms α) :
    Stable P M ↔ Gamma P M = M ∧ ConstraintsOK P M M := by
  constructor
  · intro ⟨hmodel, hmin⟩
    have hle := gamma_le hmodel.1
    have hgc := constraints_down hle hmodel.2
    have hge := hmin (Gamma P M) ⟨gamma_closed P M, hgc⟩ hle
    exact ⟨sub_antisymm hle hge, hmodel.2⟩
  · intro ⟨heq, hc⟩
    refine ⟨⟨?_, hc⟩, ?_⟩
    · simpa only [heq] using (gamma_closed P M)
    · intro X hX _
      rw [← heq]
      exact gamma_le hX.1

theorem gate_eq_on {r : Rule α} {S z w : Atoms α}
    (hcover : (∀ a, a ∈ r.gateTrue → S a) ∧
      (∀ a, a ∈ r.gateFalse → S a)) (h : EqOn S z w) :
    Gate r z ↔ Gate r w := by
  constructor
  · intro ⟨ht, hf⟩
    constructor
    · intro a ha
      exact (h a (hcover.1 a ha)).mp (ht a ha)
    · intro a ha hw
      exact hf a ha ((h a (hcover.2 a ha)).mpr hw)
  · intro ⟨ht, hf⟩
    constructor
    · intro a ha
      exact (h a (hcover.1 a ha)).mpr (ht a ha)
    · intro a ha hz
      exact hf a ha ((h a (hcover.2 a ha)).mp hz)

theorem consequence_eq_on {P : Program α} {S z w : Atoms α}
    (hS : GateCarrier P S) (h : EqOn S z w) :
    TEquivalent (Consequence P z) (Consequence P w) := by
  intro X a
  constructor
  · rintro ⟨r, hr, hh, hf, hg, hb⟩
    exact ⟨r, hr, hh, hf, (gate_eq_on (hS r hr) h).mp hg, hb⟩
  · rintro ⟨r, hr, hh, hf, hg, hb⟩
    exact ⟨r, hr, hh, hf, (gate_eq_on (hS r hr) h).mpr hg, hb⟩

theorem gamma_eq_on {P : Program α} {S z w : Atoms α}
    (hS : GateCarrier P S) (h : EqOn S z w) : Gamma P z = Gamma P w :=
  least_congr (consequence_eq_on hS h)

theorem constraints_eq_on {P : Program α} {S z w X : Atoms α}
    (hS : GateCarrier P S) (h : EqOn S z w) :
    ConstraintsOK P z X ↔ ConstraintsOK P w X := by
  constructor
  · intro hc r hr hh hf hg
    exact hc r hr hh hf ((gate_eq_on (hS r hr) h).mpr hg)
  · intro hc r hr hh hf hg
    exact hc r hr hh hf ((gate_eq_on (hS r hr) h).mp hg)

theorem accepted_seed_in_carrier {P : Program α} {S z : Atoms α}
    (h : Accept P S z) : Sub z S := by
  intro a ha
  rw [← h.1] at ha
  exact ha.2

/-- Acceptance reconstructs a stable model; it never seeds positive closure. -/
theorem accept_sound {P : Program α} {S z : Atoms α}
    (hS : GateCarrier P S) (h : Accept P S z) : Stable P (Gamma P z) := by
  have hagree : EqOn S z (Gamma P z) := by
    intro a ha
    have hp : (Gamma P z a ∧ S a) = z a := congrArg (fun X => X a) h.1
    exact ⟨fun hz => (Eq.mpr hp hz).1, fun hx => Eq.mp hp ⟨hx, ha⟩⟩
  have heq := gamma_eq_on hS hagree
  apply (stable_iff_gamma P (Gamma P z)).mpr
  constructor
  · exact heq.symm
  · exact (constraints_eq_on hS hagree).mp h.2

/-- Every stable model supplies the sparse true seed obtained by projection. -/
theorem stable_complete {P : Program α} {S M : Atoms α}
    (hS : GateCarrier P S) (hM : Stable P M) :
    Accept P S (Inter M S) ∧ Gamma P (Inter M S) = M := by
  have hagree : EqOn S (Inter M S) M :=
    fun _ ha => ⟨fun hm => hm.1, fun hm => ⟨hm, ha⟩⟩
  obtain ⟨hm, hc⟩ := (stable_iff_gamma P M).mp hM
  have heq : Gamma P (Inter M S) = M := (gamma_eq_on hS hagree).trans hm
  refine ⟨⟨?_, ?_⟩, heq⟩
  · rw [heq]
  · rw [heq]
    exact (constraints_eq_on hS hagree).mpr hc

theorem accepted_seed_unique {P : Program α} {S z w : Atoms α}
    (hz : Accept P S z) (hw : Accept P S w)
    (hmodel : Gamma P z = Gamma P w) : z = w := by
  calc
    z = Inter (Gamma P z) S := hz.1.symm
    _ = Inter (Gamma P w) S := congrArg (fun X => Inter X S) hmodel
    _ = w := hw.1

/-- The complete factorization theorem, including stable-model existence. -/
theorem stable_iff_exists_seed {P : Program α} {S M : Atoms α}
    (hS : GateCarrier P S) :
    Stable P M ↔ ∃ z, Accept P S z ∧ Gamma P z = M := by
  constructor
  · intro hM
    exact ⟨Inter M S, stable_complete hS hM⟩
  · rintro ⟨z, hz, heq⟩
    rw [← heq]
    exact accept_sound hS hz

end Semantics
end Zetesis
