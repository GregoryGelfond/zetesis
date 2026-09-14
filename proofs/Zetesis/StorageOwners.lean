import Std.Tactic

/-!
# Storage reserved for independent owners

Each active scalar closure has one named-storage allowance. A batch reserves
that allowance for every worker which can be active. Summing those allowances
bounds the simultaneously owned storage when each closure respects its bound.

The statements use natural-number bytes. They do not prove the Rust capacity
measure, allocation/overflow handling, worker count or release lifetime. Input
seeds, retained answers and allocator overhead are separate owners; this is not
a bound on process memory or a theorem about answer-set membership.
-/

namespace Zetesis.StorageOwners

/-- The sum of finite owner sizes is bounded by one allowance per owner.
Induct on the owners: add the head's bound to the tail's reserved capacity. -/
theorem sum_within_reservations (owners : List Nat) (allowance : Nat)
    (bounded : ∀ bytes ∈ owners, bytes ≤ allowance) :
    owners.sum ≤ owners.length * allowance := by
  induction owners with
  | nil => simp
  | cons bytes tail induction =>
    have head_bound : bytes ≤ allowance := bounded bytes (by simp)
    have tail_bound : tail.sum ≤ tail.length * allowance := by
      apply induction
      intro bytes member
      exact bounded bytes (List.mem_cons_of_mem _ member)
    simpa [List.sum_cons, List.length_cons, Nat.succ_mul, Nat.add_comm] using
      Nat.add_le_add head_bound tail_bound

/-- A finite worker reservation covers every concurrently active closure when
the owner count and each owner's capacity respect their independent bounds. -/
theorem active_storage_within_limit (owners : List Nat)
    (workers allowance limit : Nat)
    (counted : owners.length ≤ workers)
    (bounded : ∀ bytes ∈ owners, bytes ≤ allowance)
    (admitted : workers * allowance ≤ limit) : owners.sum ≤ limit := by
  have reserved : owners.sum ≤ owners.length * allowance :=
    sum_within_reservations owners allowance bounded
  have covered : owners.length * allowance ≤ workers * allowance :=
    Nat.mul_le_mul_right allowance counted
  exact Nat.le_trans reserved (Nat.le_trans covered admitted)

end Zetesis.StorageOwners
