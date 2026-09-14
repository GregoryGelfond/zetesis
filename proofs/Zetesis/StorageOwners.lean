import Std.Tactic

/-!
# Storage reserved for independent owners

Each active scalar closure has one named-storage allowance. A batch reserves
that allowance for every worker which can be active. Summing those allowances
bounds the simultaneously owned storage when each closure respects its bound.
Owners can also have different reservations. The componentwise sum law handles
those pairs; the cache corollary counts one shared owner, idle retained owners
and active owners bounded by the larger of retained size and assigned allowance.

The statements use natural-number bytes. They do not prove the Rust capacity
measure, allocation/overflow handling, worker count or release lifetime. To
interpret a sum as total named storage, the consumer must cover all such storage
with disjoint owner entries and count the shared component once. Input seeds,
retained answers and allocator overhead are separate owners. The explicit
capacity premises do not assert a hard allocator or process-memory bound, and
none of these statements establishes answer-set membership.
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

/-- Pointwise reservations bound the sum of a finite family of owner sizes.
`actual` and `reservation` name the two sizes for each supplied owner occurrence;
unlike a uniform worker allowance, the reservations may differ.

Induct on the list. The head's explicit bound and the tail's summed bound add to
the desired inequality. Coverage and disjointness of physical owners are not
properties of these natural numbers and remain the consumer's obligation. -/
theorem sum_within_component_bounds {Owner : Type} (owners : List Owner)
    (actual reservation : Owner → Nat)
    (bounded : ∀ owner ∈ owners, actual owner ≤ reservation owner) :
    (owners.map actual).sum ≤ (owners.map reservation).sum := by
  induction owners with
  | nil => simp
  | cons owner tail induction =>
    have head_bound : actual owner ≤ reservation owner := bounded owner (by simp)
    have tail_bound : (tail.map actual).sum ≤ (tail.map reservation).sum := by
      apply induction
      intro owner member
      exact bounded owner (List.mem_cons_of_mem _ member)
    simpa only [List.map_cons, List.sum_cons] using
      Nat.add_le_add head_bound tail_bound

/-- A shared, idle and active reservation covers their total named storage.
Each pair records actual bytes first and retained bytes second. Idle owners
respect their retained bounds; active owners respect the larger of retained
bytes and the assigned allowance. The shared owner has its own explicit bound.
The admitted reservation counts that shared owner once, then all idle bounds
and active maxima. Empty lists need no separate case or candidate allowance.

Sum the two pointwise families using `sum_within_component_bounds`, add their
bounds to the shared bound, then use the admitted collective ceiling. For the
prepared cache, `allowance` is the per-candidate limit after checked subtraction
of its shared preparation header. This theorem does not perform that subtraction
or establish concrete allocation, failure, scheduling or lifetime premises. -/
theorem shared_idle_active_within_limit
    (sharedActual sharedReservation : Nat) (idle active : List (Nat × Nat))
    (allowance limit : Nat)
    (shared_bounded : sharedActual ≤ sharedReservation)
    (idle_bounded : ∀ owner ∈ idle, owner.1 ≤ owner.2)
    (active_bounded : ∀ owner ∈ active, owner.1 ≤ max owner.2 allowance)
    (admitted : sharedReservation + (idle.map Prod.snd).sum +
      (active.map (fun owner => max owner.2 allowance)).sum ≤ limit) :
    sharedActual + (idle.map Prod.fst).sum + (active.map Prod.fst).sum ≤ limit := by
  have idle_bound : (idle.map Prod.fst).sum ≤ (idle.map Prod.snd).sum :=
    sum_within_component_bounds idle Prod.fst Prod.snd idle_bounded
  have active_bound : (active.map Prod.fst).sum ≤
      (active.map (fun owner => max owner.2 allowance)).sum :=
    sum_within_component_bounds active Prod.fst
      (fun owner => max owner.2 allowance) active_bounded
  have total_bound :
      sharedActual + (idle.map Prod.fst).sum + (active.map Prod.fst).sum ≤
      sharedReservation + (idle.map Prod.snd).sum +
        (active.map (fun owner => max owner.2 allowance)).sum :=
    Nat.add_le_add (Nat.add_le_add shared_bounded idle_bound) active_bound
  exact Nat.le_trans total_bound admitted

end Zetesis.StorageOwners
