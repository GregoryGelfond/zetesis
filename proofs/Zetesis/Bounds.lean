import Zetesis.Semantics
import Zetesis.Search

/-!
# Concrete must/may bounds for reduct search

This module discharges the search geometry's enclosure obligations using the
actual normalized rule semantics. Positive bodies, filters, and heads remain
unchanged. Only frozen candidate gates are approximated.
-/
namespace Zetesis
namespace Bounds

open Semantics

universe u
variable {α : Type u}

def MustGate (r : Rule α) (c : Cube α) : Prop :=
  (∀ a, a ∈ r.gateTrue → c.lower a) ∧
  (∀ a, a ∈ r.gateFalse → ¬ c.upper a)

def MayGate (r : Rule α) (c : Cube α) : Prop :=
  (∀ a, a ∈ r.gateTrue → c.upper a) ∧
  (∀ a, a ∈ r.gateFalse → ¬ c.lower a)

def SelectedConsequence (P : Program α) (enabled : Rule α → Prop) :
    Transformer α :=
  fun X a => ∃ r, r ∈ P ∧ r.head = some a ∧ r.filter ∧ enabled r ∧ Body r X

def LowerConsequence (P : Program α) (c : Cube α) : Transformer α :=
  SelectedConsequence P (fun r => MustGate r c)

def UpperConsequence (P : Program α) (c : Cube α) : Transformer α :=
  SelectedConsequence P (fun r => MayGate r c)

def LowerGamma (P : Program α) (c : Cube α) : Atoms α :=
  Least (LowerConsequence P c)

def UpperGamma (P : Program α) (c : Cube α) : Atoms α :=
  Least (UpperConsequence P c)

def Narrow (P : Program α) (S : Atoms α) (c : Cube α) : Cube α :=
  c.narrow (Inter (LowerGamma P c) S) (Inter (UpperGamma P c) S)

theorem gate_sandwich (r : Rule α) {c : Cube α} {z : Atoms α}
    (hz : c.Contains z) :
    (MustGate r c → Gate r z) ∧ (Gate r z → MayGate r c) := by
  constructor
  · intro hg
    constructor
    · intro a ha
      exact hz.1 a (hg.1 a ha)
    · intro a ha hza
      exact hg.2 a ha (hz.2 a hza)
  · intro hg
    constructor
    · intro a ha
      exact hz.2 a (hg.1 a ha)
    · intro a ha hla
      exact hg.2 a ha (hz.1 a hla)

theorem selected_consequence_mono (P : Program α) (enabled : Rule α → Prop) :
    MonotoneT (SelectedConsequence P enabled) := by
  intro X Y hXY a ha
  obtain ⟨r, hr, hh, hf, hg, hb⟩ := ha
  exact ⟨r, hr, hh, hf, hg, body_mono hXY hb⟩

theorem selected_consequence_refines (P : Program α)
    {left right : Rule α → Prop} (h : ∀ r, left r → right r) :
    TBelow (SelectedConsequence P left) (SelectedConsequence P right) := by
  intro X a ha
  obtain ⟨r, hr, hh, hf, hg, hb⟩ := ha
  exact ⟨r, hr, hh, hf, h r hg, hb⟩

theorem consequence_sandwich (P : Program α) {c : Cube α} {z : Atoms α}
    (hz : c.Contains z) :
    TBelow (LowerConsequence P c) (Consequence P z) ∧
      TBelow (Consequence P z) (UpperConsequence P c) := by
  constructor
  · exact selected_consequence_refines P (fun r => (gate_sandwich r hz).1)
  · exact selected_consequence_refines P (fun r => (gate_sandwich r hz).2)

/-- The enclosures hold for every seed in the region, before acceptance is known. -/
theorem gamma_sandwich (P : Program α) {c : Cube α} {z : Atoms α}
    (hz : c.Contains z) :
    Sub (LowerGamma P c) (Gamma P z) ∧ Sub (Gamma P z) (UpperGamma P c) :=
  ⟨least_mono (consequence_sandwich P hz).1,
   least_mono (consequence_sandwich P hz).2⟩

/-- An unfinished lower run remains a sound lower bound. -/
theorem lower_prefix_sound (P : Program α) {c : Cube α} {z : Atoms α}
    (hz : c.Contains z) (n : Nat) :
    Sub (FiniteIter (LowerConsequence P c) n) (Gamma P z) :=
  sub_trans (finiteIter_sound (selected_consequence_mono P _) n)
    (gamma_sandwich P hz).1

/-- An upper approximation must be closed, rather than merely an iteration prefix. -/
theorem closed_upper_sound (P : Program α) {c : Cube α} {z X : Atoms α}
    (hz : c.Contains z) (hX : Closed (UpperConsequence P c) X) :
    Sub (Gamma P z) X :=
  sub_trans (gamma_sandwich P hz).2 (least_le hX)

theorem accepted_seed_agrees {P : Program α} {S z : Atoms α}
    (hz : Accept P S z) : EqOn S z (Gamma P z) := by
  intro a hs
  constructor
  · intro ha
    rw [← hz.1] at ha
    exact ha.1
  · intro ha
    rw [← hz.1]
    exact ⟨ha, hs⟩

/-- No enclosure, agreement, or carrier assumption is left to the caller. -/
theorem acceptance_survives_narrowing (P : Program α) (S : Atoms α)
    {c : Cube α} {z : Atoms α} (hc : c.Contains z) (hz : Accept P S z) :
    (Narrow P S c).Contains z := by
  apply Cube.consequence_narrow_preserves (Accept P S) (Gamma P)
    c S (LowerGamma P c) (UpperGamma P c)
  · intro w hw
    exact accepted_seed_in_carrier hw
  · intro w hw
    exact accepted_seed_agrees hw
  · intro w hw
    exact (gamma_sandwich P hw).1
  · intro w hw
    exact (gamma_sandwich P hw).2
  · exact hc
  · exact hz

/-- This creates the concrete ledger's narrowing node from must/may closure. -/
def coverage_narrowed (P : Program α) (S : Atoms α) (c : Cube α)
    (child : CoverageTree (Accept P S) (Narrow P S c)) :
    CoverageTree (Accept P S) c :=
  .narrowed c (Inter (LowerGamma P c) S) (Inter (UpperGamma P c) S)
    (fun _ hc hz =>
      (Cube.contains_narrow.mp (acceptance_survives_narrowing P S hc hz)).2.1)
    (fun _ hc hz =>
      (Cube.contains_narrow.mp (acceptance_survives_narrowing P S hc hz)).2.2)
    child

/-- A contradictory narrowed region refutes every accepted seed in its parent. -/
theorem inconsistent_narrowing_refutes (P : Program α) (S : Atoms α)
    (c : Cube α)
    (inconsistent : ¬ Sub (Narrow P S c).lower (Narrow P S c).upper) :
    ∀ z, c.Contains z → ¬ Accept P S z := by
  intro z hc hz
  have hn := acceptance_survives_narrowing P S hc hz
  exact inconsistent (sub_trans hn.1 hn.2)

/-- A single lower-true / upper-false atom is a concrete inconsistency witness. -/
theorem conflicting_atom_refutes (P : Program α) (S : Atoms α)
    (c : Cube α) (a : α) (hl : (Narrow P S c).lower a)
    (hu : ¬ (Narrow P S c).upper a) :
    ∀ z, c.Contains z → ¬ Accept P S z :=
  inconsistent_narrowing_refutes P S c (fun h => hu (h a hl))

/-- Definite activation and a lower-satisfied constraint refute the whole region. -/
theorem lower_constraint_refutes (P : Program α) (S : Atoms α)
    (c : Cube α) (r : Rule α) (hr : r ∈ P) (hh : r.head = none)
    (hf : r.filter) (hg : MustGate r c) (hb : Body r (LowerGamma P c)) :
    ∀ z, c.Contains z → ¬ Accept P S z := by
  intro z hc hz
  exact hz.2 r hr hh hf ((gate_sandwich r hc).1 hg)
    (body_mono (gamma_sandwich P hc).1 hb)

/-- Partial lower work already suffices to expose a definite constraint violation. -/
theorem lower_prefix_constraint_refutes (P : Program α) (S : Atoms α)
    (c : Cube α) (r : Rule α) (hr : r ∈ P) (hh : r.head = none)
    (hf : r.filter) (hg : MustGate r c) (n : Nat)
    (hb : Body r (FiniteIter (LowerConsequence P c) n)) :
    ∀ z, c.Contains z → ¬ Accept P S z := by
  intro z hc hz
  exact hz.2 r hr hh hf ((gate_sandwich r hc).1 hg)
    (body_mono (lower_prefix_sound P hc n) hb)

end Bounds
end Zetesis
