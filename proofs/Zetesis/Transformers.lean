import Zetesis.Core

/-!
Exact predicate-transformer algebra for reduct execution.

`Least T` is defined impredicatively as the intersection of all pre-fixed
points. Monotonicity is the substantive hypothesis which makes this
intersection itself closed. These proofs do not assume finite carriers,
bounded iteration, eager materialization, or a particular execution device.
-/

namespace Zetesis

universe u v

abbrev Transformer (α : Type u) := Atoms α → Atoms α

def MonotoneT {α : Type u} (T : Transformer α) : Prop :=
  ∀ {X Y}, Sub X Y → Sub (T X) (T Y)

def Closed {α : Type u} (T : Transformer α) (X : Atoms α) : Prop :=
  Sub (T X) X

def Least {α : Type u} (T : Transformer α) : Atoms α :=
  fun a => ∀ X, Closed T X → X a

def TBelow {α : Type u} (T U : Transformer α) : Prop :=
  ∀ X, Sub (T X) (U X)

def TEquivalent {α : Type u} (T U : Transformer α) : Prop :=
  ∀ X a, T X a ↔ U X a

/-- Function composition: execute `U`, then execute `T`. -/
def Compose {α : Type u} (T U : Transformer α) : Transformer α :=
  fun X => T (U X)

def Parallel {α : Type u} (T U : Transformer α) : Transformer α :=
  fun X => Union (T X) (U X)

def IdentityT {α : Type u} : Transformer α := id

def FilterT {α : Type u} (p : Atoms α) : Transformer α :=
  fun X => Inter p X

def Image {α : Type u} {β : Type v} (R : α → β → Prop)
    (X : Atoms α) : Atoms β :=
  fun b => ∃ a, X a ∧ R a b

theorem least_le {α : Type u} {T : Transformer α} {X : Atoms α}
    (hX : Closed T X) : Sub (Least T) X := by
  intro a ha
  exact ha X hX

theorem least_closed {α : Type u} {T : Transformer α}
    (hT : MonotoneT T) : Closed T (Least T) := by
  intro a ha X hX
  exact hX a (hT (least_le hX) a ha)

/-- For monotone operators the intersection of pre-fixed points is a fixed point. -/
theorem least_fixed {α : Type u} {T : Transformer α}
    (hT : MonotoneT T) : T (Least T) = Least T := by
  apply funext
  intro a
  apply propext
  constructor
  · exact least_closed hT a
  · exact least_le (hT (least_closed hT)) a

/-- Pointwise operator refinement preserves the order of least closures. -/
theorem least_mono {α : Type u} {T U : Transformer α}
    (hTU : TBelow T U) : Sub (Least T) (Least U) := by
  intro a ha X hX
  apply ha X
  intro b hb
  exact hX b (hTU X b hb)

theorem least_congr {α : Type u} {T U : Transformer α}
    (hTU : TEquivalent T U) : Least T = Least U := by
  apply funext
  intro a
  apply propext
  constructor
  · exact least_mono (fun X b h => (hTU X b).mp h) a
  · exact least_mono (fun X b h => (hTU X b).mpr h) a

/-- Sound emitted tuples plus complete closure suffice; support cycles alone do not. -/
theorem exact_of_sound_and_closed {α : Type u} {T : Transformer α}
    {X : Atoms α} (hsound : Sub X (Least T)) (hclosed : Closed T X) :
    X = Least T := by
  apply funext
  intro a
  exact propext ⟨hsound a, least_le hclosed a⟩

theorem identity_monotone {α : Type u} : MonotoneT (@IdentityT α) := by
  intro X Y h
  exact h

theorem compose_monotone {α : Type u} {T U : Transformer α}
    (hT : MonotoneT T) (hU : MonotoneT U) : MonotoneT (Compose T U) := by
  intro X Y h
  exact hT (hU h)

theorem parallel_monotone {α : Type u} {T U : Transformer α}
    (hT : MonotoneT T) (hU : MonotoneT U) : MonotoneT (Parallel T U) := by
  intro X Y h a ha
  cases ha with
  | inl ht => exact Or.inl (hT h a ht)
  | inr hu => exact Or.inr (hU h a hu)

theorem filter_monotone {α : Type u} (p : Atoms α) :
    MonotoneT (FilterT p) := by
  intro X Y h a ha
  exact ⟨ha.1, h a ha.2⟩

theorem image_monotone {α : Type u} {β : Type v} (R : α → β → Prop)
    {X Y : Atoms α} (h : Sub X Y) : Sub (Image R X) (Image R Y) := by
  intro b hb
  obtain ⟨a, ha, hab⟩ := hb
  exact ⟨a, h a ha, hab⟩

/-- Both composed operators may be refined, provided the new outer operator is monotone. -/
theorem compose_refines {α : Type u} {T T' U U' : Transformer α}
    (hT : TBelow T T') (hU : TBelow U U') (hmono : MonotoneT T') :
    TBelow (Compose T U) (Compose T' U') := by
  intro X a ha
  exact hmono (hU X) a (hT (U X) a ha)

theorem parallel_refines {α : Type u} {T T' U U' : Transformer α}
    (hT : TBelow T T') (hU : TBelow U U') :
    TBelow (Parallel T U) (Parallel T' U') := by
  intro X a ha
  cases ha with
  | inl ht => exact Or.inl (hT X a ht)
  | inr hu => exact Or.inr (hU X a hu)

theorem compose_congr {α : Type u} {T T' U U' : Transformer α}
    (hT : TEquivalent T T') (hU : TEquivalent U U') :
    TEquivalent (Compose T U) (Compose T' U') := by
  intro X a
  have heq : U X = U' X := funext (fun b => propext (hU X b))
  change T (U X) a ↔ T' (U' X) a
  rw [heq]
  exact hT (U' X) a

theorem parallel_congr {α : Type u} {T T' U U' : Transformer α}
    (hT : TEquivalent T T') (hU : TEquivalent U U') :
    TEquivalent (Parallel T U) (Parallel T' U') := by
  intro X a
  change (T X a ∨ U X a) ↔ (T' X a ∨ U' X a)
  exact or_congr (hT X a) (hU X a)

theorem compose_assoc {α : Type u} (T U V : Transformer α) :
    Compose (Compose T U) V = Compose T (Compose U V) := rfl

theorem parallel_compose {α : Type u} (T U V : Transformer α) :
    Compose (Parallel T U) V = Parallel (Compose T V) (Compose U V) := rfl

/-- A fused implementation may replace a pipeline inside recurrence only with
pointwise equivalence for every input, rather than agreement on one trace. -/
theorem fusion_preserves_least {α : Type u} {T U Fused : Transformer α}
    (hfused : TEquivalent (Compose T U) Fused) :
    Least (Compose T U) = Least Fused := least_congr hfused

/-- Gate/filter pushdown across projection requires the predicate to agree
on related source and target tuples. This states the binding obligation. -/
theorem filter_image_pushdown {α : Type u} {β : Type v}
    (R : α → β → Prop) (p : Atoms α) (q : Atoms β)
    (hbound : ∀ a b, R a b → (p a ↔ q b)) (X : Atoms α) :
    FilterT q (Image R X) = Image R (FilterT p X) := by
  apply funext
  intro b
  apply propext
  constructor
  · intro hb
    obtain ⟨a, ha, hab⟩ := hb.2
    exact ⟨a, ⟨(hbound a b hab).mpr hb.1, ha⟩, hab⟩
  · intro hb
    obtain ⟨a, ha, hab⟩ := hb
    exact ⟨(hbound a b hab).mp ha.1, a, ha.2, hab⟩

/-- A batch is exactly a product of isolated worlds; world mixing is excluded
by construction of the slice passed to each operator. -/
def Batch {α : Type u} {ω : Type v} (T : ω → Transformer α) :
    Transformer (ω × α) :=
  fun X wa => T wa.1 (fun a => X (wa.1, a)) wa.2

theorem batch_monotone {α : Type u} {ω : Type v} {T : ω → Transformer α}
    (hT : ∀ w, MonotoneT (T w)) : MonotoneT (Batch T) := by
  intro X Y h wa
  exact hT wa.1 (fun a ha => h (wa.1, a) ha) wa.2

theorem least_batch {α : Type u} {ω : Type v} {T : ω → Transformer α}
    (hT : ∀ w, MonotoneT (T w)) (w : ω) (a : α) :
    Least (Batch T) (w, a) ↔ Least (T w) a := by
  constructor
  · intro ha
    have hclosed : Closed (Batch T) (fun wa => Least (T wa.1) wa.2) := by
      intro wa hwa
      exact least_closed (hT wa.1) wa.2 hwa
    exact least_le hclosed (w, a) ha
  · intro ha X hX
    apply ha (fun b => X (w, b))
    intro b hb
    exact hX (w, b) hb

/-- Every finite prefix is a sound lower approximation, even without a
termination argument. Prefix closure equality must be established separately. -/
def FiniteIter {α : Type u} (T : Transformer α) : Nat → Atoms α
  | 0 => Empty
  | n + 1 => Union (FiniteIter T n) (T (FiniteIter T n))

theorem finiteIter_grows {α : Type u} (T : Transformer α) (n : Nat) :
    Sub (FiniteIter T n) (FiniteIter T (n + 1)) := by
  intro a ha
  exact Or.inl ha

theorem finiteIter_sound {α : Type u} {T : Transformer α}
    (hT : MonotoneT T) (n : Nat) : Sub (FiniteIter T n) (Least T) := by
  induction n with
  | zero =>
      intro a ha
      exact False.elim ha
  | succ n ih =>
      intro a ha
      cases ha with
      | inl hp => exact ih a hp
      | inr ht => exact least_closed hT a (hT ih a ht)

theorem finiteIter_exact_if_closed {α : Type u} {T : Transformer α}
    (hT : MonotoneT T) (n : Nat) (hclosed : Closed T (FiniteIter T n)) :
    FiniteIter T n = Least T :=
  exact_of_sound_and_closed (finiteIter_sound hT n) hclosed

end Zetesis
