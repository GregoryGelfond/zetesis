import Zetesis.ModelSelections

/-!
# Append-only atom identities

A catalog owns a finite sequence of atoms. During a synchronous grounding round,
its committed prefix is borrowed while new identities enter a disjoint pending
suffix. Committing moves the suffix after the prefix without renumbering either.
Selections, rather than catalog presence, determine the current interpretation.

These laws describe that representation. Uniqueness is a premise for unique IDs;
arbitrary immutable catalogs may contain equal atoms at distinct positions.
Index balancing, checked comparisons, Rust borrows, allocation and resource stops
remain executable refinement obligations. No law here establishes satisfaction
or answer-set membership.
-/

namespace Zetesis.AtomCatalogs

universe u
variable {Atom : Type u}

/-- Decode one local position while payload storage is split into two regions. -/
def lookup (committed pending : List Atom) (position : Nat) : Option Atom :=
  if position < committed.length then committed[position]?
  else pending[position - committed.length]?

/-- Moving the pending region after the committed prefix preserves every lookup.
The two cases are precisely the two branches of finite-list append lookup. -/
theorem commit_preserves_lookup (committed pending : List Atom) (position : Nat) :
    (committed ++ pending)[position]? = lookup committed pending position := by
  exact List.getElem?_append

/-- A position in the committed prefix retains its atom after any append. -/
theorem append_preserves_identity (committed pending : List Atom) (position : Nat)
    (present : position < committed.length) :
    lookup committed pending position = committed[position]? := by
  simp only [lookup, if_pos present]

/-- Committing preserves the interpretation of any selected positions, including
positions discovered during the current round. The pointwise lookup equality
supplies the existing selection-preservation law. -/
theorem commit_preserves_interpretation (committed pending : List Atom)
    (positions : List Nat) :
    ModelSelections.denotes (fun position => (committed ++ pending)[position]?) positions =
      ModelSelections.denotes (lookup committed pending) positions := by
  apply ModelSelections.unselected_entries_irrelevant
  intro position _selected
  exact commit_preserves_lookup committed pending position

/-- New catalog identities do not change an interpretation selected entirely
from the old prefix. Truth requires selection; discovery alone supplies none. -/
theorem discovery_preserves_interpretation (committed pending : List Atom)
    (positions : List Nat) (old : ∀ position ∈ positions, position < committed.length) :
    ModelSelections.denotes (lookup committed pending) positions =
      ModelSelections.denotes (fun position => committed[position]?) positions := by
  apply ModelSelections.unselected_entries_irrelevant
  intro position selected
  exact append_preserves_identity committed pending position (old position selected)

/-- In a unique catalog, two successful equal lookups have the same local ID.
The premise concerns complete atom equality, not hashes or source spellings. -/
theorem identity_determines_position (committed pending : List Atom)
    (unique : (committed ++ pending).Nodup) (first second : Nat) (atom : Atom)
    (left : lookup committed pending first = some atom)
    (right : lookup committed pending second = some atom) : first = second := by
  have first_present : (committed ++ pending)[first]? = some atom :=
    (commit_preserves_lookup committed pending first).trans left
  have in_bounds : first < (committed ++ pending).length :=
    List.getElem?_eq_some_iff.mp first_present |>.choose
  have same : (committed ++ pending)[first]? = (committed ++ pending)[second]? := by
    rw [commit_preserves_lookup, commit_preserves_lookup, left, right]
  exact (List.getElem?_inj in_bounds unique).mp same

end Zetesis.AtomCatalogs
