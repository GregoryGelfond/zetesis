import Zetesis.Search

/-!
# The frontier of a coverage tree walked by several workers

A coverage tree is walked by keeping a frontier: the regions reached and not
yet visited, each with the subtree that covers it, as a worker's stack and the
shared pool hold them under `--workers`. A step takes any pending region,
whichever worker holds it, and does what its subtree says: a refuted region is
dropped, an accepted leaf is emitted, a split region is replaced by its two
children, a narrowed region by its child. Steps interleave in any order, and a
region moving between a stack and the pool changes nothing here, since the
frontier is one list whoever holds its members. The law is that the leaves
emitted and the leaves still pending are together a permutation of the tree's
outputs after every step, so a walk that empties the frontier has emitted every
accepted leaf exactly once, in the schedule's order: the family is exact at any
worker count, and the order is not a property of the result.
-/
namespace Zetesis

universe u

/-- A region reached and not yet visited, with the subtree that covers it. -/
structure Pending {α : Type u} (valid : Atoms α → Prop) where
  cube : Cube α
  tree : CoverageTree valid cube

namespace Pending

variable {α : Type u} {valid : Atoms α → Prop}

/-- The accepted leaves the pending regions still hold, in frontier order. -/
def outputs (frontier : List (Pending valid)) : List (Atoms α) :=
  frontier.flatMap (fun pending => pending.tree.outputs)

theorem outputs_append (left right : List (Pending valid)) :
    outputs (left ++ right) = outputs left ++ outputs right :=
  List.flatMap_append

theorem outputs_cons (pending : Pending valid) (rest : List (Pending valid)) :
    outputs (pending :: rest) = pending.tree.outputs ++ outputs rest :=
  List.flatMap_cons

/-- One step of one worker on one pending region, at any position of the
frontier, with the leaves emitted so far. -/
inductive Step :
    List (Pending valid) → List (Atoms α) → List (Pending valid) → List (Atoms α) → Prop
  | refuted (before after : List (Pending valid)) (emitted : List (Atoms α))
      (c : Cube α) (sound : ∀ z, c.Contains z → ¬ valid z) :
      Step (before ++ ⟨c, .refuted c sound⟩ :: after) emitted (before ++ after) emitted
  | accepted (before after : List (Pending valid)) (emitted : List (Atoms α))
      (z : Atoms α) (sound : valid z) :
      Step (before ++ ⟨Cube.singleton z, .accepted z sound⟩ :: after) emitted
        (before ++ after) (emitted ++ [z])
  | split (before after : List (Pending valid)) (emitted : List (Atoms α))
      (c : Cube α) (a : α) (fresh : c.Fresh a)
      (left : CoverageTree valid (c.splitFalse a))
      (right : CoverageTree valid (c.splitTrue a)) :
      Step (before ++ ⟨c, .split c a fresh left right⟩ :: after) emitted
        (before ++ ⟨c.splitFalse a, left⟩ :: ⟨c.splitTrue a, right⟩ :: after) emitted
  | narrowed (before after : List (Pending valid)) (emitted : List (Atoms α))
      (c : Cube α) (lo hi : Atoms α)
      (forcedLower : ∀ z, c.Contains z → valid z → Sub lo z)
      (forcedUpper : ∀ z, c.Contains z → valid z → Sub z hi)
      (child : CoverageTree valid (c.narrow lo hi)) :
      Step (before ++ ⟨c, .narrowed c lo hi forcedLower forcedUpper child⟩ :: after) emitted
        (before ++ ⟨c.narrow lo hi, child⟩ :: after) emitted

/-- A step keeps the leaves emitted and the leaves pending, together, a
permutation of what they were: a dropped region held none, an emitted leaf
moves from pending to emitted, and children hold exactly their parent's. -/
theorem Step.perm {frontier frontier' : List (Pending valid)}
    {emitted emitted' : List (Atoms α)}
    (step : Step frontier emitted frontier' emitted') :
    (emitted ++ outputs frontier).Perm (emitted' ++ outputs frontier') := by
  cases step with
  | refuted before after emitted c sound =>
    simp only [outputs_append, outputs_cons, CoverageTree.outputs, List.nil_append]
    exact List.Perm.refl _
  | accepted before after emitted z sound =>
    simp only [outputs_append, outputs_cons, CoverageTree.outputs, List.singleton_append]
    have moved : (outputs before ++ z :: outputs after).Perm (z :: (outputs before ++ outputs after)) :=
      List.perm_middle
    have front : emitted ++ [z] ++ (outputs before ++ outputs after)
        = emitted ++ z :: (outputs before ++ outputs after) := by
      simp only [List.append_assoc, List.singleton_append]
    rw [front]
    exact List.Perm.append_left emitted moved
  | split before after emitted c a fresh left right =>
    simp only [outputs_append, outputs_cons, CoverageTree.outputs, List.append_assoc]
    exact List.Perm.refl _
  | narrowed before after emitted c lo hi forcedLower forcedUpper child =>
    simp only [outputs_append, outputs_cons, CoverageTree.outputs]
    exact List.Perm.refl _

/-- Any number of steps, in any interleaving of workers. -/
inductive Walk :
    List (Pending valid) → List (Atoms α) → List (Pending valid) → List (Atoms α) → Prop
  | done (frontier : List (Pending valid)) (emitted : List (Atoms α)) :
      Walk frontier emitted frontier emitted
  | step {frontier emitted frontier' emitted' frontier'' emitted''}
      (first : Step frontier emitted frontier' emitted')
      (rest : Walk frontier' emitted' frontier'' emitted'') :
      Walk frontier emitted frontier'' emitted''

/-- The permutation law holds along a whole walk. -/
theorem Walk.perm {frontier frontier' : List (Pending valid)}
    {emitted emitted' : List (Atoms α)}
    (walk : Walk frontier emitted frontier' emitted') :
    (emitted ++ outputs frontier).Perm (emitted' ++ outputs frontier') := by
  induction walk with
  | done _ _ => exact List.Perm.refl _
  | step first _ ih => exact first.perm.trans ih

/-- A walk from the root that empties the frontier has emitted the tree's
accepted leaves in some order, each once: exactly the valid seeds of the root
region, by `mem_outputs_iff` and `outputs_nodup`. The order is the walk's. -/
theorem Walk.exhausted {c : Cube α} (tree : CoverageTree valid c)
    {emitted : List (Atoms α)} (walk : Walk [⟨c, tree⟩] [] [] emitted) :
    emitted.Perm tree.outputs ∧ (∀ z, z ∈ emitted ↔ c.Contains z ∧ valid z) ∧ emitted.Nodup := by
  have law : ([] ++ outputs [⟨c, tree⟩]).Perm (emitted ++ outputs []) := walk.perm
  simp only [outputs, List.flatMap_cons, List.flatMap_nil, List.append_nil,
    List.nil_append] at law
  refine ⟨law.symm, fun z => ?_, ?_⟩
  · exact (law.symm.mem_iff).trans (tree.mem_outputs_iff z)
  · exact law.nodup_iff.mp tree.outputs_nodup

end Pending

end Zetesis
