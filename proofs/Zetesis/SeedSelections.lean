import Zetesis.Core

/-!
# Shared selections and owned seed interpretations

A finite selection contains handles whose immutable denotations are logical
atoms. Materialization orders those denotations and removes duplicates. The
result denotes exactly the same true set, even when distinct handles denote
the same atom; every atom absent from the selection remains false.

The permutation premise isolates the ordering implementation. Rust canonical
comparison, Arc lifetimes and allocation, program-instance validation, binary
search and BTreeSet construction remain executable refinement obligations.
This module establishes interpretation equality, not stable membership or a
new candidate-coverage theorem.
-/

namespace Zetesis.SeedSelections

universe u v
variable {Handle : Type u} {Atom : Type v}

/-- A handle denotes an immutable atom; no injectivity assumption is needed. -/
def selected (denote : Handle → Atom) (handles : List Handle) : Atoms Atom :=
  fun atom => ∃ handle ∈ handles, denote handle = atom

/-- A canonically ordered owned list uses set membership, coalescing duplicates. -/
def owned [BEq Atom] (ordered : List Atom) : Atoms Atom :=
  fun atom => atom ∈ ordered.eraseDups

/-- Ordering and coalescing materialized atom identities preserves the complete
selection interpretation. First deduplication preserves list membership; the
ordering permutation then reduces membership to a selected handle's denotation.
Distinct equal-valued handles need not have equal identities. -/
theorem materialization_exact [BEq Atom] [LawfulBEq Atom]
    (denote : Handle → Atom) (handles : List Handle) (ordered : List Atom)
    (ordered_values : ordered.Perm (handles.map denote)) :
    owned ordered = selected denote handles := by
  apply atoms_ext
  intro atom
  have order_preserves_membership : atom ∈ ordered ↔ atom ∈ handles.map denote :=
    ordered_values.mem_iff
  simpa only [owned, selected, List.mem_eraseDups, List.mem_map] using
    order_preserves_membership

end Zetesis.SeedSelections
