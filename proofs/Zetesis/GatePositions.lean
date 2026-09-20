import Zetesis.OptionalIndex

/-!
# Gate-carrier positions in a complete atom carrier

A complete carrier assigns dense positions in its canonical order. Retaining
only gate atoms leaves their relative order unchanged. Consequently, the nth
retained gate atom and the nth retained dense position still name the same atom.
This is the correspondence used to avoid resolving that atom by value repeatedly.

The list result needs no ordering or uniqueness premise: filtering preserves
occurrences even when a list has duplicates. Rust additionally establishes that
Program's gate predicate list is a sorted subset of its full predicate list,
both iterators use the same sorted domain and tuple order, and complete graph
construction retains precisely that subsequence. Program instance checks keep
unrelated carriers apart. Those Rust properties, opaque token construction,
allocation, machine-word limits and bit packing are not proved here. The checked
positive position representation reuses OptionalIndex's successor laws.

The mixed-radix laws also relate a left-to-right digit fold to the sum of
lexicographic block offsets, and bound that sum by the tuple cardinality.
They do not prove that Rust's value comparison and binary search supply those
digits, or that its checked arithmetic implements natural-number arithmetic.
-/

namespace Zetesis.GatePositions

variable {Atom : Type}

/-- The offset contributed by each coordinate when the last coordinate varies
fastest. A coordinate skips one block of suffix tuples per preceding digit. -/
def blockOffset (radix : Nat) : List Nat → Nat
  | [] => 0
  | digit :: tail => digit * radix ^ tail.length + blockOffset radix tail

/-- Accumulating domain ranks from left to right computes the same offset as
skipping lexicographic blocks. An existing prefix contributes one whole tuple
cardinality per prefix position. This identity holds even for radix zero;
validity of the supplied digits is needed only for the range theorem below.

The induction peels off the leading digit, applies the tail identity to the
updated prefix, and distributes multiplication over that update. -/
theorem rank_fold_eq_blocks (radix : Nat) (digits : List Nat) (initial : Nat) :
    digits.foldl (fun rank digit => rank * radix + digit) initial =
      initial * radix ^ digits.length + blockOffset radix digits := by
  induction digits generalizing initial with
  | nil => simp [blockOffset]
  | cons digit tail tail_rank =>
    simp only [List.foldl_cons, List.length_cons, blockOffset]
    rw [tail_rank]
    simp only [Nat.pow_succ, Nat.add_mul, Nat.mul_assoc, Nat.mul_comm radix,
      Nat.add_assoc]

/-- Valid domain digits name a position strictly inside the complete tuple
carrier. The empty tuple has position zero in a carrier of size one, including
an empty domain. A nonempty tuple over that domain has no valid digits.

Each valid leading digit leaves a full suffix block above the suffix offset;
the digit bound then places that block within the complete carrier. -/
theorem blockOffset_lt_cardinality (radix : Nat) (digits : List Nat)
    (valid : ∀ digit ∈ digits, digit < radix) :
    blockOffset radix digits < radix ^ digits.length := by
  induction digits with
  | nil => simp [blockOffset]
  | cons digit tail tail_bound =>
    have digit_valid : digit < radix := valid digit (List.mem_cons_self ..)
    have tail_valid : ∀ value ∈ tail, value < radix := by
      intro value member
      exact valid value (List.mem_cons_of_mem digit member)
    have suffix_inside : blockOffset radix tail < radix ^ tail.length :=
      tail_bound tail_valid
    calc
      blockOffset radix (digit :: tail)
          < digit * radix ^ tail.length + radix ^ tail.length :=
        Nat.add_lt_add_left suffix_inside _
      _ = (digit + 1) * radix ^ tail.length := (Nat.succ_mul ..).symm
      _ ≤ radix * radix ^ tail.length :=
        Nat.mul_le_mul_right _ digit_valid
      _ = radix ^ (digit :: tail).length := by
        simp only [List.length_cons, Nat.pow_succ, Nat.mul_comm]

/-- Gate atoms paired with their original dense positions, in carrier order. -/
def entries (keep : Atom → Bool) (carrier : List Atom) : List (Atom × Nat) :=
  carrier.zipIdx.filter (fun entry => keep entry.1)

/-- Projecting the retained pairs gives exactly the gate subsequence. Filtering
commutes with projecting the atom, and projecting indexed atoms restores the
original carrier. No atom is reordered or newly introduced. -/
theorem atoms_exact (keep : Atom → Bool) (carrier : List Atom) :
    (entries keep carrier).map Prod.fst = carrier.filter keep := by
  have projection_commutes :=
    (List.filter_map (f := Prod.fst) (p := keep) (l := carrier.zipIdx)).symm
  simpa [entries, Function.comp_def, List.zipIdx_map_fst] using projection_commutes

/-- A retained entry's atom is at its original dense position and at its gate
rank in the filtered carrier. First membership in the indexed carrier validates
the dense lookup; projecting the exact retained occurrence validates gate rank.
This relates positions, not answer-set membership or executable refinement. -/
theorem retained_position_exact (keep : Atom → Bool) (carrier : List Atom)
    (rank dense : Nat) (atom : Atom)
    (retained : (entries keep carrier)[rank]? = some (atom, dense)) :
    carrier[dense]? = some atom ∧ (carrier.filter keep)[rank]? = some atom := by
  have member : (atom, dense) ∈ entries keep carrier := List.mem_of_getElem? retained
  have original_member : (atom, dense) ∈ carrier.zipIdx :=
    (List.mem_filter.mp member).1
  have dense_lookup : carrier[dense]? = some atom :=
    List.mk_mem_zipIdx_iff_getElem?.mp original_member
  have projected_lookup : ((entries keep carrier).map Prod.fst)[rank]? = some atom := by
    simp only [List.getElem?_map, retained, Option.map_some]
  have gate_lookup : (carrier.filter keep)[rank]? = some atom := by
    simpa only [atoms_exact] using projected_lookup
  exact ⟨dense_lookup, gate_lookup⟩

end Zetesis.GatePositions
