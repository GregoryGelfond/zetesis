import Zetesis.Core

/-!
# Compositional lifted rule evaluation

This file proves a relational contract for evaluating rule templates without
enumerating a carrier of substitutions. `Binding` is an abstract type, not an
enumeration. A concrete parser, join implementation, GPU kernel, or event network
must separately establish that it implements these relations.

The default-negation gate is evaluated against a frozen candidate `z`; ordinary
positive antecedents are evaluated against the growing consequence set `X`.
The materializer results below are valid for the stated `X` and `z` only. A new
positive delta or a new candidate requires renewed coverage evidence.
-/

namespace Zetesis.Lifted

universe u v w

abbrev Bindings (β : Type v) := β → Prop

/-- A source template indexed by its possible substitutions. Filters include
    source-level equality/inequality and any already defined domain conditions. -/
structure Template (α : Type u) (β : Type v) where
  positive : β → List α
  gateTrue : β → List α
  gateFalse : β → List α
  filter : β → Prop
  head : β → α

variable {α : Type u} {β : Type v} {κ : Type w}

/-- The positive body join, including the empty-body binding when appropriate. -/
def Bind (t : Template α β) (X : Atoms α) : Bindings β :=
  fun b => ∀ a, a ∈ t.positive b → X a

def Enabled (t : Template α β) (z : Atoms α) (b : β) : Prop :=
  (∀ a, a ∈ t.gateTrue b → z a) ∧
  (∀ a, a ∈ t.gateFalse b → ¬ z a)

def Filter (t : Template α β) (R : Bindings β) : Bindings β :=
  fun b => R b ∧ t.filter b

def Gate (t : Template α β) (z : Atoms α) (R : Bindings β) : Bindings β :=
  fun b => R b ∧ Enabled t z b

/-- Existential projection already coalesces duplicate heads at the set level. -/
def Project (t : Template α β) (R : Bindings β) : Atoms α :=
  fun a => ∃ b, R b ∧ t.head b = a

def RuleTransform (t : Template α β) (z X : Atoms α) : Atoms α :=
  Project t (Gate t z (Filter t (Bind t X)))

/-- The consequence relation obtained by conceptual instantiation of every
    binding. The definition does not require materializing those instances. -/
def DirectConsequence (t : Template α β) (z X : Atoms α) : Atoms α :=
  fun a => ∃ b, t.filter b ∧ Enabled t z b ∧
    (∀ p, p ∈ t.positive b → X p) ∧ t.head b = a

theorem composition_exact (t : Template α β) (z X : Atoms α) (a : α) :
    RuleTransform t z X a ↔ DirectConsequence t z X a := by
  constructor
  · rintro ⟨b, ⟨⟨hb, hf⟩, hg⟩, hh⟩
    exact ⟨b, hf, hg, hb, hh⟩
  · rintro ⟨b, hf, hg, hb, hh⟩
    exact ⟨b, ⟨⟨hb, hf⟩, hg⟩, hh⟩

theorem bind_monotone (t : Template α β) {X Y : Atoms α}
    (hXY : Sub X Y) : ∀ b, Bind t X b → Bind t Y b := by
  intro b hb a ha
  exact hXY a (hb a ha)

theorem ruleTransform_monotone (t : Template α β) (z : Atoms α)
    {X Y : Atoms α} (hXY : Sub X Y) :
    Sub (RuleTransform t z X) (RuleTransform t z Y) := by
  intro a ha
  rcases ha with ⟨b, ⟨⟨hb, hf⟩, hg⟩, hh⟩
  exact ⟨b, ⟨⟨bind_monotone t hXY b hb, hf⟩, hg⟩, hh⟩

theorem filter_gate_commute (t : Template α β) (z : Atoms α)
    (R : Bindings β) (b : β) :
    Gate t z (Filter t R) b ↔ Filter t (Gate t z R) b := by
  constructor
  · rintro ⟨⟨hr, hf⟩, hg⟩
    exact ⟨⟨hr, hg⟩, hf⟩
  · rintro ⟨⟨hr, hg⟩, hf⟩
    exact ⟨⟨hr, hf⟩, hg⟩

/-- A gate can move to a partial binding only when its truth is determined by
    that partial binding. A concrete compiler must prove this factorization,
    for example by showing that every gate argument has already been bound. -/
def GateFactorsThrough (t : Template α β) (z : Atoms α)
    (key : β → κ) (ready : κ → Prop) : Prop :=
  ∀ b, Enabled t z b ↔ ready (key b)

def PushdownBind (t : Template α β) (X : Atoms α)
    (key : β → κ) (ready : κ → Prop) : Bindings β :=
  fun b => ready (key b) ∧ Bind t X b

theorem gate_pushdown_exact (t : Template α β) (z X : Atoms α)
    (key : β → κ) (ready : κ → Prop)
    (hfactor : GateFactorsThrough t z key ready) (a : α) :
    RuleTransform t z X a ↔
      Project t (Filter t (PushdownBind t X key ready)) a := by
  constructor
  · rintro ⟨b, ⟨⟨hb, hf⟩, hg⟩, hh⟩
    exact ⟨b, ⟨⟨(hfactor b).mp hg, hb⟩, hf⟩, hh⟩
  · rintro ⟨b, ⟨⟨hr, hb⟩, hf⟩, hh⟩
    exact ⟨b, ⟨⟨hb, hf⟩, (hfactor b).mpr hr⟩, hh⟩

/-- A partial-binding test may be only a necessary condition for gate success.
    It may still be pushed down if the complete gate remains after the join. -/
theorem necessary_gate_pushdown_exact (t : Template α β) (z X : Atoms α)
    (key : β → κ) (ready : κ → Prop)
    (hnecessary : ∀ b, Enabled t z b → ready (key b)) (a : α) :
    RuleTransform t z X a ↔
      Project t (Gate t z (Filter t (PushdownBind t X key ready))) a := by
  constructor
  · rintro ⟨b, ⟨⟨hb, hf⟩, hg⟩, hh⟩
    exact ⟨b, ⟨⟨⟨hnecessary b hg, hb⟩, hf⟩, hg⟩, hh⟩
  · rintro ⟨b, ⟨⟨⟨_hr, hb⟩, hf⟩, hg⟩, hh⟩
    exact ⟨b, ⟨⟨hb, hf⟩, hg⟩, hh⟩

/-- The selected bindings for one materializer snapshot. Selection is an
    arbitrary relation and may be sparse; false means "not selected", not that
    any atom occurring in that binding is known false. -/
def RestrictedBind (t : Template α β) (X : Atoms α)
    (selected : Bindings β) : Bindings β :=
  fun b => Bind t X b ∧ selected b

def MaterializedTransform (t : Template α β) (z X : Atoms α)
    (selected : Bindings β) : Atoms α :=
  Project t (Gate t z (Filter t (RestrictedBind t X selected)))

/-- Source coverage is stated per binding, not assumed output equality. It
    suffices to visit all enabled, filter-valid, positively satisfied bindings.
    This condition is sufficient, not necessary when different bindings have
    the same head. -/
def SourceCoverage (t : Template α β) (z X : Atoms α)
    (selected : Bindings β) : Prop :=
  ∀ b, Bind t X b → t.filter b → Enabled t z b → selected b

theorem materialized_sound (t : Template α β) (z X : Atoms α)
    (selected : Bindings β) :
    Sub (MaterializedTransform t z X selected) (DirectConsequence t z X) := by
  intro a ha
  rcases ha with ⟨b, ⟨⟨⟨hb, _hs⟩, hf⟩, hg⟩, hh⟩
  exact ⟨b, hf, hg, hb, hh⟩

theorem materialized_complete (t : Template α β) (z X : Atoms α)
    (selected : Bindings β) (hcover : SourceCoverage t z X selected) :
    Sub (DirectConsequence t z X) (MaterializedTransform t z X selected) := by
  intro a ha
  rcases ha with ⟨b, hf, hg, hb, hh⟩
  exact ⟨b, ⟨⟨⟨hb, hcover b hb hf hg⟩, hf⟩, hg⟩, hh⟩

theorem materialized_exact (t : Template α β) (z X : Atoms α)
    (selected : Bindings β) (hcover : SourceCoverage t z X selected) (a : α) :
    MaterializedTransform t z X selected a ↔ DirectConsequence t z X a :=
  ⟨materialized_sound t z X selected a,
    materialized_complete t z X selected hcover a⟩

/-- An omission is justified by a failed antecedent, filter, or frozen gate.
    This explicit certificate is one way to prove source coverage. -/
def OmissionCertificate (t : Template α β) (z X : Atoms α)
    (selected : Bindings β) : Prop :=
  ∀ b, ¬ selected b →
    (∃ a, a ∈ t.positive b ∧ ¬ X a) ∨
    ¬ t.filter b ∨
    (∃ a, a ∈ t.gateTrue b ∧ ¬ z a) ∨
    (∃ a, a ∈ t.gateFalse b ∧ z a)

theorem omissions_cover (t : Template α β) (z X : Atoms α)
    (selected : Bindings β) (hcert : OmissionCertificate t z X selected) :
    SourceCoverage t z X selected := by
  intro b hb hf hg
  classical
  by_cases hs : selected b
  · exact hs
  apply False.elim
  have hn : ¬ selected b := hs
  rcases hcert b hn with hpos | hfilter | htrue | hfalse
  · rcases hpos with ⟨a, ha, hna⟩
    exact hna (hb a ha)
  · exact hfilter hf
  · rcases htrue with ⟨a, ha, hna⟩
    exact hna (hg.1 a ha)
  · rcases hfalse with ⟨a, ha, hza⟩
    exact hg.2 a ha hza

theorem certified_materializer_exact (t : Template α β) (z X : Atoms α)
    (selected : Bindings β) (hcert : OmissionCertificate t z X selected) (a : α) :
    MaterializedTransform t z X selected a ↔ DirectConsequence t z X a :=
  materialized_exact t z X selected (omissions_cover t z X selected hcert) a

/-- Adding selected bindings cannot withdraw consequences. This is a set-level
    retry/continuation property; publication and cursor protocols remain an
    implementation obligation. -/
theorem materialized_selection_monotone (t : Template α β) (z X : Atoms α)
    {M N : Bindings β} (hMN : ∀ b, M b → N b) :
    Sub (MaterializedTransform t z X M) (MaterializedTransform t z X N) := by
  intro a ha
  rcases ha with ⟨b, ⟨⟨⟨hb, hm⟩, hf⟩, hg⟩, hh⟩
  exact ⟨b, ⟨⟨⟨hb, hMN b hm⟩, hf⟩, hg⟩, hh⟩

theorem materialized_replay_idempotent (t : Template α β) (z X : Atoms α)
    (M : Bindings β) (a : α) :
    MaterializedTransform t z X (fun b => M b ∨ M b) a ↔
      MaterializedTransform t z X M a := by
  constructor
  · exact materialized_selection_monotone t z X (fun _ h => h.elim id id) a
  · exact materialized_selection_monotone t z X (fun _ h => Or.inl h) a

/-- The same binding/filter/gate pipeline checks constraints by existential
    reduction instead of head projection. The template head is unused here. -/
def ConstraintTriggered (t : Template α β) (z X : Atoms α) : Prop :=
  ∃ b, t.filter b ∧ Enabled t z b ∧ Bind t X b

def MaterializedConstraint (t : Template α β) (z X : Atoms α)
    (selected : Bindings β) : Prop :=
  ∃ b, Gate t z (Filter t (RestrictedBind t X selected)) b

theorem materialized_constraint_exact (t : Template α β) (z X : Atoms α)
    (selected : Bindings β) (hcover : SourceCoverage t z X selected) :
    MaterializedConstraint t z X selected ↔ ConstraintTriggered t z X := by
  constructor
  · rintro ⟨b, ⟨⟨hb, _hs⟩, hf⟩, hg⟩
    exact ⟨b, hf, hg, hb⟩
  · rintro ⟨b, hf, hg, hb⟩
    exact ⟨b, ⟨⟨⟨hb, hcover b hb hf hg⟩, hf⟩, hg⟩⟩

/-- A heterogeneous family permits a different binding type per source
    template. Source registration is represented by the whole index type `ι`. -/
def FamilyConsequence {ι : Type w} {β : ι → Type v}
    (templates : (i : ι) → Template α (β i)) (z X : Atoms α) : Atoms α :=
  fun a => ∃ i, DirectConsequence (templates i) z X a

def MaterializedFamily {ι : Type w} {β : ι → Type v}
    (templates : (i : ι) → Template α (β i)) (z X : Atoms α)
    (selected : (i : ι) → Bindings (β i)) : Atoms α :=
  fun a => ∃ i, MaterializedTransform (templates i) z X (selected i) a

theorem family_exact {ι : Type w} {β : ι → Type v}
    (templates : (i : ι) → Template α (β i)) (z X : Atoms α)
    (selected : (i : ι) → Bindings (β i))
    (hcover : ∀ i, SourceCoverage (templates i) z X (selected i)) (a : α) :
    MaterializedFamily templates z X selected a ↔
      FamilyConsequence templates z X a := by
  constructor
  · rintro ⟨i, hi⟩
    exact ⟨i, (materialized_exact _ _ _ _ (hcover i) a).mp hi⟩
  · rintro ⟨i, hi⟩
    exact ⟨i, (materialized_exact _ _ _ _ (hcover i) a).mpr hi⟩

/-- Only a materializer covered at this exact `X,z` can certify source-level
    closedness. An exhausted queue without coverage does not meet this premise. -/
theorem family_closedness_exact {ι : Type w} {β : ι → Type v}
    (templates : (i : ι) → Template α (β i)) (z X : Atoms α)
    (selected : (i : ι) → Bindings (β i))
    (hcover : ∀ i, SourceCoverage (templates i) z X (selected i)) :
    Sub (MaterializedFamily templates z X selected) X ↔
      Sub (FamilyConsequence templates z X) X := by
  constructor
  · intro hm a ha
    exact hm a ((family_exact templates z X selected hcover a).mpr ha)
  · intro hd a ha
    exact hd a ((family_exact templates z X selected hcover a).mp ha)

/-- A synchronous consequence round; inflation is explicit. -/
def FullStep {ι : Type w} {β : ι → Type v}
    (templates : (i : ι) → Template α (β i)) (z X : Atoms α) : Atoms α :=
  Union X (FamilyConsequence templates z X)

def LazyStep {ι : Type w} {β : ι → Type v}
    (templates : (i : ι) → Template α (β i)) (z X : Atoms α)
    (selected : (i : ι) → Bindings (β i)) : Atoms α :=
  Union X (MaterializedFamily templates z X selected)

theorem lazy_step_exact {ι : Type w} {β : ι → Type v}
    (templates : (i : ι) → Template α (β i)) (z X : Atoms α)
    (selected : (i : ι) → Bindings (β i))
    (hcover : ∀ i, SourceCoverage (templates i) z X (selected i)) :
    LazyStep templates z X selected = FullStep templates z X := by
  apply atoms_ext
  intro a
  constructor
  · intro ha
    rcases ha with hx | hm
    · exact Or.inl hx
    · exact Or.inr ((family_exact templates z X selected hcover a).mp hm)
  · intro ha
    rcases ha with hx | hd
    · exact Or.inl hx
    · exact Or.inr ((family_exact templates z X selected hcover a).mpr hd)

def FullStage {ι : Type w} {β : ι → Type v}
    (templates : (i : ι) → Template α (β i)) (z : Atoms α) : Nat → Atoms α
  | 0 => Empty
  | n + 1 => FullStep templates z (FullStage templates z n)

/-- Selection can depend on the round and its current consequence snapshot.
    These are covered rounds, not arbitrary partially drained queue states. -/
def LazyStage {ι : Type w} {β : ι → Type v}
    (templates : (i : ι) → Template α (β i)) (z : Atoms α)
    (selected : Nat → Atoms α → (i : ι) → Bindings (β i)) : Nat → Atoms α
  | 0 => Empty
  | n + 1 => LazyStep templates z (LazyStage templates z selected n)
      (selected n (LazyStage templates z selected n))

/-- Coverage must hold at every completed round. This proves equivalence to
    conceptual full instantiation at each finite stage, without enumerating
    substitutions. It does not claim termination on infinite atom domains. -/
theorem lazy_stages_exact {ι : Type w} {β : ι → Type v}
    (templates : (i : ι) → Template α (β i)) (z : Atoms α)
    (selected : Nat → Atoms α → (i : ι) → Bindings (β i))
    (hcover : ∀ n i, SourceCoverage (templates i) z
      (LazyStage templates z selected n)
      (selected n (LazyStage templates z selected n) i)) :
    ∀ n, LazyStage templates z selected n = FullStage templates z n := by
  intro n
  induction n with
  | zero => rfl
  | succ n ih =>
    change LazyStep templates z (LazyStage templates z selected n)
      (selected n (LazyStage templates z selected n)) =
      FullStep templates z (FullStage templates z n)
    rw [lazy_step_exact templates z _ _ (hcover n), ih]

/-- A stale coverage certificate is unsafe after the positive relation grows.
    This template has the single instance `1 :- 0`. -/
def SnapshotExample : Template Nat Unit where
  positive := fun _ => [0]
  gateTrue := fun _ => []
  gateFalse := fun _ => []
  filter := fun _ => True
  head := fun _ => 1

theorem empty_snapshot_covered :
    SourceCoverage SnapshotExample Empty Empty (fun _ => False) := by
  intro b hb _hf _hg
  exact hb 0 (by simp [SnapshotExample])

theorem grown_snapshot_not_covered :
    ¬ SourceCoverage SnapshotExample Empty (fun a => a = 0) (fun _ => False) := by
  intro h
  apply h ()
  · intro a ha
    simpa [SnapshotExample] using ha
  · trivial
  · constructor <;> intro a ha <;> simp [SnapshotExample] at ha

theorem omitted_new_consequence :
    DirectConsequence SnapshotExample Empty (fun a => a = 0) 1 ∧
    ¬ MaterializedTransform SnapshotExample Empty (fun a => a = 0)
      (fun _ => False) 1 := by
  constructor
  · refine ⟨(), True.intro, ?_, ?_, rfl⟩
    · constructor <;> intro a ha <;> simp [SnapshotExample] at ha
    · intro a ha
      simpa [SnapshotExample] using ha
  · rintro ⟨_b, ⟨⟨⟨_hb, hfalse⟩, _hf⟩, _hg⟩, _hh⟩
    exact hfalse

end Zetesis.Lifted
