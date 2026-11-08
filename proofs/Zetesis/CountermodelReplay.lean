import Zetesis.CountermodelReuse

/-!
# Replaying a proper-subset witness using selected frozen nodes

A finite justification selects formulas whose original truth the next candidate must
preserve. At the fixed witness, a true disjunction needs just one true branch, and
a false one needs both false branches. Conjunction uses the dual choice. An
implication also needs its own original truth, because a false root freezes to
falsum irrespective of what its children would say at the witness.

The selection is a mathematical list of formula occurrences with their signs read
in the original candidate. Shared DAG identities, deduplication, authenticated
row construction and resource bounds belong to the implementation.
-/

namespace Zetesis.LearnedRegions

open Ferraris
universe u
variable {α : Type u}

/-- Agreement at the selected occurrences is about original satisfaction in the
two candidates. It does not require either candidate to agree with the witness. -/
def NodeAgreement (M M' : Atoms α) (selected : List (Formula α)) : Prop :=
  ∀ F ∈ selected, Satisfies M F ↔ Satisfies M' F

open Classical in
/-- For a disjunction either true input suffices. If both inputs are false,
both explanations must survive. The left branch wins a tie. -/
noncomputable def alternativeNodes (P Q : Prop) (left right : List (Formula α)) :
    List (Formula α) :=
  if P then left else if Q then right else left ++ right

open Classical in
/-- A finite selection sufficient to preserve the truth of the frozen formula at N.
The valuations are mathematical propositions, so branch selection uses classical
decidability. An atom needs no mask check when both candidates contain N. -/
noncomputable def replayNodes (M N : Atoms α) : Formula α → List (Formula α)
  | .atom _ | .bot => []
  | .conj F G => alternativeNodes (¬ Satisfies N (Reduct M F)) (¬ Satisfies N (Reduct M G))
      (replayNodes M N F) (replayNodes M N G)
  | .disj F G => alternativeNodes (Satisfies N (Reduct M F)) (Satisfies N (Reduct M G))
      (replayNodes M N F) (replayNodes M N G)
  | .imp F G =>
      let root := Formula.imp F G
      if Satisfies M root then
        root :: alternativeNodes (¬ Satisfies N (Reduct M F)) (Satisfies N (Reduct M G))
          (replayNodes M N F) (replayNodes M N G)
      else [root]

/-- Replaying the selected alternatives preserves their disjunction. If the
original disjunction was false, agreement applies to both lists of conditions. -/
theorem alternative_nodes_agree {M M' : Atoms α} {P Q P' Q' : Prop}
    {left right : List (Formula α)}
    (leftReplay : NodeAgreement M M' left → (P ↔ P'))
    (rightReplay : NodeAgreement M M' right → (Q ↔ Q'))
    (agree : NodeAgreement M M' (alternativeNodes P Q left right)) : (P ∨ Q) ↔ (P' ∨ Q') := by
  classical
  by_cases first : P
  · have next : P' := (leftReplay (by simpa [alternativeNodes, first] using agree)).mp first
    simp [first, next]
  by_cases second : Q
  · have next : Q' :=
      (rightReplay (by simpa [alternativeNodes, first, second] using agree)).mp second
    simp [second, next]
  have both : NodeAgreement M M' (left ++ right) := by
    simpa [alternativeNodes, first, second] using agree
  have l := leftReplay (fun F h => both F (List.mem_append_left _ h))
  have r := rightReplay (fun F h => both F (List.mem_append_right _ h))
  exact or_congr l r

/-- Agreement on the selected nodes preserves both readings of a nested formula.
The induction uses the reduct factorization of each connective. At an implication
the selected parent's truth is replayed before interpreting its branches. -/
theorem replay_nodes_agree {M M' N : Atoms α} (oldSubset : Sub N M) (newSubset : Sub N M')
    (F : Formula α) : NodeAgreement M M' (replayNodes M N F) →
      (Satisfies N (Reduct M F) ↔ Satisfies N (Reduct M' F)) := by
  classical
  induction F with
  | atom a =>
    intro _
    rw [atom_reduct, atom_reduct]
    exact ⟨fun h => ⟨newSubset a h.2, h.2⟩, fun h => ⟨oldSubset a h.2, h.2⟩⟩
  | bot => exact fun _ => Iff.rfl
  | conj F G left right =>
    intro agree
    have failing := alternative_nodes_agree (fun h => not_congr (left h))
      (fun h => not_congr (right h)) agree
    rw [RuleFactorization.reduct_conj, RuleFactorization.reduct_conj]
    simpa only [not_or, Classical.not_not] using not_congr failing
  | disj F G left right =>
    intro agree
    rw [RuleFactorization.reduct_disj, RuleFactorization.reduct_disj]
    exact alternative_nodes_agree left right agree
  | imp F G left right =>
    intro agree
    by_cases original : Satisfies M (.imp F G)
    · have selected : NodeAgreement M M' (.imp F G ::
          alternativeNodes (¬ Satisfies N (Reduct M F)) (Satisfies N (Reduct M G))
            (replayNodes M N F) (replayNodes M N G)) := by
        simpa only [replayNodes, if_pos original] using agree
      have active := (selected _ List.mem_cons_self).mp original
      have branches := alternative_nodes_agree (fun h => not_congr (left h)) right
        (fun node member => selected node (List.mem_cons_of_mem _ member))
      rw [RuleFactorization.reduct_imp, RuleFactorization.reduct_imp]
      simpa only [original, active, true_and, Classical.or_iff_not_imp_left, Classical.not_not] using branches
    · have selected : NodeAgreement M M' [.imp F G] := by
        simpa only [replayNodes, if_neg original] using agree
      have masked : ¬ Satisfies M' (.imp F G) := fun h =>
        original ((selected _ List.mem_cons_self).mpr h)
      simp only [RuleFactorization.reduct_imp, original, masked, false_and]

/-- A checked proper-subset witness can refute a new candidate whose region holds
N and any extra atom. Agreement is required only on the selected formulas of each
original root. The new candidate need not contain the old candidate. -/
theorem countermodel_row_refutes {T : Theory α} {N M M' : Atoms α}
    (witness : ReductRegions.Countermodel M T N)
    (agree : ∀ F ∈ T, NodeAgreement M M' (replayNodes M N F))
    (contained : Sub N M') {a : α} (extra : M' a) (outside : ¬ N a) : ¬ Stable M' T := by
  have strict : ProperSub N M' := ⟨contained, fun back => outside (back a extra)⟩
  have replayed : Models N (ReductTheory M' T) := by
    intro reduced member
    obtain ⟨F, inTheory, rfl⟩ := List.mem_map.mp member
    have original := witness.2 _ (List.mem_map.mpr ⟨F, inTheory, rfl⟩)
    exact (replay_nodes_agree witness.1.1 contained F (agree F inTheory)).mp original
  exact ReductRegions.leaf_refutes M' N T strict replayed

end Zetesis.LearnedRegions
