import Zetesis.StratifiedEvaluation

/-!
# One producer pass per dependency rank

For a normal program whose positive and frozen dependencies all have smaller
rank than their producer, the truth of a whole rank can be computed from the
completed lower ranks. No positive fixed-point iteration is needed inside it.
The pass is the union of the earlier result and the filtered consequence image.

The final interpretation equals the complete stratified reduct evaluation.
Constraints then decide whether that unique interpretation is an answer set.
These laws concern ground rules and exact producer scans. Source classification,
binding enumeration, checked arithmetic, storage and Rust execution remain
separate correspondence obligations.
-/

namespace Zetesis.AcyclicEvaluation

universe u
variable {A : Type u}

/-- Every atom read by a producer has strictly smaller rank than its head.
Headless constraints are checked after evaluation and impose no rank condition. -/
def Ranked (P : Semantics.Program A) (rank : A → Nat) : Prop :=
  ∀ r ∈ P, ∀ a, r.head = some a →
    (∀ b ∈ r.positive, rank b < rank a) ∧
    (∀ b ∈ r.gateTrue, rank b < rank a) ∧
    (∀ b ∈ r.gateFalse, rank b < rank a)

/-- A strict dependency rank is also a valid stratification. -/
theorem ranked_stratified (P : Semantics.Program A) (rank : A → Nat)
    (ranked : Ranked P rank) : StratifiedEvaluation.Stratified P rank := by
  intro r member a head
  obtain ⟨positive, gates⟩ := ranked r member a head
  exact ⟨fun b present => Nat.le_of_lt (positive b present), gates⟩

/-- Publish every producer at the next rank from the already completed prefix.
All rules with a head at that rank contribute before the next rank begins. -/
def scan (P : Semantics.Program A) (rank : A → Nat) : Nat → Atoms A
  | 0 => Empty
  | n + 1 => fun a => scan P rank n a ∨
      rank a = n ∧ Semantics.Consequence P (scan P rank n) (scan P rank n) a

/-- Agreement below the producer's rank determines its entire consequence
test, including negative gates. Transfer the same producing rule and all its
body witnesses in each direction. -/
theorem consequence_agrees (P : Semantics.Program A) (rank : A → Nat)
    (ranked : Ranked P rank) (n : Nat) (X Y : Atoms A)
    (agree : StratifiedEvaluation.AgreeBelow rank n X Y) (a : A)
    (at_rank : rank a = n) :
    Semantics.Consequence P X X a ↔ Semantics.Consequence P Y Y a := by
  have transfer (U V : Atoms A)
      (same : StratifiedEvaluation.AgreeBelow rank n U V) :
      Semantics.Consequence P U U a → Semantics.Consequence P V V a := by
    rintro ⟨r, member, head, filter, gates, body⟩
    obtain ⟨positive_rank, true_rank, false_rank⟩ := ranked r member a head
    have positive : Semantics.Body r V := by
      intro b present
      have lower : rank b < n := at_rank ▸ positive_rank b present
      exact (same b lower).mp (body b present)
    have frozen : Semantics.Gate r V := by
      constructor
      · intro b present
        have lower : rank b < n := at_rank ▸ true_rank b present
        exact (same b lower).mp (gates.1 b present)
      · intro b present holds
        have lower : rank b < n := at_rank ▸ false_rank b present
        exact gates.2 b present ((same b lower).mpr holds)
    exact ⟨r, member, head, filter, frozen, positive⟩
  exact ⟨transfer X Y agree, transfer Y X (fun b lower => (agree b lower).symm)⟩

/-- After n passes, exactly the true atoms below rank n have been published.

Compare with any reduct-closure fixed point. Its closure is a fixed point of
the positive consequence operator. The induction hypothesis therefore makes
each next-rank producer test exact, while retaining the earlier prefix.
-/
theorem scan_prefix (P : Semantics.Program A) (rank : A → Nat)
    (ranked : Ranked P rank) (M : Atoms A)
    (fixed : Semantics.Gamma P M = M) (n : Nat) :
    ∀ a, scan P rank n a ↔ rank a < n ∧ M a := by
  have consequence_fixed : Semantics.Consequence P M M = M := by
    have closure_fixed := least_fixed (Semantics.consequence_mono P M)
    change Semantics.Consequence P M (Semantics.Gamma P M) =
      Semantics.Gamma P M at closure_fixed
    simpa only [fixed] using closure_fixed
  induction n with
  | zero =>
    intro a
    simp only [scan, Empty, Nat.not_lt_zero, false_and]
  | succ n previous =>
    have agree : StratifiedEvaluation.AgreeBelow rank n (scan P rank n) M := by
      intro a lower
      exact ⟨fun present => ((previous a).mp present).2,
        fun present => (previous a).mpr ⟨lower, present⟩⟩
    intro a
    constructor
    · intro present
      rcases present with earlier | ⟨at_rank, produced⟩
      · obtain ⟨lower, holds⟩ := (previous a).mp earlier
        exact ⟨Nat.lt_succ_of_lt lower, holds⟩
      · have holds : M a := by
          have at_final := (consequence_agrees P rank ranked n
            (scan P rank n) M agree a at_rank).mp produced
          simpa only [consequence_fixed] using at_final
        exact ⟨at_rank ▸ Nat.lt_succ_self n, holds⟩
    · rintro ⟨lower, holds⟩
      by_cases earlier : rank a < n
      · exact Or.inl ((previous a).mpr ⟨earlier, holds⟩)
      · have at_rank : rank a = n := by omega
        have produced : Semantics.Consequence P M M a := by
          simpa only [consequence_fixed] using holds
        exact Or.inr ⟨at_rank,
          (consequence_agrees P rank ranked n (scan P rank n) M
            agree a at_rank).mpr produced⟩

/-- A complete dependency-ordered scan equals complete stratified reduct
evaluation. The bound covers every atom, including atoms with no producer. -/
theorem scan_eq_stages (P : Semantics.Program A) (rank : A → Nat)
    (ranked : Ranked P rank) (levels : Nat) (bounded : ∀ a, rank a < levels) :
    scan P rank levels = StratifiedEvaluation.stages P levels := by
  have fixed := StratifiedEvaluation.final_fixed P rank
    (ranked_stratified P rank ranked) levels bounded
  apply funext
  intro a
  apply propext
  have completed_prefix := scan_prefix P rank ranked (StratifiedEvaluation.stages P levels)
    fixed levels a
  exact ⟨fun present => (completed_prefix.mp present).2,
    fun present => completed_prefix.mpr ⟨bounded a, present⟩⟩

/-- The complete scan is the only possible answer set; it is an answer set
exactly when the original headless constraints hold. -/
theorem normalized_answer_set_iff (P : Semantics.Program A) (rank : A → Nat)
    (ranked : Ranked P rank) (levels : Nat) (bounded : ∀ a, rank a < levels)
    (M : Atoms A) :
    Semantics.Stable P M ↔ M = scan P rank levels ∧
      Semantics.ConstraintsOK P (scan P rank levels) (scan P rank levels) := by
  rw [scan_eq_stages P rank ranked levels bounded]
  exact StratifiedEvaluation.normalized_answer_set_iff P rank
    (ranked_stratified P rank ranked) levels bounded M

/-- The same producer scan computes the unique candidate for the Ferraris
translation. Arbitrary additional constraints still filter that candidate. -/
theorem answer_set_iff (P : Semantics.Program A) (rank : A → Nat)
    (ranked : Ranked P rank) (levels : Nat) (bounded : ∀ a, rank a < levels)
    (constraints : List (Ferraris.Formula A)) (M : Atoms A) :
    Ferraris.Stable M (NormalFerraris.translate P ++ constraints.map Ferraris.Neg) ↔
      M = scan P rank levels ∧
      Semantics.ConstraintsOK P (scan P rank levels) (scan P rank levels) ∧
      Ferraris.Models (scan P rank levels) (constraints.map Ferraris.Neg) := by
  rw [scan_eq_stages P rank ranked levels bounded]
  exact StratifiedEvaluation.answer_set_iff P rank
    (ranked_stratified P rank ranked) levels bounded constraints M

end Zetesis.AcyclicEvaluation
