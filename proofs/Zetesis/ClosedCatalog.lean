import Zetesis.CanonicalCatalog
import Std.Tactic

/-!
# Lookup in a closed base and a local row suffix

Two lookup functions describe a fixed base and a local suffix. Query the base
first; on absence, translate a successful local position by the base length.
Component soundness makes every combined result decode correctly. Component
coverage makes combined absence exact, even when equal atoms occur more than
once. Only identifying the unique returned position requires global uniqueness.
The queries are pure; `none` means completed absence, never a stopped lookup.

The decoder is the existing `AtomCatalogs.lookup`. This module does not model
hash tables, scope tokens, shared allocations, machine-width offsets or effectful
publication. Their correspondence remains an implementation obligation. The
physical base/suffix split is independent of the semantic base/defined-atom
partition in `TerminalDefinitions`.
-/

namespace Zetesis.ClosedCatalog

universe u
variable {Atom : Type u}

/-- Prefer an existing base position; otherwise translate a suffix position.
The two queries have their own coordinate domains. -/
def find (baseSize : Nat) (baseQuery localQuery : Atom → Option Nat)
    (atom : Atom) : Option Nat :=
  match baseQuery atom with
  | some position => some position
  | none => (localQuery atom).map (fun position => baseSize + position)

/-- Every combined result denotes the queried atom in the concatenated rows.
Only sound successful component queries are needed; duplicates are allowed.

A base hit lies in the preserved prefix. A suffix hit is translated past that
prefix, where the split decoder subtracts exactly the added offset.
-/
theorem find_sound (base suffix : List Atom)
    (baseQuery localQuery : Atom → Option Nat)
    (base_sound : ∀ atom position,
      baseQuery atom = some position → base[position]? = some atom)
    (local_sound : ∀ atom position,
      localQuery atom = some position → suffix[position]? = some atom)
    (atom : Atom) (position : Nat)
    (found : find base.length baseQuery localQuery atom = some position) :
    (base ++ suffix)[position]? = some atom := by
  rw [AtomCatalogs.commit_preserves_lookup]
  cases base_found : baseQuery atom with
  | some basePosition =>
    have same_position : basePosition = position :=
      Option.some.inj (by simpa only [find, base_found] using found)
    subst position
    have decoded : base[basePosition]? = some atom :=
      base_sound atom basePosition base_found
    have in_prefix : basePosition < base.length :=
      (List.getElem?_eq_some_iff.mp decoded).choose
    exact (AtomCatalogs.append_preserves_identity base suffix basePosition in_prefix).trans
      decoded
  | none =>
    cases local_found : localQuery atom with
    | none =>
      have impossible : (none : Option Nat) = some position := by
        simpa only [find, base_found, local_found, Option.map_none] using found
      cases impossible
    | some localPosition =>
      have same_position : base.length + localPosition = position :=
        Option.some.inj (by
          simpa only [find, base_found, local_found, Option.map_some] using found)
      subst position
      have outside_prefix : ¬ base.length + localPosition < base.length := by omega
      have local_offset : base.length + localPosition - base.length = localPosition := by omega
      simpa only [AtomCatalogs.lookup, if_neg outside_prefix, local_offset] using
        local_sound atom localPosition local_found

/-- Combined absence is exactly absence from every represented row, provided
both component queries are sound and cover every decoded atom. This law needs
no uniqueness or disjointness assumption: a base occurrence may hide an equal
suffix occurrence without hiding the atom itself.

An absent combined result forces both component queries to be absent, so either
branch of the split decoder contradicts coverage. Conversely, a successful
result would contradict absence by `find_sound`.
-/
theorem find_none_iff (base suffix : List Atom)
    (baseQuery localQuery : Atom → Option Nat)
    (base_sound : ∀ atom position,
      baseQuery atom = some position → base[position]? = some atom)
    (local_sound : ∀ atom position,
      localQuery atom = some position → suffix[position]? = some atom)
    (base_covered : ∀ (atom : Atom) (position : Nat), base[position]? = some atom →
      ∃ found, baseQuery atom = some found)
    (local_covered : ∀ (atom : Atom) (position : Nat), suffix[position]? = some atom →
      ∃ found, localQuery atom = some found)
    (atom : Atom) :
    find base.length baseQuery localQuery atom = none ↔
      ∀ position : Nat, (base ++ suffix)[position]? ≠ some atom := by
  constructor
  · intro missing position decoded
    have base_missing : baseQuery atom = none := by
      cases queried : baseQuery atom with
      | none => rfl
      | some found =>
        have impossible : some found = (none : Option Nat) := by
          simpa only [find, queried] using missing
        cases impossible
    have local_missing : localQuery atom = none := by
      cases queried : localQuery atom with
      | none => rfl
      | some found =>
        have impossible : some (base.length + found) = (none : Option Nat) := by
          simpa only [find, base_missing, queried, Option.map_some] using missing
        cases impossible
    have split_decoded : AtomCatalogs.lookup base suffix position = some atom :=
      (AtomCatalogs.commit_preserves_lookup base suffix position).symm.trans decoded
    by_cases in_prefix : position < base.length
    · have base_decoded : base[position]? = some atom := by
        simpa only [AtomCatalogs.lookup, if_pos in_prefix] using split_decoded
      obtain ⟨found, queried⟩ := base_covered atom position base_decoded
      have impossible : (none : Option Nat) = some found := base_missing.symm.trans queried
      cases impossible
    · have local_decoded : suffix[position - base.length]? = some atom := by
        simpa only [AtomCatalogs.lookup, if_neg in_prefix] using split_decoded
      obtain ⟨found, queried⟩ := local_covered atom (position - base.length) local_decoded
      have impossible : (none : Option Nat) = some found := local_missing.symm.trans queried
      cases impossible
  · intro absent
    cases queried : find base.length baseQuery localQuery atom with
    | none => rfl
    | some position =>
      have decoded : (base ++ suffix)[position]? = some atom :=
        find_sound base suffix baseQuery localQuery base_sound local_sound atom position queried
      exact False.elim (absent position decoded)

/-- In a globally unique base-plus-suffix catalog, combined lookup returns
exactly the position decoding to the queried atom. Global uniqueness excludes
both duplicates within a component and equal atoms in different components.

Soundness is `find_sound`. For completeness, decoded membership excludes an
absent query by `find_none_iff`; the existing atom-catalog uniqueness law equates
the resulting position with the supplied one.
-/
theorem find_exact (base suffix : List Atom)
    (baseQuery localQuery : Atom → Option Nat)
    (base_sound : ∀ atom position,
      baseQuery atom = some position → base[position]? = some atom)
    (local_sound : ∀ atom position,
      localQuery atom = some position → suffix[position]? = some atom)
    (base_covered : ∀ (atom : Atom) (position : Nat), base[position]? = some atom →
      ∃ found, baseQuery atom = some found)
    (local_covered : ∀ (atom : Atom) (position : Nat), suffix[position]? = some atom →
      ∃ found, localQuery atom = some found)
    (unique : (base ++ suffix).Nodup) (atom : Atom) (position : Nat) :
    find base.length baseQuery localQuery atom = some position ↔
      (base ++ suffix)[position]? = some atom := by
  constructor
  · exact find_sound base suffix baseQuery localQuery base_sound local_sound atom position
  · intro decoded
    cases queried : find base.length baseQuery localQuery atom with
    | none =>
      have absent : ∀ selected, (base ++ suffix)[selected]? ≠ some atom :=
        (find_none_iff base suffix baseQuery localQuery base_sound local_sound
          base_covered local_covered atom).mp queried
      exact False.elim (absent position decoded)
    | some found =>
      have found_decoded : AtomCatalogs.lookup base suffix found = some atom :=
        (AtomCatalogs.commit_preserves_lookup base suffix found).symm.trans
          (find_sound base suffix baseQuery localQuery base_sound local_sound atom found queried)
      have selected_decoded : AtomCatalogs.lookup base suffix position = some atom :=
        (AtomCatalogs.commit_preserves_lookup base suffix position).symm.trans decoded
      have same_position : found = position :=
        AtomCatalogs.identity_determines_position base suffix unique found position atom
          found_decoded selected_decoded
      exact congrArg some same_position

end Zetesis.ClosedCatalog
