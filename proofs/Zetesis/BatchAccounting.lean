import Zetesis.Ferraris

/-!
# Proposed, pending and committed candidate accounting

Blocked proposals remain pending until their complete membership classification
is committed. Prefix moves preserve coverage, exact classification preserves
sound stores, and exhaustion requires both queues empty and no delayed failure.

This abstract list ledger and trusted membership contract does not refine Rust
cursor/block insertion, callback cardinality/order, allocation, machine work,
cancellation, duplicate-free semantic identity or GPU transport. Accepted values
are membership results, not proof of successful external output emission. Producer coverage remains an explicit premise.
Objective pruning must separately preserve the required result set. Original
Ferraris theory identity is unchanged throughout membership completion.
-/
namespace Zetesis.BatchAccounting
universe u v
variable {α : Type u}

structure Ledger (α : Type u) where
  queued : List α
  pending : List α
  accepted : List α
  rejected : List α
  delayedError : Bool

def initial (candidates : List α) : Ledger α := ⟨candidates, [], [], [], false⟩
def Accounted (s : Ledger α) (a : α) : Prop :=
  a ∈ s.queued ∨ a ∈ s.pending ∨ a ∈ s.accepted ∨ a ∈ s.rejected
def Covers (carrier : List α) (s : Ledger α) : Prop :=
  ∀ a, a ∈ carrier ↔ Accounted s a
def Sound (required : α → Prop) (s : Ledger α) : Prop :=
  (∀ a, a ∈ s.accepted → required a) ∧ (∀ a, a ∈ s.rejected → ¬ required a)
def Complete (s : Ledger α) : Prop :=
  s.queued = [] ∧ s.pending = [] ∧ s.delayedError = false

def propose (count : Nat) (s : Ledger α) : Ledger α :=
  { s with queued := s.queued.drop count, pending := s.pending ++ s.queued.take count }

/-- A transport failure instead retains the original ledger. This operation
    commits only a completely classified prefix, not a partially read batch. -/
def commit (count : Nat) (classify : α → Bool) (s : Ledger α) : Ledger α :=
  { s with
    pending := s.pending.drop count
    accepted := s.accepted ++ (s.pending.take count).filter classify
    rejected := s.rejected ++ (s.pending.take count).filter (fun a => !classify a) }

theorem initial_covers (candidates : List α) : Covers candidates (initial candidates) := by
  intro a
  simp [Accounted, initial]

theorem initial_sound (required : α → Prop) (candidates : List α) :
    Sound required (initial candidates) := by
  simp [Sound, initial]

theorem prefix_membership (values : List α) (count : Nat) (a : α) :
    a ∈ values ↔ a ∈ values.take count ∨ a ∈ values.drop count := by
  rw [← List.mem_append, List.take_append_drop]

theorem propose_accounted (count : Nat) (s : Ledger α) (a : α) :
    Accounted (propose count s) a ↔ Accounted s a := by
  simp only [Accounted, propose, List.mem_append]
  rw [prefix_membership s.queued count a]
  simp only [or_assoc, or_left_comm, or_comm]

/-- Total candidate occurrences, independent of semantic set membership. -/
def occurrences (s : Ledger α) : Nat :=
  s.queued.length + s.pending.length + s.accepted.length + s.rejected.length

theorem proposal_preserves_occurrences (count : Nat) (s : Ledger α) :
    occurrences (propose count s) = occurrences s := by
  have partition := congrArg List.length (List.take_append_drop count s.queued)
  simp only [List.length_append] at partition
  simp only [occurrences, propose, List.length_append]
  omega

theorem filter_partition_length (values : List α) (classify : α → Bool) :
    (values.filter classify).length + (values.filter (fun a => !classify a)).length =
      values.length := by
  induction values with
  | nil => rfl
  | cons a tail ih => cases h : classify a <;> simp [h] <;> omega

theorem commit_preserves_occurrences (count : Nat) (classify : α → Bool)
    (s : Ledger α) : occurrences (commit count classify s) = occurrences s := by
  have split := congrArg List.length (List.take_append_drop count s.pending)
  simp only [List.length_append] at split
  have partition := filter_partition_length (s.pending.take count) classify
  simp only [occurrences, commit, List.length_append]
  omega

theorem proposal_preserves_coverage (count : Nat) (s : Ledger α)
    (carrier : List α) (covered : Covers carrier s) :
    Covers carrier (propose count s) := by
  intro a
  exact (covered a).trans (propose_accounted count s a).symm

theorem proposal_preserves_soundness (count : Nat) (s : Ledger α)
    (required : α → Prop) (sound : Sound required s) :
    Sound required (propose count s) := sound

theorem commit_accounted (count : Nat) (classify : α → Bool)
    (s : Ledger α) (a : α) : Accounted (commit count classify s) a ↔ Accounted s a := by
  simp only [Accounted, commit, List.mem_append, List.mem_filter]
  rw [prefix_membership s.pending count a]
  cases classify a <;> simp [or_assoc, or_left_comm, or_comm]

theorem commit_preserves_coverage (count : Nat) (classify : α → Bool)
    (s : Ledger α) (carrier : List α) (covered : Covers carrier s) :
    Covers carrier (commit count classify s) := by
  intro a
  exact (covered a).trans (commit_accounted count classify s a).symm

theorem exact_commit_preserves_soundness (count : Nat) (classify : α → Bool)
    (s : Ledger α) (required : α → Prop) (sound : Sound required s)
    (exact : ∀ a, a ∈ s.pending.take count → (classify a = true ↔ required a)) :
    Sound required (commit count classify s) := by
  constructor
  · intro a member
    obtain old | fresh := List.mem_append.mp member
    · exact sound.1 a old
    · obtain ⟨inside, yes⟩ := List.mem_filter.mp fresh
      exact (exact a inside).mp yes
  · intro a member valid
    obtain old | fresh := List.mem_append.mp member
    · exact sound.2 a old valid
    · obtain ⟨inside, no⟩ := List.mem_filter.mp fresh
      have yes := (exact a inside).mpr valid
      simp [yes] at no

theorem completed_results_exact (carrier : List α) (s : Ledger α)
    (required : α → Prop) (covered : Covers carrier s)
    (sound : Sound required s) (done : Complete s) (a : α) :
    a ∈ s.accepted ↔ a ∈ carrier ∧ required a := by
  constructor
  · intro accepted
    exact ⟨(covered a).mpr (Or.inr (Or.inr (Or.inl accepted))), sound.1 a accepted⟩
  · rintro ⟨inside, valid⟩
    have accounted := (covered a).mp inside
    simp only [Accounted, done.1, done.2.1, List.not_mem_nil, false_or] at accounted
    exact accounted.resolve_right (fun rejected => sound.2 a rejected valid)

theorem pending_prevents_exhaustion (s : Ledger α) (a : α)
    (pending : a ∈ s.pending) : ¬ Complete s := by
  intro done
  rw [done.2.1] at pending
  exact List.not_mem_nil pending

theorem delayed_error_prevents_exhaustion (s : Ledger α)
    (delayed : s.delayedError = true) : ¬ Complete s := by
  intro done
  have impossible := done.2.2
  rw [delayed] at impossible
  contradiction

theorem empty_pending_is_not_exhaustion :
    (initial [()]).pending = [] ∧ ¬ Complete (initial [()]) := by
  simp [initial, Complete]

inductive PartialVerdict where
  | noProperSubset | residual | notModel

def VerdictSound {β : Type v} (T : Ferraris.Theory β) (M : Atoms β) :
    PartialVerdict → Prop
  | .noProperSubset => Ferraris.Stable M T
  | .residual => True
  | .notModel => ¬ Ferraris.Models M T

def completeVerdict (exactNative : Bool) : PartialVerdict → Bool
  | .noProperSubset => true
  | .residual => exactNative
  | .notModel => false

theorem hybrid_completion_exact {β : Type v} (T : Ferraris.Theory β)
    (M : Atoms β) (verdict : PartialVerdict) (native : Bool)
    (sound : VerdictSound T M verdict)
    (exact : native = true ↔ Ferraris.Stable M T) :
    completeVerdict native verdict = true ↔ Ferraris.Stable M T := by
  cases verdict with
  | noProperSubset => simp [completeVerdict, VerdictSound] at *; exact sound
  | residual => exact exact
  | notModel =>
    simp only [completeVerdict, Bool.false_eq_true, false_iff]
    exact fun stable => sound stable.1

end Zetesis.BatchAccounting
