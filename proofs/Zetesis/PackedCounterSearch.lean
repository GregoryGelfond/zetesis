import Zetesis.PackedSubsets
import Zetesis.CounterSearch

/-!
# Streaming packed countermodel search

The original formula mask is computed once. Proper subsets are then queried one
at a time using checked indexed evaluation; a witness returns immediately.
The counter state is a 64-bit word list plus its maintained population, and its
transition is the existing proved set/clear carry from `PackedSubsets`.

Representation establishes each query's exact atom function and each successor
state. The resulting all-fuel correspondence composes with `CounterSearch`'s
completion and membership laws; there is no additional enumeration theorem or
assumed query agreement. This is an authored Lean algorithm, not extracted Rust.
-/

namespace Zetesis.Refinement.PackedCounterSearch

open PackedInterpretations

/-- A represented packed state supplies the reference selection's exact Boolean
    atom function. This derives query agreement before formula evaluation. -/
theorem represented_truth {size : Nat} (words : List (BitVec 64))
    (chosen : List (Fin size)) (represented : PackedSubsets.Represents words chosen) :
    PackedSubsets.truth words = FiniteMembership.candidate chosen := by
  funext atom
  simp [PackedSubsets.truth, contains_eq_bit, atom.isLt,
    PackedSubsets.bit_member words chosen represented, FiniteMembership.candidate]

/-- Query one packed subset against a previously computed immutable mask.
    Original truth is not evaluated or reconstructed by this operation. -/
def query {size : Nat} (frozen : List Bool) (table : List (DagSharing.Node (Fin size)))
    (roots : List Nat) (words : List (BitVec 64)) : Option Bool := do
  let values ← IndexedEvaluation.evaluate (PackedSubsets.truth words) (some frozen) table
  IndexedEvaluation.roots values roots

/-- Checked packed evaluation equals the existing positional query because its
    atom function is proved equal. Invalid child, mask or root reads therefore
    retain the same failure channel as well; correctness of a query is not a
    caller premise. -/
theorem query_refines {size : Nat} (atoms : List (Fin size)) (bits : List Bool)
    (words : List (BitVec 64)) (frozen : List Bool)
    (table : List (DagSharing.Node (Fin size))) (roots : List Nat)
    (represented : PackedSubsets.Represents words (SubsetCounter.selected atoms bits)) :
    query frozen table roots words = CounterSearch.query atoms frozen table roots bits := by
  unfold query CounterSearch.query
  rw [represented_truth words _ represented]

/-- Test the current proper subset before performing another carry. A true
    query returns immediately; a false query uses the existing set/clear
    transition and maintained population. The full candidate is not queried.
    Fuel counts queries, and zero fuel at a nonfull state is unfinished. -/
def search {size : Nat} (atoms : List (Fin size)) (frozen : List Bool)
    (table : List (DagSharing.Node (Fin size))) (roots : List Nat) :
    Nat → List (BitVec 64) → Nat → Option Bool
  | fuel, words, present =>
      if present < atoms.length then
        match fuel with
        | 0 => none
        | remaining + 1 => do
            let witness ← query frozen table roots words
            if witness then some true else
              let next ← PackedSubsets.carry atoms words present
              search atoms frozen table roots remaining next.1 next.2
      else some false

/-- At every fuel allowance, the packed stream and existing positional stream
    have the same completed or unfinished result. The representation and count
    invariant align the guard; `query_refines` aligns the actual checked query.
    After a false query, the proved packed carry establishes both invariants for
    induction. A true query needs neither a carry nor a later query.
-/
theorem search_refines {size : Nat} (atoms : List (Fin size)) (frozen : List Bool)
    (table : List (DagSharing.Node (Fin size))) (roots : List Nat) (fuel : Nat)
    (bits : List Bool) (words : List (BitVec 64)) (present : Nat)
    (unique : atoms.Nodup) (width : bits.length = atoms.length)
    (represented : PackedSubsets.Represents words (SubsetCounter.selected atoms bits))
    (counted : present = SubsetCounter.population bits) :
    search atoms frozen table roots fuel words present =
      CounterSearch.search (CounterSearch.query atoms frozen table roots) fuel bits := by
  induction fuel generalizing bits words present with
  | zero =>
    by_cases full : SubsetCounter.full bits = true
    · have stopped : ¬ present < atoms.length := by
        rw [← width, SubsetCounter.guard_exact bits present counted]
        exact fun notFull => notFull full
      simp [search, CounterSearch.search, full, stopped]
    · have continuing : present < atoms.length := by
        rw [← width, SubsetCounter.guard_exact bits present counted]
        exact full
      simp [search, CounterSearch.search, full, continuing]
  | succ fuel ih =>
    by_cases full : SubsetCounter.full bits = true
    · have stopped : ¬ present < atoms.length := by
        rw [← width, SubsetCounter.guard_exact bits present counted]
        exact fun notFull => notFull full
      simp [search, CounterSearch.search, full, stopped]
    · have continuing : present < atoms.length := by
        rw [← width, SubsetCounter.guard_exact bits present counted]
        exact full
      have current := query_refines atoms bits words frozen table roots represented
      cases queried : CounterSearch.query atoms frozen table roots bits with
      | none => simp [search, CounterSearch.search, continuing, full, current, queried]
      | some witness =>
        cases witness with
        | true => simp [search, CounterSearch.search, continuing, full, current, queried]
        | false =>
          cases advanced : SubsetCounter.increment bits with
          | none => exact False.elim (full ((SubsetCounter.increment_none_iff bits).mp advanced))
          | some next =>
            obtain ⟨updated, carried, next_represents⟩ := PackedSubsets.carry_counted atoms bits next
              words present unique width represented counted advanced
            have next_width : next.length = atoms.length :=
              (SubsetCounter.increment_length bits next advanced).trans width
            have continued := ih next updated (SubsetCounter.population next)
              next_width next_represents rfl
            simp [search, CounterSearch.search, continuing, full, current, queried,
              carried, advanced, continued]

/-- A structurally admitted packed truth assignment, with no satisfaction or
    stability claim. These fields express word shape and padding only; they do
    not establish Rust allocation, borrowing or exact theory-owner identity. -/
structure Candidate (size : Nat) where
  words : List (BitVec 64)
  stored : words.length = count64 size
  padding : ZeroPadding size words

/-- Bounded packed construction supplies the candidate's structural proofs.
    Repeated input coordinates are allowed and denote one selected atom. -/
def Candidate.ofAtoms {size : Nat} (atoms : List (Fin size)) : Candidate size :=
  let shape := PackedSubsets.represented_shape (pack size (atoms.map Fin.val)) atoms
    (PackedSubsets.pack_represents atoms)
  ⟨pack size (atoms.map Fin.val), shape.2.1, shape.2.2⟩

/-- The constructor's candidate denotes exactly the supplied finite atom set. -/
theorem Candidate.of_atoms_denotes {size : Nat} (atoms : List (Fin size)) :
    denotes size (Candidate.ofAtoms atoms).words =
      TightEvaluation.interpretation (FiniteMembership.candidate atoms) :=
  (PackedSubsets.represented_shape _ atoms (PackedSubsets.pack_represents atoms)).1

/-- Every coordinate read by the universe scan or an admitted formula atom is
    backed by a stored word. The total membership accessor's default is unused. -/
theorem Candidate.coordinate_stored {size : Nat} (candidate : Candidate size) (atom : Fin size) :
    atom.val / 64 < candidate.words.length := by
  rw [candidate.stored]
  unfold count64
  have bounded := atom.isLt
  omega

/-- Compute original truth once and reject a false original root before
    constructing the selected-ID list. Otherwise stream packed proper subsets
    against that same immutable mask. No visit list or repeated packing occurs.
    `none` is unfinished fuel or a failed indexed read, never a verdict. -/
def check {size : Nat} (fuel : Nat) (candidate : Candidate size)
    (table : List (DagSharing.Node (Fin size))) (roots : List Nat) : Option Bool := do
  let frozen ← IndexedEvaluation.evaluate (PackedSubsets.truth candidate.words) none table
  let original ← IndexedEvaluation.roots frozen roots
  if original then
    let atoms := PackedSubsets.selectedAtoms size candidate.words
    return !(← search atoms frozen table roots fuel (pack size []) 0)
  else some false

/-- The complete packed streaming checker has exactly the same result at every
    fuel allowance as the existing checked positional checker. Original truth
    agrees by the proved selected-ID producer; each frozen-mask search agrees
    by `search_refines`. This theorem assumes no evaluator or coverage equation.
-/
theorem check_refines {size : Nat} (fuel : Nat) (candidate : Candidate size)
    (table : List (DagSharing.Node (Fin size))) (roots : List Nat) :
    check fuel candidate table roots =
      CounterSearch.check fuel (PackedSubsets.selectedAtoms size candidate.words) table roots := by
  let atoms := PackedSubsets.selectedAtoms size candidate.words
  have initial : PackedSubsets.Represents (pack size [])
      (SubsetCounter.selected atoms (List.replicate atoms.length false)) := by
    rw [PackedSubsets.selected_false]
    exact PackedSubsets.pack_represents []
  have streams (frozen : List Bool) : search atoms frozen table roots fuel (pack size []) 0 =
      CounterSearch.search (CounterSearch.query atoms frozen table roots) fuel
        (List.replicate atoms.length false) :=
    search_refines atoms frozen table roots fuel _ _ 0
      (PackedSubsets.selected_atoms_nodup size candidate.words) (List.length_replicate ..)
      initial (PackedSubsets.population_zero atoms.length).symm
  unfold check CounterSearch.check
  rw [PackedSubsets.selected_atoms_truth]
  dsimp only [atoms] at streams
  simp only [streams]

/-- The proved complete allowance counts proper-subset queries, not machine
    operations. The selected-ID list is computed from the original candidate. -/
def bound {size : Nat} (candidate : Candidate size) : Nat :=
  2 ^ (PackedSubsets.selectedAtoms size candidate.words).length - 1

/-- At the constructive finite bound, checked packed streaming completes with
    the finite semantic membership decision. DAG edges must point backward and
    every asserted root must exist; candidate storage is carried by its type. -/
theorem check_exact {size : Nat} (candidate : Candidate size)
    (table : List (DagSharing.Node (Fin size))) (roots : List Nat)
    (admitted : DagSharing.WellFormed table) (rootBounds : ∀ root ∈ roots, root < table.length) :
    check (bound candidate) candidate table roots =
      some (FiniteMembership.check (PackedSubsets.selectedAtoms size candidate.words) table roots) := by
  rw [check_refines]
  exact CounterSearch.check_exact _ (PackedSubsets.selected_atoms_nodup size candidate.words)
    table roots admitted rootBounds

/-- Every structurally admitted input has a completed verdict at the explicit
    bound. No claim of completion is made for an arbitrary smaller allowance. -/
theorem check_completes {size : Nat} (candidate : Candidate size)
    (table : List (DagSharing.Node (Fin size))) (roots : List Nat)
    (admitted : DagSharing.WellFormed table) (rootBounds : ∀ root ∈ roots, root < table.length) :
    ∃ result, check (bound candidate) candidate table roots = some result :=
  ⟨_, check_exact candidate table roots admitted rootBounds⟩

/-- Any completed verdict is exact, including early original rejection or an
    early countermodel before the complete bound. This reuses the established
    streaming extension law; `none` supplies no membership decision. -/
theorem completed_check_exact {size : Nat} (fuel : Nat) (candidate : Candidate size)
    (table : List (DagSharing.Node (Fin size))) (roots : List Nat)
    (admitted : DagSharing.WellFormed table) (rootBounds : ∀ root ∈ roots, root < table.length)
    (result : Bool) (completed : check fuel candidate table roots = some result) :
    result = FiniteMembership.check (PackedSubsets.selectedAtoms size candidate.words) table roots := by
  rw [check_refines] at completed
  exact CounterSearch.completed_check_exact fuel _
    (PackedSubsets.selected_atoms_nodup size candidate.words) table roots
    admitted rootBounds result completed

/-- A completed packed-stream verdict is true exactly when the original packed
    interpretation is an answer set. Both rejection and acceptance are exact;
    the conclusion is conditional on completion, not on a complete oracle. -/
theorem completed_check_iff_answer_set {size : Nat} (fuel : Nat) (candidate : Candidate size)
    (table : List (DagSharing.Node (Fin size))) (roots : List Nat)
    (admitted : DagSharing.WellFormed table) (rootBounds : ∀ root ∈ roots, root < table.length)
    (result : Bool) (completed : check fuel candidate table roots = some result) :
    result = true ↔ Ferraris.Stable (denotes size candidate.words) (DagSharing.assertions table roots) := by
  rw [completed_check_exact fuel candidate table roots admitted rootBounds result completed]
  have semantic := FiniteMembership.check_iff_answer_set
    (PackedSubsets.selectedAtoms size candidate.words) table roots
  rw [PackedSubsets.selected_atoms_truth] at semantic
  exact semantic

/-- At the proved bound, the executable packed checker accepts exactly the
    answer sets of the original denoted formula theory. -/
theorem check_iff_answer_set {size : Nat} (candidate : Candidate size)
    (table : List (DagSharing.Node (Fin size))) (roots : List Nat)
    (admitted : DagSharing.WellFormed table) (rootBounds : ∀ root ∈ roots, root < table.length) :
    check (bound candidate) candidate table roots = some true ↔
      Ferraris.Stable (denotes size candidate.words) (DagSharing.assertions table roots) := by
  rw [check_exact candidate table roots admitted rootBounds, Option.some.injEq]
  have semantic := FiniteMembership.check_iff_answer_set
    (PackedSubsets.selectedAtoms size candidate.words) table roots
  rw [PackedSubsets.selected_atoms_truth] at semantic
  exact semantic

/-- Two required atoms on different words remain an answer set after the
stream carries across that word boundary and exhausts their proper subsets. -/
theorem cross_word_stream_accepts_facts :
    check 3 (Candidate.ofAtoms [⟨0, by omega⟩, ⟨64, by omega⟩] : Candidate 65)
      [.atom ⟨0, by omega⟩, .atom ⟨64, by omega⟩, .conj 0 1] [2] = some true := by decide

end Zetesis.Refinement.PackedCounterSearch
