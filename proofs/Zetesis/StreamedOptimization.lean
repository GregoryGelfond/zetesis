import Zetesis.StreamedConstraints
import Zetesis.Optimization

/-!
# Ranking answer sets after completed constraint checks

A finite list covers the answer sets of a retained theory. Fully checked
partitions of an additional constraint family filter that list before its
minimum and ties are selected. The result contains exactly the optima of the
full theory, provided the partitions cover the source occurrences and each
Boolean body test agrees with original truth on the listed interpretations.

This composition uses one unbounded integer cost. It does not establish source
lowering, Rust acceptance and incumbent ordering, priority-vector scoring,
checked arithmetic, dynamic bound coverage, or completion after an interrupted
check. Those remain separate premises or implementation obligations.
-/

namespace Zetesis.StreamedOptimization

open Ferraris StreamedConstraints

universe u v
variable {ι : Type u} {A : Type v}

/-- Checking every covered constraint part before ranking a complete retained
answer-set list yields exactly the full theory's optimal answer sets. Merely
being an answer set of the retained theory does not authorize ranking: a
constraint-violating interpretation is removed before `bestTies` reads its cost.
The conclusion concerns membership; duplicate input entries are not removed.

Proof outline: identify the filter's true branch with completed part scans.
The streamed constraint law establishes soundness of every surviving answer.
Conversely, an answer of the full theory is an answer of the retained theory,
so list coverage supplies it and its completed scans keep it in the filter.
Apply the existing exact-ties law to this sound, complete filtered list. -/
theorem completed_ties_exact (P : Theory A) (source : List ι)
    (body : ι → Formula A) (test : Atoms A → ι → Bool)
    (parts : List (List ι)) (coverage : parts.flatten.Perm source)
    (models : List (Atoms A))
    (sound : ∀ N ∈ models, Stable N P)
    (complete : ∀ N, Stable N P → N ∈ models)
    (evaluation : ∀ N ∈ models, ∀ occurrence ∈ source,
      test N occurrence = true ↔ Satisfies N (body occurrence))
    (cost : Atoms A → Int) (M : Atoms A) :
    M ∈ Optimization.bestTies cost (models.filter (fun N =>
      parts.all (fun part =>
        match scan (test N) (part.length + 1) part with
        | .complete => true
        | _ => false))) ↔
      Optimization.Optimal (P ++ (source.map body).map Ferraris.Neg) cost M := by
  let selected := models.filter (fun N => parts.all (fun part =>
    match scan (test N) (part.length + 1) part with
    | .complete => true
    | _ => false))
  have scan_selected (N : Atoms A) (part : List ι) :
      (match scan (test N) (part.length + 1) part with
        | .complete => true
        | _ => false) = true ↔
      scan (test N) (part.length + 1) part = .complete := by
    cases scan (test N) (part.length + 1) part <;> simp
  have selected_iff (N : Atoms A) :
      N ∈ selected ↔ N ∈ models ∧
        ∀ part ∈ parts, scan (test N) (part.length + 1) part = .complete := by
    simp only [selected, List.mem_filter, List.all_eq_true, scan_selected]
  have selected_sound :
      ∀ N ∈ selected, Stable N (P ++ (source.map body).map Ferraris.Neg) := by
    intro N present
    obtain ⟨listed, checked⟩ := (selected_iff N).mp present
    exact (stable_iff_completed_partition P source body N (test N)
      (evaluation N listed) parts coverage).mpr ⟨sound N listed, checked⟩
  have selected_complete :
      ∀ N, Stable N (P ++ (source.map body).map Ferraris.Neg) → N ∈ selected := by
    intro N stable
    have retained : Stable N P :=
      ((ConstrainedPositive.stable_append_constraints P (source.map body) N).mp stable).1
    have listed : N ∈ models := complete N retained
    have checked :
        ∀ part ∈ parts, scan (test N) (part.length + 1) part = .complete :=
      ((stable_iff_completed_partition P source body N (test N)
        (evaluation N listed) parts coverage).mp stable).2
    exact (selected_iff N).mpr ⟨listed, checked⟩
  exact Optimization.completed_ties_exact
    (P ++ (source.map body).map Ferraris.Neg) cost selected
    selected_sound selected_complete M

end Zetesis.StreamedOptimization
