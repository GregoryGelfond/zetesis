import Zetesis.AtomCatalogs

/-!
# Canonical atoms and dense occurrences

A canonical catalog decodes atom identities. A separate occurrence map preserves
original dense positions, including distinct positions naming the same identity.
Selecting occurrences denotes the same interpretation as selecting their decoded
identities; ordering and coalescing those identities do not change truth.

Appending atoms preserves old selections under an explicit prefix bound. Moving
identities between owners instead requires agreement of their decoded atoms.
An equal raw ID supplies no such agreement. These laws reuse the interpretation
of `ModelSelections`; they do not establish satisfaction or answer-set membership.
Rust bounds checks, interning uniqueness, snapshot lifetimes, allocation and
resource accounting remain implementation obligations.
-/

namespace Zetesis.CanonicalCatalog

universe u v w x
variable {Slot : Type u} {Id : Type v} {OtherId : Type w} {Atom : Type x}

/-- Decode an original occurrence through its canonical atom identity.
Missing occurrences and missing canonical identities both remain absent. -/
def lookup (decode : Id → Option Atom) (occurrences : Slot → Option Id)
    (position : Slot) : Option Atom :=
  (occurrences position).bind decode

/-- Selecting original occurrences is equivalent to selecting their canonical
identities. No injectivity premise is required: several positions may name one
identity. Each direction retains the original occurrence witnessing membership. -/
theorem selection_exact (decode : Id → Option Atom)
    (occurrences : Slot → Option Id) (positions : List Slot) :
    ModelSelections.denotes decode (positions.filterMap occurrences) =
      ModelSelections.denotes (lookup decode occurrences) positions := by
  apply atoms_ext
  intro atom
  constructor
  · rintro ⟨identity, selected, decoded⟩
    obtain ⟨position, present, mapped⟩ := List.mem_filterMap.mp selected
    have occurrence_decodes : lookup decode occurrences position = some atom := by
      simp only [lookup, mapped, Option.bind_some, decoded]
    exact ⟨position, present, occurrence_decodes⟩
  · rintro ⟨position, selected, decoded⟩
    cases mapped : occurrences position with
    | none =>
      have missing : (none : Option Atom) = some atom := by
        simpa only [lookup, mapped, Option.bind_none] using decoded
      cases missing
    | some identity =>
      have identity_decodes : decode identity = some atom := by
        simpa only [lookup, mapped, Option.bind_some] using decoded
      exact ⟨identity, List.mem_filterMap.mpr ⟨position, selected, mapped⟩,
        identity_decodes⟩

/-- Ordering and coalescing selected canonical identities preserves the original
occurrence interpretation. The permutation premise specifies coverage, not a
numeric ID order. Duplicate occurrences therefore need no duplicate atom payload.

First preserve identity-list membership, then apply `selection_exact`.
-/
theorem normalized_selection_exact [BEq Id] [LawfulBEq Id]
    (decode : Id → Option Atom) (occurrences : Slot → Option Id)
    (positions : List Slot) (ordered : List Id)
    (order : ordered.Perm (positions.filterMap occurrences)) :
    ModelSelections.denotes decode ordered.eraseDups =
      ModelSelections.denotes (lookup decode occurrences) positions := by
  have same_members : ∀ identity,
      identity ∈ ordered.eraseDups ↔ identity ∈ positions.filterMap occurrences := by
    intro identity
    have same_order : identity ∈ ordered ↔ identity ∈ positions.filterMap occurrences :=
      order.mem_iff
    simpa only [List.mem_eraseDups] using same_order
  have same_selection : ModelSelections.denotes decode ordered.eraseDups =
      ModelSelections.denotes decode (positions.filterMap occurrences) := by
    apply atoms_ext
    intro atom
    constructor
    · rintro ⟨identity, selected, decoded⟩
      exact ⟨identity, (same_members identity).mp selected, decoded⟩
    · rintro ⟨identity, selected, decoded⟩
      exact ⟨identity, (same_members identity).mpr selected, decoded⟩
  exact same_selection.trans (selection_exact decode occurrences positions)

/-- Appending canonical atoms preserves an interpretation whose mapped selected
IDs lie in the old prefix. Discovery alone adds no truth. The prefix premise
excludes an invalid old ID becoming a newly valid selected atom after append.

At each selected occurrence, the existing append law preserves its decode;
agreement at selected positions then preserves the interpretation.
-/
theorem append_preserves_interpretation (committed pending : List Atom)
    (occurrences : Slot → Option Nat) (positions : List Slot)
    (old : ∀ position ∈ positions, ∀ identity,
      occurrences position = some identity → identity < committed.length) :
    ModelSelections.denotes
        (lookup (fun identity => (committed ++ pending)[identity]?) occurrences) positions =
      ModelSelections.denotes
        (lookup (fun identity => committed[identity]?) occurrences) positions := by
  apply ModelSelections.unselected_entries_irrelevant
  intro position selected
  cases mapped : occurrences position with
  | none => simp only [lookup, mapped, Option.bind_none]
  | some identity =>
    have in_prefix : identity < committed.length := old position selected identity mapped
    have same_decode : (committed ++ pending)[identity]? = committed[identity]? := by
      exact (AtomCatalogs.commit_preserves_lookup committed pending identity).trans
        (AtomCatalogs.append_preserves_identity committed pending identity in_prefix)
    simpa only [lookup, mapped, Option.bind_some] using same_decode

/-- Re-encoding selected identities for another owner preserves interpretation
when each replacement decodes to the same optional atom. The owners may use
different ID types or assign different atoms to equal raw IDs. Decoded agreement,
not raw equality or a shared shape, supplies the correspondence.
-/
theorem owner_transfer_preserves_interpretation
    (before : Id → Option Atom) (after : OtherId → Option Atom)
    (occurrences : Slot → Option Id) (positions : List Slot) (transfer : Id → OtherId)
    (preserves : ∀ position ∈ positions, ∀ identity,
      occurrences position = some identity → after (transfer identity) = before identity) :
    ModelSelections.denotes
        (lookup after (fun position => (occurrences position).map transfer)) positions =
      ModelSelections.denotes (lookup before occurrences) positions := by
  apply ModelSelections.unselected_entries_irrelevant
  intro position selected
  cases mapped : occurrences position with
  | none => simp only [lookup, mapped, Option.map_none, Option.bind_none]
  | some identity =>
    have same_decode : after (transfer identity) = before identity :=
      preserves position selected identity mapped
    simpa only [lookup, mapped, Option.map_some, Option.bind_some] using same_decode

/-- Identical raw selection lists can denote different interpretations under
different owner decoders. This concrete collision explains why owner transfer
requires its decoded-agreement premise. -/
theorem raw_ids_do_not_determine_interpretation :
    ModelSelections.denotes (fun _ : Nat => some false) [0] ≠
      ModelSelections.denotes (fun _ : Nat => some true) [0] := by
  intro same
  have present : ModelSelections.denotes (fun _ : Nat => some false) [0] false := by
    exact ⟨0, by simp, rfl⟩
  have absent : ¬ ModelSelections.denotes (fun _ : Nat => some true) [0] false := by
    rintro ⟨_position, _selected, decoded⟩
    cases decoded
  exact absent (same ▸ present)

end Zetesis.CanonicalCatalog
