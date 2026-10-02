import Zetesis.FiniteClosure
import Zetesis.PackedInterpretations

/-!
# Packed sequential closure of a frozen normal reduct

This authored executable model uses 32-bit words, numeric mask reads and writes,
one frozen rule selection, and immediate in-place-order consequence scans.
It refines `FiniteClosure`, rather than assuming that its packed result denotes
the least closure. The fresh-insertion flag is related to an unchanged complete
reference scan; bounded iteration then inherits the existing leastness argument.

Atom coordinates are `Fin size`, so admission is structural. The model does not
extract Rust or model allocation, machine-index overflow, resource stops,
cancellation, statistics or program ownership. Source grounding and filter
evaluation precede this normalized finite presentation.
-/

namespace Zetesis.Refinement.PackedClosure

open FiniteClosure
namespace Packing

/-- Raw membership reads a numeric one-bit mask, as the static CPU checker does.
    The representation invariant separately proves that admitted reads exist. -/
def read (words : List (BitVec 32)) (atom : Nat) : Bool :=
  (PackedInterpretations.word words (atom / 32) &&& (1#32 <<< (atom % 32))) != 0#32

/-- Write one bit without changing the word array's length. -/
def insert (words : List (BitVec 32)) (atom : Nat) : List (BitVec 32) :=
  words.set (atom / 32)
    (PackedInterpretations.word words (atom / 32) ||| (1#32 <<< (atom % 32)))

/-- The 32-bit mask operation reads exactly the selected raw bit. -/
theorem read_bit (words : List (BitVec 32)) (atom : Nat) :
    read words atom = PackedInterpretations.bit32 words atom := by
  exact PackedInterpretations.mask_bit _ _ (Nat.mod_lt atom (by decide))

/-- A stored insertion sets exactly one coordinate, including when its previous
    value was already true; every other bit and the storage length survive. -/
theorem insert_exact (words : List (BitVec 32)) (atom : Nat)
    (stored : atom / 32 < words.length) :
    (insert words atom).length = words.length ∧
      ∀ tested, read (insert words atom) tested = (read words tested || decide (tested = atom)) := by
  have length : (insert words atom).length = words.length := by simp [insert]
  have membership : ∀ tested,
      read (insert words atom) tested = (read words tested || decide (tested = atom)) := by
    intro tested
    rw [read_bit, read_bit]
    by_cases sameWord : atom / 32 = tested / 32
    · have sameAtom : tested % 32 = atom % 32 ↔ tested = atom := by omega
      simp only [PackedInterpretations.bit32, insert, PackedInterpretations.word]
      rw [← sameWord, List.getElem?_set_self stored]
      simp only [Option.getD_some, BitVec.getLsbD_or]
      rw [← BitVec.twoPow_eq]
      simp [Nat.mod_lt _ (by decide : 0 < 32), ← sameAtom, eq_comm]
    · have distinct : tested ≠ atom := by
        intro same
        exact sameWord (congrArg (fun value => value / 32) same.symm)
      simp [PackedInterpretations.bit32, insert, PackedInterpretations.word,
        List.getElem?_set_ne sameWord, distinct]
  exact ⟨length, membership⟩

/-- Exact storage and truth at every coordinate. Since the listed atoms are
    bounded, this also states that all physical padding bits remain zero. -/
def Represents {size : Nat} (words : List (BitVec 32)) (known : List (Fin size)) : Prop :=
  words.length = PackedInterpretations.count32 size ∧
    ∀ atom, read words atom = decide (atom ∈ known.map Fin.val)

/-- Initialize 32-bit zero words and perform the admitted seed writes directly,
    in input order, as the static CPU checker's successful preparation does. -/
def encode {size : Nat} (known : List (Fin size)) : List (BitVec 32) :=
  known.foldl (fun words atom => insert words atom.val)
    (List.replicate (PackedInterpretations.count32 size) 0)

/-- Zero initialization establishes empty truth and all padding bits directly. -/
theorem zero_represents (size : Nat) :
    Represents (List.replicate (PackedInterpretations.count32 size) 0)
      ([] : List (Fin size)) := by
  refine ⟨by simp, ?_⟩
  intro atom
  rw [read_bit]
  simp only [PackedInterpretations.bit32, PackedInterpretations.word, List.getElem?_replicate]
  split <;> simp

/-- Reading an admitted coordinate agrees with membership in its represented
    list, without any uniqueness or ordering premise on that list. -/
theorem read_member {size : Nat} (words : List (BitVec 32)) (known : List (Fin size))
    (represented : Represents words known) (atom : Fin size) :
    read words atom.val = decide (atom ∈ known) := by
  rw [represented.2]
  congr 1
  simp only [List.mem_map]
  apply propext
  constructor
  · rintro ⟨source, member, same⟩
    exact (Fin.ext same) ▸ member
  · intro member
    exact ⟨atom, member, rfl⟩

/-- The missing-word default is never used by an admitted atom read or write. -/
theorem coordinate_stored {size : Nat} (words : List (BitVec 32))
    (known : List (Fin size)) (represented : Represents words known) (atom : Fin size) :
    atom.val / 32 < words.length := by
  rw [represented.1]
  unfold PackedInterpretations.count32
  have bounded := atom.isLt
  omega

/-- A bounded insertion preserves the representation and zero padding while
    adding exactly its atom to the represented set. -/
theorem insert_represents {size : Nat} (words : List (BitVec 32))
    (known : List (Fin size)) (represented : Represents words known) (atom : Fin size) :
    Represents (insert words atom.val) (atom :: known) := by
  obtain ⟨length, membership⟩ := insert_exact words atom.val
    (coordinate_stored words known represented atom)
  refine ⟨length.trans represented.1, ?_⟩
  intro tested
  rw [membership, represented.2]
  simp [Bool.or_comm]

/-- A finite sequence of admitted writes represents exactly its new atoms and
    the previous truth. The reference list is reversed because each write adds
    its coordinate at the front; order and repeated entries do not alter truth. -/
theorem insert_all_represents {size : Nat} (inputs known : List (Fin size))
    (words : List (BitVec 32)) (represented : Represents words known) :
    Represents (inputs.foldl (fun current atom => insert current atom.val) words)
      (inputs.reverse ++ known) := by
  induction inputs generalizing words known with
  | nil => simpa using represented
  | cons atom rest ih =>
    have next := insert_represents words known represented atom
    have completed := ih (atom :: known) (insert words atom.val) next
    simpa only [List.foldl_cons, List.reverse_cons, List.append_assoc,
      List.singleton_append] using completed

/-- Direct 32-bit initialization and writes establish the representation;
    successful seed packing is not a supplied correctness assumption. -/
theorem encode_represents {size : Nat} (known : List (Fin size)) :
    Represents (encode known) known := by
  have completed := insert_all_represents known [] _ (zero_represents size)
  simpa only [Represents, encode, List.append_nil, List.map_reverse, List.mem_reverse] using completed

end Packing

/-- Test a finite positive body by packed membership. `List.all` stops at its
    first false entry, matching the logical control of the static scan. -/
def allPresent {size : Nat} (words : List (BitVec 32)) (atoms : List (Fin size)) : Bool :=
  atoms.all (fun atom => Packing.read words atom.val)

/-- Packed positive-body evaluation equals the reference list-membership test. -/
theorem all_present_refines {size : Nat} (words : List (BitVec 32))
    (known atoms : List (Fin size)) (represented : Packing.Represents words known) :
    allPresent words atoms = atoms.all (fun atom => decide (atom ∈ known)) := by
  simp only [allPresent, Packing.read_member words known represented]

/-- Both gate polarities read only the frozen words, never derived truth. -/
def enabled {size : Nat} (rule : Rule (Fin size)) (frozen : List (BitVec 32)) : Bool :=
  allPresent frozen rule.gateTrue &&
    rule.gateFalse.all (fun atom => !Packing.read frozen atom.val)

/-- Both packed gate tests equal the existing finite frozen-gate decision. -/
theorem enabled_refines {size : Nat} (rule : Rule (Fin size))
    (frozen : List (BitVec 32)) (seed : List (Fin size))
    (represented : Packing.Represents frozen seed) :
    enabled rule frozen = gates rule seed := by
  simp [enabled, all_present_refines frozen seed _ represented, gates,
    Packing.read_member frozen seed represented]

/-- Select enabled consequence rules once, retaining their original order.
    Constraints have a separate final check and are not scanned for heads. -/
def select {size : Nat} (rules : List (Rule (Fin size)))
    (frozen : List (BitVec 32)) : List (Rule (Fin size)) :=
  rules.filter (fun rule => enabled rule frozen && rule.head.isSome)

/-- Every selected consequence is enabled by the same immutable seed. -/
theorem selected_enabled {size : Nat} (rules : List (Rule (Fin size)))
    (frozen : List (BitVec 32)) (seed : List (Fin size))
    (represented : Packing.Represents frozen seed) :
    ∀ rule ∈ select rules frozen, gates rule seed = true := by
  intro rule member
  have retained := (List.mem_filter.mp member).2
  have both : enabled rule frozen = true ∧ rule.head.isSome = true := by
    simpa only [Bool.and_eq_true] using retained
  exact (enabled_refines rule frozen seed represented) ▸ both.1

/-- A rule excluded by frozen selection cannot change the reference closure. -/
theorem unselected_step {size : Nat} (rule : Rule (Fin size))
    (frozen : List (BitVec 32)) (seed known : List (Fin size))
    (represented : Packing.Represents frozen seed)
    (excluded : (enabled rule frozen && rule.head.isSome) = false) :
    FiniteClosure.step rule seed known = known := by
  rw [enabled_refines rule frozen seed represented] at excluded
  cases head : rule.head with
  | none => simp [FiniteClosure.step, head]
  | some atom =>
    have disabled : gates rule seed = false := by simpa [head] using excluded
    simp [FiniteClosure.step, head, disabled]

/-- Removing disabled rules and constraints leaves every sequential reference
    scan unchanged, including the order in which fresh heads become visible. -/
theorem selection_refines {size : Nat} (rules : List (Rule (Fin size)))
    (frozen : List (BitVec 32)) (seed known : List (Fin size))
    (represented : Packing.Represents frozen seed) :
    FiniteClosure.scan (select rules frozen) seed known = FiniteClosure.scan rules seed known := by
  induction rules generalizing known with
  | nil => rfl
  | cons rule rest ih =>
    by_cases selected : (enabled rule frozen && rule.head.isSome) = true
    · simpa [select, List.filter_cons, selected, FiniteClosure.scan] using
        ih (FiniteClosure.step rule seed known)
    · have excluded : (enabled rule frozen && rule.head.isSome) = false := by
        cases truth : enabled rule frozen && rule.head.isSome <;> simp_all
      have unchanged := unselected_step rule frozen seed known represented excluded
      simpa [select, List.filter_cons, selected, FiniteClosure.scan, unchanged] using ih known

/-- Scan one selected consequence rule. Test the head first; only a fresh,
    enabled head performs a write and raises the changed flag. The absent-head
    branch is inert, though selection never supplies it. -/
def step {size : Nat} (rule : Rule (Fin size))
    (words : List (BitVec 32)) : List (BitVec 32) × Bool :=
  match rule.head with
  | none => (words, false)
  | some head =>
      if Packing.read words head.val then (words, false)
      else if allPresent words rule.positive then (Packing.insert words head.val, true)
      else (words, false)

/-- A selected packed step represents the reference step; its flag is false
    exactly when that step made no insertion. This is derived from reads and
    the numeric write, not assumed as a relation between final results. -/
theorem step_refines {size : Nat} (rule : Rule (Fin size))
    (words : List (BitVec 32)) (seed known : List (Fin size))
    (represented : Packing.Represents words known) (gate : gates rule seed = true) :
    Packing.Represents (step rule words).1 (FiniteClosure.step rule seed known) ∧
      ((step rule words).2 = false ↔ FiniteClosure.step rule seed known = known) := by
  have positive : allPresent words rule.positive = body rule known :=
    all_present_refines words known rule.positive represented
  cases head : rule.head with
  | none => simpa [step, FiniteClosure.step, head] using represented
  | some atom =>
    have membership := Packing.read_member words known represented atom
    by_cases present : atom ∈ known
    · cases truth : body rule known <;>
        simpa [step, FiniteClosure.step, head, membership, present, gate, positive, truth]
          using represented
    · by_cases satisfied : body rule known = true
      · have different : atom :: known ≠ known := by
          intro same
          have lengths := congrArg List.length same
          simp at lengths
        simpa [step, FiniteClosure.step, head, membership, present, gate,
          positive, satisfied, different] using Packing.insert_represents words known represented atom
      · simpa [step, FiniteClosure.step, head, membership, present, gate, positive, satisfied]
          using represented

/-- Each later rule sees the preceding write; the scan flag is the disjunction
    of all fresh-insertion flags. No synchronous snapshot is introduced. -/
def scan {size : Nat} : List (Rule (Fin size)) → List (BitVec 32) → List (BitVec 32) × Bool
  | [], words => (words, false)
  | rule :: rest, words =>
      let first := step rule words
      let later := scan rest first.1
      (later.1, first.2 || later.2)

/-- A complete packed scan has the same truth as the sequential list scan and
    reports no change exactly when that full reference scan is unchanged.
    The reverse flag direction uses inflationarity: later rules cannot undo
    a fresh head inserted by an earlier rule. -/
theorem scan_refines {size : Nat} (rules : List (Rule (Fin size)))
    (words : List (BitVec 32)) (seed known : List (Fin size))
    (represented : Packing.Represents words known)
    (selected : ∀ rule ∈ rules, gates rule seed = true) :
    Packing.Represents (scan rules words).1 (FiniteClosure.scan rules seed known) ∧
      ((scan rules words).2 = false ↔ FiniteClosure.scan rules seed known = known) := by
  induction rules generalizing words known with
  | nil => simpa [scan, FiniteClosure.scan] using represented
  | cons rule rest ih =>
    obtain ⟨first_represents, first_flag⟩ :=
      step_refines rule words seed known represented (selected rule List.mem_cons_self)
    obtain ⟨later_represents, later_flag⟩ := ih (step rule words).1
      (FiniteClosure.step rule seed known) first_represents
      (fun source member => selected source (List.mem_cons_of_mem _ member))
    refine ⟨later_represents, ?_⟩
    change ((step rule words).2 || (scan rest (step rule words).1).2) = false ↔ _
    rw [Bool.or_eq_false_iff, first_flag, later_flag]
    change (_ ∧ _) ↔ FiniteClosure.scan rest seed (FiniteClosure.step rule seed known) = known
    constructor
    · rintro ⟨first_same, later_same⟩
      exact later_same.trans first_same
    · intro unchanged
      have first_retained := step_retains rule seed known
      have later_retained := scan_retains rest seed (FiniteClosure.step rule seed known)
      have equal_length : known.length = (FiniteClosure.step rule seed known).length := by
        have first_bound := first_retained.length_le
        have later_bound := later_retained.length_le
        rw [unchanged] at later_bound
        omega
      have first_same : FiniteClosure.step rule seed known = known :=
        (first_retained.eq_of_length equal_length).symm
      exact ⟨first_same, unchanged.trans first_same.symm⟩

/-- Fuel bounds whole scans. A successful execution has observed a false
    fresh-insertion flag after a complete scan; zero fuel is unfinished. -/
def iterate {size : Nat} (rules : List (Rule (Fin size))) :
    Nat → List (BitVec 32) → Option (List (BitVec 32))
  | 0, _ => none
  | fuel + 1, words =>
      let next := scan rules words
      if next.2 then iterate rules fuel next.1 else some next.1

/-- Every completed packed execution has a completed reference execution with
    the same represented truth, even when its starting interpretation is not
    empty. No correctness or closedness assumption about the result is needed. -/
theorem iterate_refines {size : Nat} (rules : List (Rule (Fin size)))
    (seed known : List (Fin size)) (words result : List (BitVec 32)) (fuel : Nat)
    (represented : Packing.Represents words known)
    (selected : ∀ rule ∈ rules, gates rule seed = true)
    (completed : iterate rules fuel words = some result) :
    ∃ reference, FiniteClosure.iterate rules seed fuel known = some reference ∧
      Packing.Represents result reference := by
  induction fuel generalizing known words with
  | zero => simp [iterate] at completed
  | succ fuel ih =>
    obtain ⟨next_represents, next_flag⟩ := scan_refines rules words seed known represented selected
    cases changed : (scan rules words).2 with
    | false =>
      have unchanged : FiniteClosure.scan rules seed known = known := next_flag.mp changed
      have returned : (scan rules words).1 = result := by
        simpa [iterate, changed] using completed
      refine ⟨known, by simp [FiniteClosure.iterate, unchanged], ?_⟩
      simpa only [returned, unchanged] using next_represents
    | true =>
      have unequal : FiniteClosure.scan rules seed known ≠ known := by
        intro same
        have impossible := next_flag.mpr same
        simp [changed] at impossible
      have continued : iterate rules fuel (scan rules words).1 = some result := by
        simpa [iterate, changed] using completed
      obtain ⟨reference, reference_completed, exact⟩ :=
        ih (FiniteClosure.scan rules seed known) (scan rules words).1 next_represents continued
      exact ⟨reference, by simpa [FiniteClosure.iterate, unequal] using reference_completed, exact⟩

/-- Reference completion is not lost by the packed representation. In each
    round the proved flag equivalence forces both executions down the same
    continuing or completed branch. -/
theorem iterate_completes {size : Nat} (rules : List (Rule (Fin size)))
    (seed known reference : List (Fin size)) (words : List (BitVec 32)) (fuel : Nat)
    (represented : Packing.Represents words known)
    (selected : ∀ rule ∈ rules, gates rule seed = true)
    (completed : FiniteClosure.iterate rules seed fuel known = some reference) :
    ∃ result, iterate rules fuel words = some result ∧ Packing.Represents result reference := by
  induction fuel generalizing known words with
  | zero => simp [FiniteClosure.iterate] at completed
  | succ fuel ih =>
    obtain ⟨next_represents, next_flag⟩ := scan_refines rules words seed known represented selected
    by_cases unchanged : FiniteClosure.scan rules seed known = known
    · have changed : (scan rules words).2 = false := next_flag.mpr unchanged
      have same : known = reference := by
        simpa [FiniteClosure.iterate, unchanged] using completed
      refine ⟨(scan rules words).1, by simp [iterate, changed], ?_⟩
      rw [unchanged, same] at next_represents
      exact next_represents
    · have changed : (scan rules words).2 = true := by
        cases truth : (scan rules words).2 with
        | false => exact False.elim (unchanged (next_flag.mp truth))
        | true => rfl
      have continued : FiniteClosure.iterate rules seed fuel
          (FiniteClosure.scan rules seed known) = some reference := by
        simpa [FiniteClosure.iterate, unchanged] using completed
      obtain ⟨result, packed_completed, exact⟩ := ih (FiniteClosure.scan rules seed known)
        (scan rules words).1 next_represents continued
      exact ⟨result, by simpa [iterate, changed] using packed_completed, exact⟩

/-- Frozen preselection preserves the reference algorithm's stopping point and
    result, not just its eventual least set. This follows round by round from
    the equality of the actual sequential scans. -/
theorem selection_iteration {size : Nat} (rules : List (Rule (Fin size)))
    (frozen : List (BitVec 32)) (seed known : List (Fin size)) (fuel : Nat)
    (represented : Packing.Represents frozen seed) :
    FiniteClosure.iterate (select rules frozen) seed fuel known =
      FiniteClosure.iterate rules seed fuel known := by
  induction fuel generalizing known with
  | zero => rfl
  | succ fuel ih =>
    simp only [FiniteClosure.iterate, selection_refines rules frozen seed known represented]
    split
    · rfl
    · exact ih _

/-- The complete static closure algorithm: encode the frozen seed, select its
    consequence rules once, and iterate from empty positive truth. -/
def close {size : Nat} (rules : List (Rule (Fin size)))
    (seed : List (Fin size)) (fuel : Nat) : Option (List (BitVec 32)) :=
  iterate (select rules (Packing.encode seed)) fuel (Packing.encode ([] : List (Fin size)))

/-- The same conservative head-list bound suffices for the packed algorithm.
    Duplicate heads and rule ordering require no special assumption. -/
theorem close_completes {size : Nat} (rules : List (Rule (Fin size)))
    (seed : List (Fin size)) :
    ∃ result, close rules seed ((heads rules).length + 1) = some result := by
  obtain ⟨reference, completed⟩ := FiniteClosure.empty_completes rules seed
  have frozen_represents := Packing.encode_represents seed
  have selected_completed : FiniteClosure.iterate (select rules (Packing.encode seed))
      seed ((heads rules).length + 1) [] = some reference := by
    rw [selection_iteration rules _ seed [] _ frozen_represents]
    exact completed
  obtain ⟨result, packed_completed, _⟩ := iterate_completes
    (select rules (Packing.encode seed)) seed [] reference (Packing.encode []) _
    (Packing.encode_represents []) (selected_enabled rules _ seed frozen_represents)
    selected_completed
  exact ⟨result, packed_completed⟩

/-- Interpret the completed packed truth without committing to an atom list. -/
def denotes {size : Nat} (words : List (BitVec 32)) : Atoms (Fin size) :=
  fun atom => Packing.read words atom.val = true

/-- The representation relation supplies an extensional semantic interpretation. -/
theorem represents_denotes {size : Nat} (words : List (BitVec 32))
    (known : List (Fin size)) (represented : Packing.Represents words known) :
    denotes words = atoms known := by
  apply atoms_ext
  intro atom
  simp [denotes, Packing.read_member words known represented, atoms]

/-- A completed packed run is the independently defined least reduct closure.
    It also retains the exact word count and zero raw padding. The proof first
    reconstructs a completed list run, then invokes its soundness and closedness
    theorem; no agreement premise between implementations is assumed. -/
theorem close_exact {size : Nat} (rules : List (Rule (Fin size)))
    (seed : List (Fin size)) (fuel : Nat) (result : List (BitVec 32))
    (completed : close rules seed fuel = some result) :
    denotes result = Semantics.Gamma (program rules) (atoms seed) ∧
      result.length = PackedInterpretations.count32 size ∧
      (∀ atom, size ≤ atom → Packing.read result atom = false) := by
  have frozen_represents := Packing.encode_represents seed
  obtain ⟨reference, reference_completed, represented⟩ := iterate_refines
    (select rules (Packing.encode seed)) seed [] (Packing.encode []) result fuel
    (Packing.encode_represents []) (selected_enabled rules _ seed frozen_represents) completed
  have original_completed : FiniteClosure.iterate rules seed fuel [] = some reference := by
    rw [← selection_iteration rules _ seed [] fuel frozen_represents]
    exact reference_completed
  have sound : Sub (atoms ([] : List (Fin size))) (Semantics.Gamma (program rules) (atoms seed)) := by
    intro _ impossible
    exact False.elim (List.not_mem_nil impossible)
  have least : denotes result = Semantics.Gamma (program rules) (atoms seed) :=
    (represents_denotes result reference represented).trans
      (FiniteClosure.iterate_exact rules seed fuel [] reference sound original_completed)
  have padding : ∀ atom, size ≤ atom → Packing.read result atom = false := by
    intro atom outside
    have absent : atom ∉ reference.map Fin.val := by
      intro member
      obtain ⟨coordinate, _, same⟩ := List.mem_map.mp member
      have bounded := coordinate.isLt
      omega
    simp [represented.2, absent]
  exact ⟨least, represented.1, padding⟩

end Zetesis.Refinement.PackedClosure
