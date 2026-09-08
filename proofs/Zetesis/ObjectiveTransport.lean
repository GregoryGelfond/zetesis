import Zetesis.ObjectiveDirections

/-!
# Objective observations through total relation renamings

A predicate renaming with an argument permutation transports complete rows
without selecting rows or changing their values. Two distinct correspondences
matter: completed grounding carriers determine priority presence, while truth
in an interpretation determines active contribution keys. These laws keep those
carriers separate. Presence is never reconstructed from accepted models.

The cost laws retain complete normalized keys and therefore compose with global
tuple coalescing, mixed objective directions and zero weights. The supplied
truth correspondence applies independently to any original or frozen-reduct
interpretation; it is not a stability certificate.

This module establishes finite semantic transport laws. It does not prove the
Rust structural recognizer, grounding completion, clingo's priority carrier,
finite-width arithmetic, or an optimization of the original Ferraris theory.
-/

namespace Zetesis.ObjectiveTransport

open ObjectiveDirections

universe u v w x
variable {σ : Type u} {χ : Type v} {ψ : Type w} {ρ : Type x}

/-- A completed carrier retains a priority precisely when one of its rows
    supplies the required numeric contribution. Its eligibility predicate is
    separate from model-relative activation. -/
def Present (rows : List χ) (numeric : χ → Prop) : Prop :=
  ∃ row, row ∈ rows ∧ numeric row

/-- Transporting every completed row preserves existence of a numeric witness.
    The pointwise premise concerns numeric presence, not accepted-model truth. -/
theorem present_transport (rows : List χ) (rename : χ → ψ)
    (before : χ → Prop) (after : ψ → Prop)
    (correspondence : ∀ row, after (rename row) ↔ before row) :
    Present (rows.map rename) after ↔ Present rows before := by
  constructor
  · rintro ⟨image, member, numeric⟩
    obtain ⟨row, original, same⟩ := List.mem_map.mp member
    have retained : before row :=
      (correspondence row).mp (same.symm ▸ numeric)
    exact ⟨row, original, retained⟩
  · rintro ⟨row, member, numeric⟩
    have retained : after (rename row) := (correspondence row).mpr numeric
    exact ⟨rename row, List.mem_map.mpr ⟨row, member, rfl⟩, retained⟩

/-- Renaming eligibility leaves direction, numeric weight, priority and every
    tuple component unchanged. In particular, zero is still a contribution. -/
def transport (rename : χ → ψ) (entry : Entry σ χ) : Entry σ ψ :=
  ⟨entry.direction, entry.weight, entry.priority, entry.tuple, rename entry.condition⟩

theorem transport_key (rename : χ → ψ) (entry : Entry σ χ) :
    normalizedKey (transport rename entry) = normalizedKey entry := by
  rfl

/-- Any finite chain of renamings composes before the objective boundary. -/
theorem transport_composes (first : χ → ψ) (second : ψ → ρ)
    (entry : Entry σ χ) :
    transport second (transport first entry) = transport (second ∘ first) entry := by
  rfl

/-- Original and transported activation select the same complete keys in the
    same order. Duplicate eligibility alternatives remain duplicate occurrences;
    they are not mistaken for separate contribution identities. -/
theorem active_keys_transport (entries : List (Entry σ χ)) (rename : χ → ψ)
    (before : χ → Bool) (after : ψ → Bool)
    (correspondence : ∀ condition, after (rename condition) = before condition) :
    (((entries.map (transport rename)).filter (fun entry => after entry.condition)).map
      normalizedKey) =
      (entries.filter (fun entry => before entry.condition)).map normalizedKey := by
  induction entries with
  | nil => rfl
  | cons entry rest ih =>
    have same : after (transport rename entry).condition = before entry.condition :=
      correspondence entry.condition
    cases active : before entry.condition <;>
      simp [same, active, transport_key, ih]

/-- Global key coalescing gives identical integer costs at each priority. This
    law assumes neither unique source occurrences nor nonzero weights. -/
theorem mixed_cost_transport [DecidableEq σ] (priority : Int)
    (entries : List (Entry σ χ)) (rename : χ → ψ)
    (before : χ → Bool) (after : ψ → Bool)
    (correspondence : ∀ condition, after (rename condition) = before condition) :
    mixedCost priority after (entries.map (transport rename)) =
      mixedCost priority before entries := by
  have keys := active_keys_transport entries rename before after correspondence
  simp only [mixedCost, raw_cost_keys]
  rw [keys]

/-- A fixed completed priority layout retains all zero slots while its costs
    transport pointwise. Establishing that layout is a separate obligation. -/
theorem cost_vector_transport [DecidableEq σ] (priorities : List Int)
    (entries : List (Entry σ χ)) (rename : χ → ψ)
    (before : χ → Bool) (after : ψ → Bool)
    (correspondence : ∀ condition, after (rename condition) = before condition) :
    priorities.map (fun priority => mixedCost priority after (entries.map (transport rename))) =
      priorities.map (fun priority => mixedCost priority before entries) := by
  apply List.map_congr_left
  intro priority _
  exact mixed_cost_transport priority entries rename before after correspondence

end Zetesis.ObjectiveTransport
