import Zetesis.NumericWitnessReplay

/-!
# Candidate knowledge from a complete producer scan

For a held atom the candidate narrower can demand a unique surviving
producer's body. When no producer survives it can cut the atom. Its two
blocking tests read a body already known false or, for an ordinary producer,
another head already held. Their evidence may hold only in stable completions.
This module states the invariant on that class; none of these facts is used
to certify a proper-subset model in the membership check.
-/

namespace Zetesis.Refinement.StableProducerKnowledge

open Ferraris DisjunctiveSupport
universe u
variable {α : Type u} [DecidableEq α]

/-- A signed original-formula fact on the stable completions of one region. -/
def Sign (T : Theory α) (r : Region α) (F : Formula α) (b : Bool) : Prop :=
  ∀ M, r.Contains M → Stable M T → (Satisfies M F ↔ b = true)

/-- An ordinary row blocks on a competing held head. Atomic choices have
only their own head and carry no such blocking condition. -/
inductive Source (α : Type u) where
  | ordinary (p : Producer α)
  | choice (p : ChoiceProducer α)

/-- Head occurrences in source order. Repeated coordinates retain their positions. -/
def headAtoms : Head α → List α
  | .atom a => [a]
  | .bot => []
  | .disj F G => headAtoms F ++ headAtoms G

/-- The head column of an extracted source row. -/
def Source.heads : Source α → List α
  | .ordinary p => headAtoms p.head
  | .choice p => [p.head]

/-- An unconditional fact has no body to block or force. -/
def Source.body : Source α → Option (Formula α)
  | .ordinary (.rule body _) | .choice (.rule body _) | .choice (.reversedRule body _) => some body
  | _ => none

/-- Use the existing ordinary or atomic-choice support semantics. -/
def Source.supports (M : Atoms α) (a : α) : Source α → Prop
  | .ordinary p => p.Supports M a
  | .choice p => p.Supports M a

/-- The two bit lookups performed for a prospective supporter of a. -/
def possible (a : α) (held : α → Bool) (never : Formula α → Bool) (s : Source α) : Bool :=
  s.heads.contains a && !(s.body.any never) && match s with
    | .choice _ => true
    | .ordinary _ => !(s.heads.any fun b => b != a && held b)

/-- The semantic inventory includes every ordinary and choice producer
of the admitted theory. Filtering retains order and repeated sources. -/
def remaining (a : α) (held : α → Bool) (never : Formula α → Bool)
    (rules : List (Producer α)) (choices : List (ChoiceProducer α)) : List (Source α) :=
  (rules.map Source.ordinary ++ choices.map Source.choice).filter (possible a held never)

omit [DecidableEq α] in
/-- Flattening a positive disjunctive head preserves semantic head membership. -/
theorem head_mem (head : Head α) (a : α) : a ∈ headAtoms head ↔ head.Contains a := by
  induction head with
  | atom b => simp [headAtoms, Head.Contains]
  | bot => simp [headAtoms, Head.Contains]
  | disj F G left right => simp [headAtoms, Head.Contains, left, right]

omit [DecidableEq α] in
/-- A supporting ordinary or choice row with a body has that body true. -/
theorem supported_body {M : Atoms α} {a : α} (s : Source α) (active : s.supports M a)
    {F : Formula α} (present : s.body = some F) : Satisfies M F := by
  cases s with
  | ordinary p => cases p <;> simp_all [Source.body, Source.supports, Producer.Supports, Producer.Enabled]
  | choice p => cases p <;> simp_all [Source.body, Source.supports, ChoiceProducer.Supports, ChoiceProducer.Enabled]

/-- Neither admissible blocker can eliminate actual support in the tested interpretation. -/
theorem supported_possible {M : Atoms α} {a : α} (s : Source α) (active : s.supports M a)
    {held : α → Bool} {never : Formula α → Bool}
    (heldTrue : ∀ b, held b = true → M b) (bodyFalse : ∀ F, never F = true → ¬ Satisfies M F) :
    possible a held never s = true := by
  have body : s.body.any never = false := by
    cases present : s.body with
    | none => rfl
    | some F =>
      have trueBody := supported_body s active present
      have absent : never F = false := Bool.eq_false_iff.mpr (fun marked => bodyFalse F marked trueBody)
      simp [absent]
  cases s with
  | choice p =>
    have head := active.2
    simp [possible, Source.heads, body, head]
  | ordinary p =>
    have member := (head_mem p.head a).mpr active.2.1
    have other : (headAtoms p.head).any (fun b => b != a && held b) = false := by
      apply Bool.eq_false_iff.mpr
      intro found
      obtain ⟨b, inside, bits⟩ := List.any_eq_true.mp found
      have signs : b ≠ a ∧ held b = true := by simpa using bits
      have equal := active.2.2 b ((head_mem p.head b).mp inside) (heldTrue b signs.2)
      simp [equal] at signs
    simp [possible, Source.heads, body, member, other]

/-- With complete producer coverage, every stable completion containing a
has a surviving support row. Use the reduct support theorem to obtain the
row and show that neither of the two concrete blockers could eliminate it. -/
theorem stable_support_survives {T : Theory α} {r : Region α}
    (rules : List (Producer α)) (choices : List (ChoiceProducer α))
    (covered : CoveredWithChoices T rules choices)
    (held : α → Bool) (never : Formula α → Bool)
    (holds : ∀ a, held a = true → Sign T r (.atom a) true)
    (fails : ∀ F, never F = true → Sign T r F false)
    {M : Atoms α} (inside : r.Contains M) (stable : Stable M T) {a : α} (present : M a) :
    ∃ s ∈ remaining a held never rules choices, s.supports M a := by
  have survives (s : Source α) (active : s.supports M a) : possible a held never s = true :=
    supported_possible s active (fun b h => (holds b h M inside stable).mpr rfl)
      (fun F h satisfied => Bool.false_ne_true ((fails F h M inside stable).mp satisfied))
  rcases answer_set_supported_with_choices M T rules choices covered stable a present with
    ⟨p, inRules, active⟩ | ⟨p, inChoices, active⟩
  · have accepts := survives (.ordinary p) active
    exact ⟨.ordinary p, List.mem_filter.mpr ⟨List.mem_append_left _ (List.mem_map_of_mem inRules), accepts⟩, active⟩
  · have accepts := survives (.choice p) active
    exact ⟨.choice p, List.mem_filter.mpr ⟨List.mem_append_right _ (List.mem_map_of_mem inChoices), accepts⟩, active⟩

/-- An empty scan supplies an atom-never bit. A single surviving body supplies
a sure bit when its head is held. Both newly published signs have the same
stable-completion invariant as the input bits, so they may supply later producer scans. -/
theorem scan_signs {T : Theory α} (r : Region α) (rules : List (Producer α))
    (choices : List (ChoiceProducer α)) (covered : CoveredWithChoices T rules choices)
    (held : α → Bool) (never : Formula α → Bool)
    (holds : ∀ a, held a = true → Sign T r (.atom a) true)
    (fails : ∀ F, never F = true → Sign T r F false) (a : α) :
    (remaining a held never rules choices = [] → Sign T r (.atom a) false) ∧
    (∀ s F, remaining a held never rules choices = [s] → s.body = some F → held a = true →
      Sign T r F true) := by
  constructor
  · intro none M inside stable
    constructor
    · intro present
      obtain ⟨s, member, _⟩ := stable_support_survives rules choices covered held never holds fails inside stable present
      simp [none] at member
    · intro impossible; contradiction
  · intro s F sole body h M inside stable
    have present : M a := (holds a h M inside stable).mpr rfl
    obtain ⟨source, member, active⟩ := stable_support_survives rules choices covered held never holds fails inside stable present
    have same : source = s := by simpa [sole] using member
    exact ⟨fun _ => rfl, fun _ => supported_body s (same ▸ active) body⟩

open Classical in
omit [DecidableEq α] in
/-- A witness row can use established stable-only original signs. A prospective
stable completion supplies every asserted root; elsewhere the row must carry
the sign of the old candidate. Holding the witness and one further atom makes
the transferred model of the reduct a proper subset of the completion. -/
theorem stable_signs_refute_region {T : Theory α} {M N : Atoms α} (r : Region α)
    (model : Models M T) (witness : ReductRegions.Countermodel M T N)
    (held : Sub N r.lower) {a : α} (extra : r.lower a) (outside : ¬ N a)
    (knowledge : ∀ F ∈ T, ∀ G ∈ LearnedRegions.replayNodes M N F, G ∉ T →
      Sign T r G (decide (Satisfies M G))) :
    ∀ z, r.Contains z → ¬ Stable z T := by
  classical
  intro z inside stable
  apply LearnedRegions.countermodel_row_refutes witness _ (sub_trans held inside.1) (inside.1 a extra) outside stable
  intro F root G selected
  by_cases asserted : G ∈ T
  · exact ⟨fun _ => stable.1 G asserted, fun _ => model G asserted⟩
  · have sign := knowledge F root G selected asserted z inside stable
    simpa only [decide_eq_true_eq] using sign.symm

end Zetesis.Refinement.StableProducerKnowledge
