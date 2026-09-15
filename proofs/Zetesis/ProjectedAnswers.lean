import Zetesis.Observations

/-!
# Full representatives of a projected answer family

A key is fixed before enumeration. Projection equivalence compares those keys;
representatives are still members of the selected original family. Selection
includes any objective-optimality condition before projection. The laws below
separate this quotient coverage from full answer-family coverage.

Source grounding must establish the fixed key domain. No law here authenticates
Rust source owners, compiles project conditions, implements bounded key history,
or proves work, storage, cancellation, or completion receipts.
-/

namespace Zetesis.ProjectedAnswers
universe u v
variable {M : Type u} {K : Type v}

/-- Equal keys describe one equivalence class without erasing the original model. -/
def Same (key : M → K) (first second : M) : Prop := key first = key second

/-- Every retained item is an original selected answer, and every original
    selected answer has a retained representative of its key. Uniqueness is a
    separate requirement: coverage alone permits repeated representatives. -/
def Covers (key : M → K) (selected representatives : List M) : Prop :=
  (∀ representative ∈ representatives, representative ∈ selected) ∧
  (∀ model ∈ selected, ∃ representative ∈ representatives, Same key model representative)

/-- Retain the next selected original answer exactly when its key is new.
    The finite list models identity history; no hashing or packed layout is assumed. -/
def retain [DecidableEq K] (key : M → K) (representatives : List M) (model : M) : List M :=
  if key model ∈ representatives.map key then representatives else representatives ++ [model]

/-- One complete identity decision preserves coverage of the consumed answer
    prefix. An existing key has an earlier witness; a new key retains its original
    answer. Both cases preserve membership in the extended selected family. -/
theorem retain_covers [DecidableEq K] (key : M → K) (selected representatives : List M)
    (model : M) (coverage : Covers key selected representatives) :
    Covers key (selected ++ [model]) (retain key representatives model) := by
  by_cases present : key model ∈ representatives.map key
  · have witness : ∃ representative ∈ representatives, Same key model representative := by
      obtain ⟨representative, retained, same⟩ := List.mem_map.mp present
      exact ⟨representative, retained, same.symm⟩
    simp only [retain, present, ↓reduceIte]
    constructor
    · intro representative retained
      exact List.mem_append_left _ (coverage.1 representative retained)
    · intro answer consumed
      rcases List.mem_append.mp consumed with old | new
      · exact coverage.2 answer old
      · have same : answer = model := List.mem_singleton.mp new
        simpa only [same] using witness
  · simp only [retain, present, ↓reduceIte]
    constructor
    · intro representative retained
      rcases List.mem_append.mp retained with old | new
      · exact List.mem_append_left _ (coverage.1 representative old)
      · exact List.mem_append_right _ new
    · intro answer consumed
      rcases List.mem_append.mp consumed with old | new
      · obtain ⟨representative, retained, same⟩ := coverage.2 answer old
        exact ⟨representative, List.mem_append_left _ retained, same⟩
      · have same : answer = model := List.mem_singleton.mp new
        exact ⟨model, List.mem_append_right _ (List.mem_singleton_self _), congrArg key same⟩

/-- Complete identity decisions never retain two representatives with the same
    key. Only the absent-key branch extends the already unique history. -/
theorem retain_unique [DecidableEq K] (key : M → K) (representatives : List M)
    (model : M) (unique : (representatives.map key).Nodup) :
    ((retain key representatives model).map key).Nodup := by
  by_cases present : key model ∈ representatives.map key
  · simpa only [retain, present, ↓reduceIte] using unique
  · have fresh : ∀ representative ∈ representatives, key representative ≠ key model := by
      intro representative retained same
      exact present (List.mem_map.mpr ⟨representative, retained, same⟩)
    simpa [retain, present, List.nodup_append, unique] using fresh

/-- Key agreement is reflexive, symmetric and transitive for any fixed domain. -/
theorem same_equivalence (key : M → K) : Equivalence (Same key) := by
  constructor
  · intro model
    exact Eq.refl (key model)
  · intro first second same
    exact same.symm
  · intro first middle last left right
    exact left.trans right

/-- Quotient coverage gives exactly the original selected key image. It makes no
    claim that the retained original models form the entire original family. -/
theorem covered_key_image (key : M → K) (selected representatives : List M)
    (coverage : Covers key selected representatives) (value : K) :
    value ∈ representatives.map key ↔ value ∈ selected.map key := by
  have sound : value ∈ representatives.map key → value ∈ selected.map key := by
    rintro present
    obtain ⟨representative, retained, same⟩ := List.mem_map.mp present
    exact List.mem_map.mpr ⟨representative, coverage.1 representative retained, same⟩
  have complete : value ∈ selected.map key → value ∈ representatives.map key := by
    intro present
    obtain ⟨model, selected_model, same⟩ := List.mem_map.mp present
    obtain ⟨representative, retained, represented⟩ := coverage.2 model selected_model
    exact List.mem_map.mpr ⟨representative, retained, represented.symm.trans same⟩
  exact ⟨sound, complete⟩

/-- Stability, optimality, or any other established selection predicate survives
    projection because each representative remains a selected original model. -/
theorem selected_property_survives (key : M → K) (selected representatives : List M)
    (property : M → Prop) (original : ∀ model ∈ selected, property model)
    (coverage : Covers key selected representatives) :
    ∀ representative ∈ representatives, property representative := by
  intro representative retained
  exact original representative (coverage.1 representative retained)

/-- Selecting one arbitrary key representative before minimizing can discard the
    better-scored original. Equal keys alone do not establish equal costs. -/
theorem projection_before_selection_can_lose_optimum :
    let answers : List Nat := [1, 0]
    let key : Nat → Nat := fun _ => 0
    Covers key answers [1] ∧ 0 ∈ answers ∧ ¬ 0 ∈ [1] := by
  simp [Covers, Same]

end Zetesis.ProjectedAnswers
