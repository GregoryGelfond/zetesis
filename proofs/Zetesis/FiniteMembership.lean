import Zetesis.ReductEvaluation

/-!
# Finite interpretations and reduct membership

Finite selections enumerate every subinterpretation of a supplied atom list.
The ambient atom type need not be finite. Repeated atom occurrences may repeat
selections; they do not change the family of represented interpretations.

The finite membership checker first establishes original satisfaction and then
searches selected proper subsets against the computed frozen reduct. Its
correctness proof establishes subset coverage from the selection algorithm;
it does not assume a complete proposal source or a correct membership oracle.
The reference algorithm is exponential, and list occurrences can repeat work.

This is an executable mathematical construction. It does not verify a Rust
candidate counter, memory allocation, resource interruption or source grounding.
-/

namespace Zetesis.FiniteMembership

universe u
variable {α : Type u}

/-- Select or omit each occurrence, retaining its original order. -/
def selections : List α → List (List α)
  | [] => [[]]
  | atom :: rest => selections rest ++ (selections rest).map (atom :: ·)

/-- A selected list contains only atoms from the supplied list. -/
theorem selection_subset (atoms chosen : List α)
    (selected : chosen ∈ selections atoms) :
    ∀ atom ∈ chosen, atom ∈ atoms := by
  induction atoms generalizing chosen with
  | nil =>
    simp only [selections, List.mem_singleton] at selected
    subst chosen
    simp
  | cons head tail inductionHypothesis =>
    simp only [selections, List.mem_append, List.mem_map] at selected
    rcases selected with omitted | ⟨suffix, selectedSuffix, rfl⟩
    · intro atom member
      exact List.mem_cons_of_mem head (inductionHypothesis chosen omitted atom member)
    · intro atom member
      rcases List.mem_cons.mp member with rfl | member
      · exact List.mem_cons_self
      · exact List.mem_cons_of_mem head
          (inductionHypothesis suffix selectedSuffix atom member)

/-- Filtering by any decidable predicate produces one enumerated selection. -/
theorem filter_selected (keep : α → Bool) (atoms : List α) :
    atoms.filter keep ∈ selections atoms := by
  induction atoms with
  | nil => simp [selections]
  | cons head tail inductionHypothesis =>
    cases enabled : keep head <;>
      simp [List.filter, enabled, selections, inductionHypothesis]

/-- Membership in a finite list, viewed as a Boolean interpretation. -/
def candidate [DecidableEq α] (atoms : List α) (atom : α) : Bool :=
  decide (atom ∈ atoms)

/-- The candidate represents exactly the supplied atoms. -/
theorem candidate_member [DecidableEq α] (atoms : List α) (atom : α) :
    TightEvaluation.interpretation (candidate atoms) atom ↔ atom ∈ atoms := by
  simp [candidate, TightEvaluation.interpretation]

/-- Every semantic subinterpretation has an enumerated finite representative.

Filter the supplied atoms by the desired interpretation. The filter is an
enumerated selection. The subset premise establishes that it loses no true
atom; membership of the filtered list establishes that it adds none.
The proof may use classical decidability of the supplied predicate. The
enumeration algorithm itself uses no such oracle.
-/
theorem selections_cover [DecidableEq α] (atoms : List α) (J : Atoms α)
    (contained : Sub J (TightEvaluation.interpretation (candidate atoms))) :
    ∃ chosen ∈ selections atoms,
      TightEvaluation.interpretation (candidate chosen) = J := by
  classical
  let chosen := atoms.filter (fun atom => decide (J atom))
  have selected : chosen ∈ selections atoms := filter_selected _ atoms
  have exactInterpretation : TightEvaluation.interpretation (candidate chosen) = J := by
    apply atoms_ext
    intro atom
    rw [candidate_member]
    change atom ∈ atoms.filter (fun item => decide (J item)) ↔ J atom
    simp only [List.mem_filter, decide_eq_true_eq]
    constructor
    · exact And.right
    · intro present
      exact ⟨(candidate_member atoms atom).mp (contained atom present), present⟩
  exact ⟨chosen, selected, exactInterpretation⟩

/-- Test whether the selected subinterpretation omits an original atom. -/
def proper [DecidableEq α] (atoms chosen : List α) : Bool :=
  !(atoms.all (candidate chosen))

/-- The finite all-atoms test expresses inclusion of the represented sets. -/
theorem all_members_iff [DecidableEq α] (atoms chosen : List α) :
    atoms.all (candidate chosen) = true ↔
      Sub (TightEvaluation.interpretation (candidate atoms))
        (TightEvaluation.interpretation (candidate chosen)) := by
  simp [List.all_eq_true, candidate, TightEvaluation.interpretation, Sub]

/-- On enumerated selections, omission is exactly proper semantic inclusion. -/
theorem proper_iff [DecidableEq α] (atoms chosen : List α)
    (selected : chosen ∈ selections atoms) :
    proper atoms chosen = true ↔
      Ferraris.ProperSub (TightEvaluation.interpretation (candidate chosen))
        (TightEvaluation.interpretation (candidate atoms)) := by
  have subset : Sub (TightEvaluation.interpretation (candidate chosen))
      (TightEvaluation.interpretation (candidate atoms)) := by
    intro atom present
    exact (candidate_member atoms atom).mpr
      (selection_subset atoms chosen selected atom ((candidate_member chosen atom).mp present))
  simp only [proper, Ferraris.ProperSub, subset, true_and]
  rw [← all_members_iff]
  cases atoms.all (candidate chosen) <;> simp

/-- Search the generated proper subsets for a model of the frozen reduct.
Every selection is interpreted as a set, so repeated atom occurrences do not
create a false witness of proper inclusion. -/
def hasCountermodel [DecidableEq α] (atoms : List α)
    (table : List (DagSharing.Node α)) (roots : List Nat) : Bool :=
  (selections atoms).any (fun chosen => proper atoms chosen &&
    TightEvaluation.rootsTrue (ReductEvaluation.values (candidate atoms) (candidate chosen) table) roots)

/-- A successful finite search is exactly a proper-subset model of the reduct.

Soundness reads the returned selection and uses the computed reduct evaluator.
Completeness represents any semantic countermodel by a selected list, then
uses exact properness and reduct evaluation to show that the search finds it.
No finite universe beyond the supplied candidate is needed.
-/
theorem has_countermodel_iff [DecidableEq α] (atoms : List α)
    (table : List (DagSharing.Node α)) (roots : List Nat) :
    hasCountermodel atoms table roots = true ↔
      ∃ J, Ferraris.ProperSub J (TightEvaluation.interpretation (candidate atoms)) ∧
        Ferraris.Models J (Ferraris.ReductTheory
          (TightEvaluation.interpretation (candidate atoms)) (DagSharing.assertions table roots)) := by
  simp only [hasCountermodel, List.any_eq_true, Bool.and_eq_true]
  constructor
  · rintro ⟨chosen, selected, omitted, modeled⟩
    have properSubset := (proper_iff atoms chosen selected).mp omitted
    have reductModel := (ReductEvaluation.roots_true_iff
      (candidate atoms) (candidate chosen) table roots).mp modeled
    exact ⟨TightEvaluation.interpretation (candidate chosen), properSubset, reductModel⟩
  · rintro ⟨J, properSubset, reductModel⟩
    obtain ⟨chosen, selected, represents⟩ := selections_cover atoms J properSubset.1
    have omitted : proper atoms chosen = true := by
      apply (proper_iff atoms chosen selected).mpr
      rw [represents]
      exact properSubset
    have modeled : TightEvaluation.rootsTrue
        (ReductEvaluation.values (candidate atoms) (candidate chosen) table) roots = true := by
      apply (ReductEvaluation.roots_true_iff (candidate atoms) (candidate chosen) table roots).mpr
      rw [represents]
      exact reductModel
    exact ⟨chosen, selected, omitted, modeled⟩

/-- Decide answer-set membership by original satisfaction and reduct minimality.
This total finite reference computation has no resource-interruption channel.
An implementation imposing a budget must represent incomplete work separately.
-/
def check [DecidableEq α] (atoms : List α)
    (table : List (DagSharing.Node α)) (roots : List Nat) : Bool :=
  TightEvaluation.rootsTrue (TightEvaluation.values (candidate atoms) table) roots &&
    !hasCountermodel atoms table roots

/-- The executable finite checker accepts exactly the answer sets of the
denoted formula theory.

Original satisfaction follows from the ordinary DAG fold. The finite subset
search detects exactly the proper-subset reduct models by
`has_countermodel_iff`. Their absence is the independent Ferraris definition.
Neither evaluator agreement nor candidate coverage is an assumption.
-/
theorem check_iff_answer_set [DecidableEq α] (atoms : List α)
    (table : List (DagSharing.Node α)) (roots : List Nat) :
    check atoms table roots = true ↔
      Ferraris.Stable (TightEvaluation.interpretation (candidate atoms))
        (DagSharing.assertions table roots) := by
  have original :
      TightEvaluation.rootsTrue (TightEvaluation.values (candidate atoms) table) roots = true ↔
        Ferraris.Models (TightEvaluation.interpretation (candidate atoms))
          (DagSharing.assertions table roots) :=
    TightEvaluation.roots_true_iff (candidate atoms) table roots
  have countermodels : hasCountermodel atoms table roots = true ↔
      ∃ J, Ferraris.ProperSub J (TightEvaluation.interpretation (candidate atoms)) ∧
        Ferraris.Models J (Ferraris.ReductTheory
          (TightEvaluation.interpretation (candidate atoms)) (DagSharing.assertions table roots)) :=
    has_countermodel_iff atoms table roots
  simp only [check, Bool.and_eq_true, Ferraris.Stable]
  rw [original, ← countermodels]
  cases hasCountermodel atoms table roots <;> simp

end Zetesis.FiniteMembership
