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
-/

namespace Zetesis.GatePositions

variable {Atom : Type}

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
