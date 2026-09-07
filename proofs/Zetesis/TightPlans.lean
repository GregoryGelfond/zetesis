import Zetesis.RuleFactorization

/-!
# Ranked support over original formula producers

A checked positive rank lets original satisfaction and support discharge reduct
minimality for finite normal and atomic-choice producers with nested bodies.
Default negation freezes an arbitrary interior; unnegated body implications do
not belong to this grammar. Original producer formulas, including choices, are
retained. No shift, SAT encoding or source-class assumption occurs in the proof.

The theorems state the formula-level certificate obligations. They do not prove
that Rust DAG extraction, graph ranking, allocation, or a lazy source registry
refines these definitions. A Rust certificate is about one complete immutable
finite theory; missing source instances need an additional coverage proof.
-/

namespace Zetesis.TightPlans

universe u
variable {α : Type u}
open Ferraris

/-- Positive nested expressions, with arbitrary candidate-frozen negations. -/
inductive Body (α : Type u) where
  | atom : α → Body α
  | bot : Body α
  | conj : Body α → Body α → Body α
  | disj : Body α → Body α → Body α
  | neg : Formula α → Body α

def Body.formula : Body α → Formula α
  | .atom a => .atom a
  | .bot => .bot
  | .conj F G => .conj F.formula G.formula
  | .disj F G => .disj F.formula G.formula
  | .neg F => Ferraris.Neg F

/-- Occurrence outside every default-negation scope, not classical polarity. -/
def Body.Positive : Body α → α → Prop
  | .atom b, a => a = b
  | .bot, _ => False
  | .conj F G, a => F.Positive a ∨ G.Positive a
  | .disj F G, a => F.Positive a ∨ G.Positive a
  | .neg _, _ => False

def truthBody : Body α := .neg .bot

/-- Each producer retains its exact original head and implication structure. -/
inductive Producer (α : Type u) where
  | fact : α → Producer α
  | normal : Body α → α → Producer α
  | choiceFact : α → Producer α
  | choice : Body α → α → Producer α
  | choiceFactReversed : α → Producer α
  | choiceReversed : Body α → α → Producer α

def Producer.head : Producer α → α
  | .fact a | .normal _ a | .choiceFact a | .choice _ a
  | .choiceFactReversed a | .choiceReversed _ a => a

def Producer.body : Producer α → Body α
  | .fact _ | .choiceFact _ | .choiceFactReversed _ => truthBody
  | .normal B _ | .choice B _ | .choiceReversed B _ => B

def Producer.formula : Producer α → Formula α
  | .fact a => .atom a
  | .normal B a => .imp B.formula (.atom a)
  | .choiceFact a => .disj (.atom a) (Ferraris.Neg (.atom a))
  | .choice B a => .imp B.formula (.disj (.atom a) (Ferraris.Neg (.atom a)))
  | .choiceFactReversed a => .disj (Ferraris.Neg (.atom a)) (.atom a)
  | .choiceReversed B a => .imp B.formula (.disj (Ferraris.Neg (.atom a)) (.atom a))

/-- Each present atom has a true original body for an actual producer. -/
def Supported (M : Atoms α) (rules : List (Producer α)) : Prop :=
  ∀ a, M a → ∃ r ∈ rules, r.head = a ∧ Satisfies M r.body.formula

/-- The certificate checks every unnegated producer dependency. -/
def Ranked (rank : α → Nat) (rules : List (Producer α)) : Prop :=
  ∀ r ∈ rules, ∀ a, r.body.Positive a → rank a < rank r.head

/-- Producers are extracted from asserted original roots, never invented. -/
def OriginalProducers (T : Theory α) (rules : List (Producer α)) : Prop :=
  ∀ r ∈ rules, r.formula ∈ T

/-- Every root is a producer, a frozen negation, or falsum. No selected-root
    projection may stand in for this completed-theory obligation. -/
def Covered (T : Theory α) (rules : List (Producer α)) : Prop :=
  ∀ F ∈ T, (∃ r ∈ rules, r.formula = F) ∨ (∃ G, F = Ferraris.Neg G) ∨ F = .bot

theorem original_of_reduct (M J : Atoms α) (F : Formula α)
    (h : Satisfies J (Reduct M F)) : Satisfies M F := by
  classical
  apply Classical.byContradiction
  intro hfalse
  rw [RuleFactorization.false_reduct M F hfalse] at h
  exact h

/-- An arbitrary default negation is frozen, including implications inside it. -/
theorem negation_frozen (M J : Atoms α) (F : Formula α) :
    Satisfies J (Reduct M (Ferraris.Neg F)) ↔ ¬ Satisfies M F := by
  classical
  by_cases h : Satisfies M F
  · simp [Reduct, Ferraris.Neg, Satisfies, h]
  · simp [Reduct, Ferraris.Neg, Satisfies, h, RuleFactorization.false_reduct M F h]

/-- A true nested body remains true in the reduct when its true positive inputs
    have been established. Frozen inputs need no fact in the smaller world. -/
theorem body_reduct_of_positive (M J : Atoms α) (B : Body α)
    (lower : ∀ a, B.Positive a → M a → J a)
    (trueBody : Satisfies M B.formula) : Satisfies J (Reduct M B.formula) := by
  induction B with
  | atom a =>
    exact (atom_reduct M J a).mpr ⟨trueBody, lower a rfl trueBody⟩
  | bot => exact False.elim trueBody
  | conj F G ihF ihG =>
    apply (RuleFactorization.reduct_conj M J F.formula G.formula).mpr
    exact ⟨ihF (fun a ha => lower a (Or.inl ha)) trueBody.1,
      ihG (fun a ha => lower a (Or.inr ha)) trueBody.2⟩
  | disj F G ihF ihG =>
    apply (RuleFactorization.reduct_disj M J F.formula G.formula).mpr
    rcases trueBody with hF | hG
    · exact Or.inl (ihF (fun a ha => lower a (Or.inl ha)) hF)
    · exact Or.inr (ihG (fun a ha => lower a (Or.inr ha)) hG)
  | neg F => exact (negation_frozen M J F).mpr trueBody

/-- Exchanging the two original atomic-choice branches preserves its frozen gate. -/
theorem reversed_choice_reduct_guard (M J : Atoms α) (a : α) :
    Satisfies J (Reduct M (.disj (Ferraris.Neg (.atom a)) (.atom a))) ↔
      (M a → J a) := by
  classical
  by_cases present : M a <;>
    simp [RuleFactorization.reduct_disj, atom_reduct, neg_atom_reduct, present]

/-- Both ordinary and choice producer reducts force a present head once the
    reduced body holds. This keeps the original choice rather than shifting. -/
theorem producer_forces_head (M J : Atoms α) (r : Producer α)
    (present : M r.head) (root : Satisfies J (Reduct M r.formula))
    (body : Satisfies J (Reduct M r.body.formula)) : J r.head := by
  cases r with
  | fact a => exact ((atom_reduct M J a).mp root).2
  | normal B a =>
    have h := ((RuleFactorization.reduct_imp M J B.formula (.atom a)).mp root).2 body
    exact ((atom_reduct M J a).mp h).2
  | choiceFact a => exact (choice_reduct_guard M J a).mp root present
  | choice B a =>
    have h := ((RuleFactorization.reduct_imp M J B.formula
      (.disj (.atom a) (Ferraris.Neg (.atom a)))).mp root).2 body
    exact (choice_reduct_guard M J a).mp h present
  | choiceFactReversed a => exact (reversed_choice_reduct_guard M J a).mp root present
  | choiceReversed B a =>
    have h := ((RuleFactorization.reduct_imp M J B.formula
      (.disj (Ferraris.Neg (.atom a)) (.atom a))).mp root).2 body
    exact (reversed_choice_reduct_guard M J a).mp h present

/-- Induction over the checked rank makes every reduct model contain M. -/
theorem ranked_support_contained (M J : Atoms α) (T : Theory α)
    (rules : List (Producer α)) (rank : α → Nat)
    (original : OriginalProducers T rules) (ranked : Ranked rank rules)
    (supported : Supported M rules) (reduced : Models J (ReductTheory M T)) :
    Sub M J := by
  have byRank : ∀ n, ∀ a, rank a = n → M a → J a := by
    intro n
    induction n using Nat.strongRecOn with
    | ind n ih =>
      intro a ha present
      obtain ⟨r, hr, head, body⟩ := supported a present
      have lower : ∀ b, r.body.Positive b → M b → J b := by
        intro b hb presentB
        have lt : rank b < n := by
          rw [← ha, ← head]
          exact ranked r hr b hb
        exact ih (rank b) lt b rfl presentB
      have bodyJ := body_reduct_of_positive M J r.body lower body
      have rootJ : Satisfies J (Reduct M r.formula) :=
        reduced (Reduct M r.formula) (List.mem_map.mpr ⟨r.formula, original r hr, rfl⟩)
      rw [← head]
      apply producer_forces_head M J r
      · simpa [head] using present
      · exact rootJ
      · exact bodyJ
  intro a present
  exact byRank (rank a) a rfl present

/-- Original satisfaction plus certified ranked support establishes stability.
    Additional asserted roots are still tested in the original interpretation. -/
theorem ranked_support_stable (M : Atoms α) (T : Theory α)
    (rules : List (Producer α)) (rank : α → Nat)
    (original : OriginalProducers T rules) (ranked : Ranked rank rules)
    (model : Models M T) (supported : Supported M rules) : Stable M T := by
  refine ⟨model, ?_⟩
  rintro ⟨J, ⟨_, proper⟩, reduced⟩
  exact proper (ranked_support_contained M J T rules rank original ranked supported reduced)

/-- An asserted original producer remains a reduct model if every relevant
    candidate head is preserved. This lemma also covers inactive bodies. -/
theorem producer_reduct_of_heads (M J : Atoms α) (r : Producer α)
    (model : Satisfies M r.formula)
    (heads : Satisfies M r.body.formula → M r.head → J r.head) :
    Satisfies J (Reduct M r.formula) := by
  cases r with
  | fact a =>
    apply (atom_reduct M J a).mpr
    exact ⟨model, heads (by simp [Producer.body, truthBody, Body.formula, Ferraris.Neg, Satisfies]) model⟩
  | normal B a =>
    apply (RuleFactorization.reduct_imp M J B.formula (.atom a)).mpr
    refine ⟨model, ?_⟩
    intro body
    have originalBody := original_of_reduct M J B.formula body
    have present : M a := model originalBody
    exact (atom_reduct M J a).mpr ⟨present, heads originalBody present⟩
  | choiceFact a =>
    apply (choice_reduct_guard M J a).mpr
    exact heads (by simp [Producer.body, truthBody, Body.formula, Ferraris.Neg, Satisfies])
  | choice B a =>
    apply (RuleFactorization.reduct_imp M J B.formula
      (.disj (.atom a) (Ferraris.Neg (.atom a)))).mpr
    refine ⟨model, ?_⟩
    intro body
    apply (choice_reduct_guard M J a).mpr
    exact heads (original_of_reduct M J B.formula body)
  | choiceFactReversed a =>
    apply (reversed_choice_reduct_guard M J a).mpr
    exact heads (by simp [Producer.body, truthBody, Body.formula, Ferraris.Neg, Satisfies])
  | choiceReversed B a =>
    apply (RuleFactorization.reduct_imp M J B.formula
      (.disj (Ferraris.Neg (.atom a)) (.atom a))).mpr
    refine ⟨model, ?_⟩
    intro body
    apply (reversed_choice_reduct_guard M J a).mpr
    exact heads (original_of_reduct M J B.formula body)

/-- An unsupported present atom can be deleted from a model of this complete
    producer/guard grammar. This proves necessity without a rank assumption. -/
theorem stable_supported (M : Atoms α) (T : Theory α) (rules : List (Producer α))
    (covered : Covered T rules) (stable : Stable M T) : Supported M rules := by
  classical
  intro a present
  apply Classical.byContradiction
  intro unsupported
  let J : Atoms α := fun b => M b ∧ b ≠ a
  have proper : ProperSub J M := by
    refine ⟨fun b hb => hb.1, ?_⟩
    intro back
    exact (back a present).2 rfl
  have reduced : Models J (ReductTheory M T) := by
    intro reducedRoot member
    obtain ⟨F, hF, rfl⟩ := List.mem_map.mp member
    rcases covered F hF with ⟨r, hr, rfl⟩ | ⟨G, rfl⟩ | rfl
    · apply producer_reduct_of_heads M J r (stable.1 r.formula hF)
      intro body presentHead
      refine ⟨presentHead, ?_⟩
      intro head
      exact unsupported ⟨r, hr, head, body⟩
    · apply (negation_frozen M J G).mpr
      exact stable.1 (Ferraris.Neg G) hF
    · exact False.elim (stable.1 .bot hF)
  exact stable.2 ⟨J, proper, reduced⟩

/-- Complete ranked producer theories have an exact satisfaction/support
    characterization. A witness-based Rust API may still return residual on
    failed support until it constructs or checks the deletion witness. -/
theorem stable_iff_ranked_support (M : Atoms α) (T : Theory α)
    (rules : List (Producer α)) (rank : α → Nat)
    (original : OriginalProducers T rules) (covered : Covered T rules)
    (ranked : Ranked rank rules) :
    Stable M T ↔ Models M T ∧ Supported M rules := by
  constructor
  · intro stable
    exact ⟨stable.1, stable_supported M T rules covered stable⟩
  · rintro ⟨model, supported⟩
    exact ranked_support_stable M T rules rank original ranked model supported

/-- A positive self-dependency cannot receive a checked strict rank. -/
theorem ranked_no_self (rank : α → Nat) (rules : List (Producer α))
    (ranked : Ranked rank rules) (r : Producer α) (member : r ∈ rules) :
    ¬ r.body.Positive r.head := by
  intro self
  exact Nat.lt_irrefl (rank r.head) (ranked r member r.head self)

/-- Renumbering a rank by a strictly increasing map preserves its certificate. -/
theorem ranked_mono (rank : α → Nat) (rules : List (Producer α))
    (ranked : Ranked rank rules) (f : Nat → Nat)
    (strict : ∀ a b, a < b → f a < f b) : Ranked (fun a => f (rank a)) rules := by
  intro r hr a ha
  exact strict (rank a) (rank r.head) (ranked r hr a ha)

end Zetesis.TightPlans
