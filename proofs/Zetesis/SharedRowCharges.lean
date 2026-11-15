import Std.Tactic

/-!
# Charging shared rows once while retaining every guard visit

A finite family has a nonnegative row charge, a count of live guards and a
nonnegative guard supplement. Charging one row owner instead of one row scan
per guard cannot increase the total charge. A family with no live guards costs
nothing; one live guard preserves its former charge. Summing these inequalities
preserves every common charge and an already admitted finite sweep prefix.

These scalar laws do not establish shader-loop bounds, finite-width overflow,
row identity, occurrence coverage, cancellation or memory ownership. The
implementation must authenticate each authoritative row and preserve repeated
condition occurrences and every live guard before applying these laws. The
charges are logical-work tariffs, not retired machine instructions.
-/
namespace Zetesis.SharedRowCharges

/-- One authoritative row and all of its live guards, with their nonnegative
logical-work charges. The supplement includes guard work, not another row scan. -/
structure Family where
  row : Nat
  guards : Nat
  supplement : Nat

/-- The former tariff scans the row once per live guard and retains the guard
supplement. A family with no live guards has no execution to charge. -/
def occurrence (f : Family) : Nat :=
  if f.guards = 0 then 0 else f.guards * f.row + f.supplement

/-- The shared tariff scans one live row owner and retains every guard visit. -/
def owner (f : Family) : Nat :=
  if f.guards = 0 then 0 else f.row + f.supplement

/-- A family without a live guard has no row owner to charge. -/
theorem empty_owner (row supplement : Nat) :
    owner ⟨row, 0, supplement⟩ = 0 := by
  simp [owner]

/-- A one-guard family has exactly the same charge under either tariff. -/
theorem one_guard (row supplement : Nat) :
    owner ⟨row, 1, supplement⟩ = occurrence ⟨row, 1, supplement⟩ := by
  simp [owner, occurrence]

/-- Charging a live row once cannot exceed charging it once per guard.
For a nonempty guard family, one scan is bounded by its positive scan count;
the unchanged guard supplement preserves that inequality. -/
theorem owner_le_occurrence (f : Family) : owner f ≤ occurrence f := by
  by_cases empty : f.guards = 0
  · simp [owner, occurrence, empty]
  · have positive : 1 ≤ f.guards := by omega
    have scans : f.row ≤ f.guards * f.row := by
      simpa using Nat.mul_le_mul_right f.row positive
    simpa [owner, occurrence, empty] using
      Nat.add_le_add_right scans f.supplement

/-- Add the supplied family charges over a finite inventory. -/
def total (charge : Family → Nat) : List Family → Nat
  | [] => 0
  | first :: rest => charge first + total charge rest

/-- Summing over the same finite family inventory preserves the row-charge
inequality. No family or guard is removed by this scalar comparison. -/
theorem total_owner_le (families : List Family) :
    total owner families ≤ total occurrence families := by
  induction families with
  | nil => simp [total]
  | cons first rest induction =>
    exact Nat.add_le_add (owner_le_occurrence first) induction

/-- Adding unchanged work preserves the finite row-charge inequality. -/
theorem common_terms_preserved (common : Nat) (families : List Family) :
    common + total owner families ≤ common + total occurrence families := by
  exact Nat.add_le_add_left (total_owner_le families) common

/-- Every previously admitted finite sweep prefix still fits after replacing
per-guard row charges with owner charges. Setup, common sweep work, sweep count
and ceiling remain unchanged; the law does not justify extra uncharged work. -/
theorem accepted_prefix_stays_within (setup common sweeps ceiling : Nat)
    (families : List Family)
    (admitted : setup + sweeps * (common + total occurrence families) ≤ ceiling) :
    setup + sweeps * (common + total owner families) ≤ ceiling := by
  have bound := Nat.mul_le_mul_left sweeps (common_terms_preserved common families)
  exact Nat.le_trans (Nat.add_le_add_left bound setup) admitted

end Zetesis.SharedRowCharges
