import Zetesis.Core

/-!
# Region geometry and finite search coverage

These definitions are independent of the reduct implementation. `valid` is the
exact seed-acceptance predicate supplied by the semantic kernel. A completed
coverage tree carries local evidence for each refutation or bound; it is not an
algorithm that manufactures such evidence. Open, interrupted, or merely sampled
regions have no constructor that declares them complete.
-/
namespace Zetesis

universe u

structure Cube (α : Type u) where
  lower : Atoms α
  upper : Atoms α

namespace Cube

variable {α : Type u}

def Contains (c : Cube α) (z : Atoms α) : Prop :=
  Sub c.lower z ∧ Sub z c.upper

def singleton (z : Atoms α) : Cube α := ⟨z, z⟩

def splitFalse (c : Cube α) (a : α) : Cube α :=
  ⟨c.lower, fun b => c.upper b ∧ b ≠ a⟩

def splitTrue (c : Cube α) (a : α) : Cube α :=
  ⟨fun b => c.lower b ∨ b = a, c.upper⟩

def Fresh (c : Cube α) (a : α) : Prop := ¬ c.lower a ∧ c.upper a

def narrow (c : Cube α) (lo hi : Atoms α) : Cube α :=
  ⟨Union c.lower lo, Inter c.upper hi⟩

theorem contains_singleton {z w : Atoms α} :
    (singleton z).Contains w ↔ w = z := by
  constructor
  · intro h
    exact sub_antisymm h.2 h.1
  · intro h
    subst w
    exact ⟨sub_refl z, sub_refl z⟩

theorem contains_splitFalse {c : Cube α} {a : α} {z : Atoms α} :
    (c.splitFalse a).Contains z ↔ c.Contains z ∧ ¬ z a := by
  constructor
  · intro h
    exact ⟨⟨h.1, fun b hb => (h.2 b hb).1⟩,
      fun ha => (h.2 a ha).2 rfl⟩
  · intro h
    refine ⟨h.1.1, ?_⟩
    intro b hb
    refine ⟨h.1.2 b hb, ?_⟩
    intro heq
    subst b
    exact h.2 hb

theorem contains_splitTrue {c : Cube α} {a : α} {z : Atoms α} :
    (c.splitTrue a).Contains z ↔ c.Contains z ∧ z a := by
  constructor
  · intro h
    exact ⟨⟨fun b hb => h.1 b (Or.inl hb), h.2⟩,
      h.1 a (Or.inr rfl)⟩
  · intro h
    refine ⟨?_, h.1.2⟩
    intro b hb
    cases hb with
    | inl hlow => exact h.1.1 b hlow
    | inr heq => simpa [heq] using h.2

/-- The two decision branches cover the parent, even when one is empty. -/
theorem split_partition (c : Cube α) (a : α) (z : Atoms α) :
    c.Contains z ↔
      (c.splitFalse a).Contains z ∨ (c.splitTrue a).Contains z := by
  classical
  rw [contains_splitFalse, contains_splitTrue]
  constructor
  · intro h
    by_cases hz : z a
    · exact Or.inr ⟨h, hz⟩
    · exact Or.inl ⟨h, hz⟩
  · intro h
    cases h with
    | inl hf => exact hf.1
    | inr ht => exact ht.1

theorem split_disjoint (c : Cube α) (a : α) (z : Atoms α) :
    ¬ ((c.splitFalse a).Contains z ∧ (c.splitTrue a).Contains z) := by
  intro h
  exact (contains_splitFalse.mp h.1).2 (contains_splitTrue.mp h.2).2

/-- A fresh decision is a proper refinement on both sides. -/
theorem fresh_branches (c : Cube α) (a : α) (h : c.Fresh a) :
    (c.splitFalse a).upper ≠ c.upper ∧ (c.splitTrue a).lower ≠ c.lower := by
  constructor
  · intro heq
    have ha : (c.splitFalse a).upper a := heq ▸ h.2
    exact ha.2 rfl
  · intro heq
    have ha : (c.splitTrue a).lower a := Or.inr rfl
    rw [heq] at ha
    exact h.1 ha

theorem contains_narrow {c : Cube α} {lo hi z : Atoms α} :
    (c.narrow lo hi).Contains z ↔
      c.Contains z ∧ Sub lo z ∧ Sub z hi := by
  constructor
  · intro h
    exact ⟨⟨fun a ha => h.1 a (Or.inl ha),
      fun a ha => (h.2 a ha).1⟩,
      (fun a ha => h.1 a (Or.inr ha)),
      (fun a ha => (h.2 a ha).2)⟩
  · intro h
    refine ⟨?_, fun a ha => ⟨h.1.2 a ha, h.2.2 a ha⟩⟩
    intro a ha
    cases ha with
    | inl hc => exact h.1.1 a hc
    | inr hlo => exact h.2.1 a hlo

theorem narrow_preserves_valid (valid : Atoms α → Prop)
    (c : Cube α) (lo hi : Atoms α)
    (forcedLower : ∀ z, c.Contains z → valid z → Sub lo z)
    (forcedUpper : ∀ z, c.Contains z → valid z → Sub z hi)
    {z : Atoms α} (hz : c.Contains z) (hv : valid z) :
    (c.narrow lo hi).Contains z :=
  contains_narrow.mpr ⟨hz, forcedLower z hz hv, forcedUpper z hz hv⟩

/--
Exact seed agreement on S and sound consequence enclosures justify the
specification's projected lower/upper narrowing. Carrier support is explicit:
agreement on S alone says nothing about candidate atoms outside S.
-/
theorem consequence_narrow_preserves
    (valid : Atoms α → Prop) (gamma : Atoms α → Atoms α)
    (c : Cube α) (S dlo dhi : Atoms α)
    (seedSupported : ∀ z, valid z → Sub z S)
    (seedAgrees : ∀ z, valid z → EqOn S z (gamma z))
    (lowerEnclosure : ∀ z, c.Contains z → Sub dlo (gamma z))
    (upperEnclosure : ∀ z, c.Contains z → Sub (gamma z) dhi)
    {z : Atoms α} (hz : c.Contains z) (hv : valid z) :
    (c.narrow (Inter dlo S) (Inter dhi S)).Contains z := by
  apply contains_narrow.mpr
  refine ⟨hz, ?_, ?_⟩
  · intro a ha
    exact (seedAgrees z hv a ha.2).mpr (lowerEnclosure z hz a ha.1)
  · intro a ha
    have hs := seedSupported z hv a ha
    exact ⟨upperEnclosure z hz a ((seedAgrees z hv a hs).mp ha), hs⟩

end Cube

/-!
The following finite tree is a proof-carrying completion ledger. Refutation
leaves require a sound region refutation, not a rejected sample. Narrowing
nodes require forced truth/falsity bounds, not a partial upper closure. These
are the local obligations a concrete checker must discharge. The coverage
theorems below derive their global consequences structurally.
-/

inductive CoverageTree {α : Type u} (valid : Atoms α → Prop) : Cube α → Type u
  | refuted (c : Cube α)
      (sound : ∀ z, c.Contains z → ¬ valid z) : CoverageTree valid c
  | accepted (z : Atoms α) (sound : valid z) :
      CoverageTree valid (Cube.singleton z)
  | split (c : Cube α) (a : α) (fresh : c.Fresh a)
      (left : CoverageTree valid (c.splitFalse a))
      (right : CoverageTree valid (c.splitTrue a)) : CoverageTree valid c
  | narrowed (c : Cube α) (lo hi : Atoms α)
      (forcedLower : ∀ z, c.Contains z → valid z → Sub lo z)
      (forcedUpper : ∀ z, c.Contains z → valid z → Sub z hi)
      (child : CoverageTree valid (c.narrow lo hi)) : CoverageTree valid c

namespace CoverageTree

variable {α : Type u} {valid : Atoms α → Prop}

/-- The finite collection of accepted singleton leaves. -/
def outputs {c : Cube α} (tree : CoverageTree valid c) : List (Atoms α) :=
  match tree with
  | .refuted _ _ => []
  | .accepted z _ => [z]
  | .split _ _ _ left right => left.outputs ++ right.outputs
  | .narrowed _ _ _ _ _ child => child.outputs

/-- Exact equality between accepted leaves and valid seeds in the root region. -/
theorem mem_outputs_iff {c : Cube α} (tree : CoverageTree valid c)
    (z : Atoms α) : z ∈ tree.outputs ↔ c.Contains z ∧ valid z := by
  induction tree with
  | refuted c sound =>
    simp only [outputs, List.not_mem_nil, false_iff, not_and]
    exact sound z
  | accepted w sound =>
    simp only [outputs, List.mem_cons, List.not_mem_nil, or_false,
      Cube.contains_singleton]
    constructor
    · intro h
      exact ⟨h, h ▸ sound⟩
    · intro h
      exact h.1
  | split c a fresh left right ihLeft ihRight =>
    simp only [outputs, List.mem_append, ihLeft, ihRight]
    constructor
    · intro h
      cases h with
      | inl hl =>
        exact ⟨(Cube.contains_splitFalse.mp hl.1).1, hl.2⟩
      | inr hr =>
        exact ⟨(Cube.contains_splitTrue.mp hr.1).1, hr.2⟩
    · intro h
      cases (Cube.split_partition c a z).mp h.1 with
      | inl hl => exact Or.inl ⟨hl, h.2⟩
      | inr hr => exact Or.inr ⟨hr, h.2⟩
  | narrowed c lo hi forcedLower forcedUpper child ih =>
    simp only [outputs, ih]
    constructor
    · intro h
      exact ⟨(Cube.contains_narrow.mp h.1).1, h.2⟩
    · intro h
      exact ⟨Cube.narrow_preserves_valid valid c lo hi forcedLower forcedUpper
        h.1 h.2, h.2⟩

theorem outputs_sound {c : Cube α} (tree : CoverageTree valid c)
    {z : Atoms α} (hz : z ∈ tree.outputs) : valid z :=
  ((tree.mem_outputs_iff z).mp hz).2

theorem outputs_complete {c : Cube α} (tree : CoverageTree valid c)
    {z : Atoms α} (hc : c.Contains z) (hv : valid z) : z ∈ tree.outputs :=
  (tree.mem_outputs_iff z).mpr ⟨hc, hv⟩

/-- Disjoint decision branches cannot produce duplicate accepted leaves. -/
theorem outputs_nodup {c : Cube α} (tree : CoverageTree valid c) :
    tree.outputs.Nodup := by
  induction tree with
  | refuted c sound => simp [outputs]
  | accepted z sound => simp [outputs]
  | split c a fresh left right ihLeft ihRight =>
    apply List.nodup_append.mpr
    refine ⟨ihLeft, ihRight, ?_⟩
    intro z hz w hw heq
    subst w
    exact Cube.split_disjoint c a z
      ⟨((left.mem_outputs_iff z).mp hz).1,
       ((right.mem_outputs_iff z).mp hw).1⟩
  | narrowed c lo hi forcedLower forcedUpper child ih => exact ih

/-- An exhausted proof tree with no accepted leaves certifies regional UNSAT. -/
theorem exhausted_no_valid {c : Cube α} (tree : CoverageTree valid c)
    (noAcceptedLeaves : tree.outputs = []) :
    ∀ z, c.Contains z → ¬ valid z := by
  intro z hz hv
  have hmem := tree.outputs_complete hz hv
  rw [noAcceptedLeaves] at hmem
  exact List.not_mem_nil hmem

end CoverageTree

/-!
Sampling has no coverage authority. Even a rejected complete seed says nothing
about a different seed in the same cube. The witness below uses a single atom,
so the distinction does not depend on large carriers or lazy materialization.
-/

def sampleCube : Cube Unit := ⟨Empty, Full⟩
def sampleValid (z : Atoms Unit) : Prop := z ()
def sampleAllValid (_ : Atoms Unit) : Prop := True

theorem rejected_sample_does_not_refute_region :
    sampleCube.Contains Empty ∧ ¬ sampleValid Empty ∧
      ∃ z, sampleCube.Contains z ∧ sampleValid z := by
  refine ⟨⟨?_, ?_⟩, ?_, Full, ⟨?_, ?_⟩, ?_⟩
  · intro a ha
    exact ha
  · intro _ _
    trivial
  · intro h
    exact h
  · intro _ h
    exact False.elim h
  · intro _ _
    trivial
  · trivial

/-- Accepting one sample also leaves another distinct valid seed unaccounted for. -/
theorem accepted_sample_does_not_exhaust_region :
    sampleCube.Contains Empty ∧ sampleAllValid Empty ∧
      ∃ z, sampleCube.Contains z ∧ sampleAllValid z ∧ z ≠ (Empty : Atoms Unit) := by
  refine ⟨⟨?_, ?_⟩, True.intro, Full, ⟨?_, ?_⟩, True.intro, ?_⟩
  · intro _ h
    exact h
  · intro _ _
    trivial
  · intro _ h
    exact False.elim h
  · intro _ _
    trivial
  · intro heq
    have ht : (Full : Atoms Unit) () := True.intro
    rw [heq] at ht
    exact ht

end Zetesis
