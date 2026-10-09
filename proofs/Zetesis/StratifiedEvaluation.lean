import Zetesis.NormalFerraris
import Zetesis.ConstrainedPositive

/-!
# Stratified evaluation through complete reduct closures

A rank assigns each atom a natural-number stratum. Positive dependencies may
stay in the producer's stratum; frozen gates must refer to lower strata.
Constraints produce no atoms and impose no rank conditions.

Starting from the empty interpretation, each stage computes the complete least
closure of the reduct frozen by the previous stage. After n stages, atoms below
rank n have their final truth. A bound on all atom ranks therefore gives a unique
fixed point. The normalized/Ferraris correspondence identifies that fixed point,
when it satisfies the constraints, with the program's answer set. Additional
Ferraris constraints may have arbitrary bodies.

This is a denotational staged algorithm. Each Gamma call includes complete
positive closure, including positive cycles; a single rule scan is insufficient.
The laws do not establish source normalization, recognition of a rank from a
dependency graph, equivalence of an SCC-local queue implementation, finite
closure execution, storage accounting or Rust refinement.
-/

namespace Zetesis.StratifiedEvaluation

universe u
variable {A : Type u}

/-- Every producer reads positive atoms no higher than its head and frozen gates
strictly below it. Headless constraints do not enter the consequence operator.
Both gate polarities are covered; ordinary default negation uses gateFalse. -/
def Stratified (P : Semantics.Program A) (rank : A → Nat) : Prop :=
  ∀ r ∈ P, ∀ a, r.head = some a →
    (∀ b ∈ r.positive, rank b ≤ rank a) ∧
    (∀ b ∈ r.gateTrue, rank b < rank a) ∧
    (∀ b ∈ r.gateFalse, rank b < rank a)

/-- Agreement on the already determined lower strata. -/
def AgreeBelow (rank : A → Nat) (n : Nat) (M N : Atoms A) : Prop :=
  ∀ a, rank a < n → (M a ↔ N a)

/-- Freeze gates at the preceding stage, then take a complete least closure.
Stages need not be globally increasing: higher, not-yet-determined atoms can
change truth while lower strata settle. -/
def stages (P : Semantics.Program A) : Nat → Atoms A
  | 0 => Empty
  | n + 1 => Semantics.Gamma P (stages P n)

/-- If the gate interpretations agree below n, every consequence below n+1
under the first also belongs to the second's complete closure.

Use a closed interpretation containing the second closure on the selected
prefix and every atom above it. A producer in the prefix reads positive atoms
inside the prefix and gates strictly below n, so its head is already in the
second closure. Leastness then gives the required containment.
-/
theorem gamma_prefix_sub (P : Semantics.Program A) (rank : A → Nat)
    (stratified : Stratified P rank) (n : Nat) (M N : Atoms A)
    (agree : AgreeBelow rank n M N) :
    ∀ a, rank a < n + 1 → Semantics.Gamma P M a → Semantics.Gamma P N a := by
  let upper : Atoms A := fun a => rank a < n + 1 → Semantics.Gamma P N a
  have upper_closed : Closed (Semantics.Consequence P M) upper := by
    intro a produced below
    obtain ⟨r, member, head, filter, gates, body⟩ := produced
    obtain ⟨positive_rank, true_rank, false_rank⟩ := stratified r member a head
    have transferred_gates : Semantics.Gate r N := by
      constructor
      · intro b present
        have lower : rank b < n := by
          have earlier := true_rank b present
          omega
        exact (agree b lower).mp (gates.1 b present)
      · intro b present holds
        have lower : rank b < n := by
          have earlier := false_rank b present
          omega
        exact gates.2 b present ((agree b lower).mpr holds)
    have transferred_body : Semantics.Body r (Semantics.Gamma P N) := by
      intro b present
      have lower : rank b < n + 1 := by
        have earlier := positive_rank b present
        omega
      exact body b present lower
    exact Semantics.gamma_closed P N a
      ⟨r, member, head, filter, transferred_gates, transferred_body⟩
  intro a below present
  exact Semantics.gamma_le upper_closed a present below

/-- Equal completed lower strata determine the next stratum's least closure.
Apply prefix containment in both directions; this needs no acyclic assumption
on positive dependencies within a stratum. -/
theorem gamma_agrees (P : Semantics.Program A) (rank : A → Nat)
    (stratified : Stratified P rank) (n : Nat) (M N : Atoms A)
    (agree : AgreeBelow rank n M N) :
    AgreeBelow rank (n + 1) (Semantics.Gamma P M) (Semantics.Gamma P N) := by
  intro a below
  exact ⟨gamma_prefix_sub P rank stratified n M N agree a below,
    gamma_prefix_sub P rank stratified n N M
      (fun b lower => (agree b lower).symm) a below⟩

/-- After n closures, every atom below rank n retains its truth at the next
stage. The empty prefix starts the induction; complete closure advances it. -/
theorem consecutive_agree (P : Semantics.Program A) (rank : A → Nat)
    (stratified : Stratified P rank) (n : Nat) :
    AgreeBelow rank n (stages P n) (stages P (n + 1)) := by
  induction n with
  | zero =>
    intro a below
    exact False.elim (Nat.not_lt_zero _ below)
  | succ n previous =>
    exact gamma_agrees P rank stratified n (stages P n) (stages P (n + 1)) previous

/-- A finite bound on atom ranks makes the final stage a fixed point of the
actual reduct-closure operator. This is derived from rank conditions, not
assumed as an evaluator postcondition. -/
theorem final_fixed (P : Semantics.Program A) (rank : A → Nat)
    (stratified : Stratified P rank) (levels : Nat)
    (bounded : ∀ a, rank a < levels) :
    Semantics.Gamma P (stages P levels) = stages P levels := by
  apply funext
  intro a
  exact propext ((consecutive_agree P rank stratified levels a (bounded a)).symm)

/-- Every reduct-closure fixed point agrees with stage n below rank n.
The same rank induction used for existence also proves uniqueness. -/
theorem stages_agree_fixed (P : Semantics.Program A) (rank : A → Nat)
    (stratified : Stratified P rank) (M : Atoms A)
    (fixed : Semantics.Gamma P M = M) (n : Nat) :
    AgreeBelow rank n (stages P n) M := by
  induction n with
  | zero =>
    intro a below
    exact False.elim (Nat.not_lt_zero _ below)
  | succ n previous =>
    have next : AgreeBelow rank (n + 1)
        (Semantics.Gamma P (stages P n)) (Semantics.Gamma P M) :=
      gamma_agrees P rank stratified n (stages P n) M previous
    simpa only [stages, fixed] using next

/-- The complete staged result is the only reduct-closure fixed point. -/
theorem fixed_iff_final (P : Semantics.Program A) (rank : A → Nat)
    (stratified : Stratified P rank) (levels : Nat)
    (bounded : ∀ a, rank a < levels) (M : Atoms A) :
    Semantics.Gamma P M = M ↔ M = stages P levels := by
  constructor
  · intro fixed
    apply funext
    intro a
    exact propext ((stages_agree_fixed P rank stratified M fixed levels
      a (bounded a)).symm)
  · intro equality
    subst M
    exact final_fixed P rank stratified levels bounded

/-- The normalized program has precisely the staged answer if its constraints
hold there, and no answer otherwise. Existing reduct minimality supplies the
link from the unique fixed point to answer-set membership. -/
theorem normalized_answer_set_iff (P : Semantics.Program A) (rank : A → Nat)
    (stratified : Stratified P rank) (levels : Nat)
    (bounded : ∀ a, rank a < levels) (M : Atoms A) :
    Semantics.Stable P M ↔ M = stages P levels ∧
      Semantics.ConstraintsOK P (stages P levels) (stages P levels) := by
  rw [Semantics.stable_iff_gamma, fixed_iff_final P rank stratified levels bounded M]
  constructor
  · rintro ⟨equality, constraints⟩
    exact ⟨equality, equality ▸ constraints⟩
  · rintro ⟨equality, constraints⟩
    exact ⟨equality, equality.symm ▸ constraints⟩

/-- The Ferraris translation, followed by arbitrary original constraints, has
exactly the staged answer satisfying all constraints.

The proof first separates constraint filtering from producer membership. The
existing normalized/Ferraris translation law then transfers the rank theorem
to original satisfaction and proper-subset frozen-reduct minimality. No
monotonicity or stratification restriction is imposed on constraint bodies.
-/
theorem answer_set_iff (P : Semantics.Program A) (rank : A → Nat)
    (stratified : Stratified P rank) (levels : Nat)
    (bounded : ∀ a, rank a < levels) (constraints : List (Ferraris.Formula A))
    (M : Atoms A) :
    Ferraris.Stable M
        (NormalFerraris.translate P ++ constraints.map Ferraris.Neg) ↔
      M = stages P levels ∧
      Semantics.ConstraintsOK P (stages P levels) (stages P levels) ∧
      Ferraris.Models (stages P levels) (constraints.map Ferraris.Neg) := by
  rw [ConstrainedPositive.stable_append_constraints,
    ← NormalFerraris.answer_set_iff,
    normalized_answer_set_iff P rank stratified levels bounded M]
  constructor
  · rintro ⟨⟨equality, original_constraints⟩, added_constraints⟩
    exact ⟨equality, original_constraints, equality ▸ added_constraints⟩
  · rintro ⟨equality, original_constraints, added_constraints⟩
    exact ⟨⟨equality, original_constraints⟩, equality.symm ▸ added_constraints⟩

end Zetesis.StratifiedEvaluation
