import Zetesis.SubsetCounter
import Zetesis.PackedInterpretations
import Init.Data.List.Nat.Pairwise

/-!
# Packed proper-subset enumeration

The counter reads and updates 64-bit words at distinct, bounded atom coordinates.
A finite universe scan constructs those coordinates from a packed candidate.
A true bit is cleared by an AND-complement mask;
the first false bit is set by the existing OR-mask insertion. The maintained
population decreases and increases with those writes.

The representation proof connects each actual word update to `SubsetCounter`'s
positional carry, then composes with its constructive coverage theorem. This is
authored Lean, not extracted Rust. Machine-index bounds, allocation, ownership,
resource interruption and source admission remain implementation obligations.
-/

namespace Zetesis.Refinement.PackedSubsets

open PackedInterpretations

/-- Clear one coordinate using the same AND-complement operation as the packed
    subset loop. Its target word must exist for the exactness theorem. -/
def clear (words : List (BitVec 64)) (atom : Nat) : List (BitVec 64) :=
  words.set (atom / 64) (word words (atom / 64) &&& ~~~(1#64 <<< (atom % 64)))

/-- A stored clear preserves length and removes exactly its coordinate. The
    proof distinguishes a different word from another bit of the same word. -/
theorem clear_exact (words : List (BitVec 64)) (atom : Nat)
    (stored : atom / 64 < words.length) :
    (clear words atom).length = words.length ∧
      ∀ tested, bit64 (clear words atom) tested =
        (bit64 words tested && decide (tested ≠ atom)) := by
  have length : (clear words atom).length = words.length := by simp [clear]
  have membership : ∀ tested, bit64 (clear words atom) tested =
      (bit64 words tested && decide (tested ≠ atom)) := by
    intro tested
    by_cases sameWord : atom / 64 = tested / 64
    · have sameAtom : tested % 64 = atom % 64 ↔ tested = atom := by omega
      simp only [bit64, clear, word]
      rw [← sameWord, List.getElem?_set_self stored]
      simp only [Option.getD_some, BitVec.getLsbD_and, BitVec.getLsbD_not]
      rw [← BitVec.twoPow_eq]
      simp [Nat.mod_lt _ (by decide : 0 < 64), ← sameAtom, eq_comm]
    · have distinct : tested ≠ atom := by
        intro same
        exact sameWord (congrArg (fun value => value / 64) same.symm)
      simp [bit64, clear, word, List.getElem?_set_ne sameWord, distinct]
  exact ⟨length, membership⟩

/-- Exact raw-bit representation at every coordinate, including storage shape
    and zero padding. The selected list itself contains only bounded atoms. -/
def Represents {size : Nat} (words : List (BitVec 64)) (chosen : List (Fin size)) : Prop :=
  words.length = count64 size ∧ ∀ atom, bit64 words atom = decide (atom ∈ chosen.map Fin.val)

/-- Existing packed construction establishes this stronger raw-bit invariant. -/
theorem pack_represents {size : Nat} (chosen : List (Fin size)) :
    Represents (pack size (chosen.map Fin.val)) chosen := by
  have inside : ∀ atom ∈ chosen.map Fin.val, atom < size := by
    intro atom member
    obtain ⟨coordinate, _, same⟩ := List.mem_map.mp member
    exact same ▸ coordinate.isLt
  obtain ⟨length, membership, padding⟩ := pack_exact size (chosen.map Fin.val) inside
  refine ⟨length, ?_⟩
  intro atom
  by_cases bounded : atom < size
  · have exact := membership atom
    simpa only [contains_eq_bit, bounded, decide_true, Bool.true_and] using exact
  · have absent : atom ∉ chosen.map Fin.val := fun member => bounded (inside atom member)
    simp only [padding atom (by omega), absent, decide_false]

/-- Every admitted coordinate addresses an existing word. The total word
    accessor's default cannot hide an out-of-bounds read in a represented state. -/
theorem coordinate_stored {size : Nat} (words : List (BitVec 64))
    (chosen : List (Fin size)) (represented : Represents words chosen) (atom : Fin size) :
    atom.val / 64 < words.length := by
  rw [represented.1]
  unfold count64
  have bounded := atom.isLt
  omega

/-- A bounded raw read is exactly membership in the represented selection. -/
theorem bit_member {size : Nat} (words : List (BitVec 64))
    (chosen : List (Fin size)) (represented : Represents words chosen) (atom : Fin size) :
    bit64 words atom.val = decide (atom ∈ chosen) := by
  rw [represented.2]
  congr 1
  apply propext
  simp only [List.mem_map]
  constructor
  · rintro ⟨source, member, same⟩
    exact (Fin.ext same) ▸ member
  · intro member
    exact ⟨atom, member, rfl⟩

/-- Setting a bounded bit adds precisely its atom to represented truth. -/
theorem insert_represents {size : Nat} (words : List (BitVec 64))
    (chosen : List (Fin size)) (represented : Represents words chosen) (atom : Fin size) :
    Represents (insert words atom.val) (atom :: chosen) := by
  obtain ⟨length, membership⟩ := insert_exact words atom.val
    (coordinate_stored words chosen represented atom)
  refine ⟨length.trans represented.1, ?_⟩
  intro tested
  rw [membership, represented.2]
  simp [Bool.or_comm]

/-- Clearing a selected first coordinate leaves exactly the tail when that
    coordinate is absent from the tail. Distinctness matters at this boundary. -/
theorem clear_represents {size : Nat} (words : List (BitVec 64))
    (atom : Fin size) (rest : List (Fin size)) (represented : Represents words (atom :: rest))
    (absent : atom ∉ rest) : Represents (clear words atom.val) rest := by
  obtain ⟨length, membership⟩ := clear_exact words atom.val
    (coordinate_stored words (atom :: rest) represented atom)
  refine ⟨length.trans represented.1, ?_⟩
  intro tested
  rw [membership, represented.2]
  have absent_value : atom.val ∉ rest.map Fin.val := by
    intro member
    obtain ⟨source, member, same⟩ := List.mem_map.mp member
    exact absent ((Fin.ext same) ▸ member)
  by_cases same : tested = atom.val
  · subst tested
    simp [absent_value]
  · simp [same]

/-- Representation supplies semantic truth as well as physical zero padding. -/
theorem represented_shape {size : Nat} (words : List (BitVec 64))
    (chosen : List (Fin size)) (represented : Represents words chosen) :
    denotes size words = TightEvaluation.interpretation (FiniteMembership.candidate chosen) ∧
      words.length = count64 size ∧ ZeroPadding size words := by
  have semantic : denotes size words =
      TightEvaluation.interpretation (FiniteMembership.candidate chosen) := by
    apply atoms_ext
    intro atom
    simp [denotes, contains_eq_bit, atom.isLt, bit_member words chosen represented,
      FiniteMembership.candidate_member]
  have padding : ZeroPadding size words := by
    intro atom outside
    have absent : atom ∉ chosen.map Fin.val := by
      rintro member
      obtain ⟨coordinate, _, same⟩ := List.mem_map.mp member
      have bounded := coordinate.isLt
      omega
    simp [represented.2, absent]
  exact ⟨semantic, represented.1, padding⟩

/-- Carry through selected coordinates, clearing true bits and decrementing the
    population until the first false bit is set. Overflow returns no next state.
    Every coordinate is bounded; `contains` therefore reads its numeric mask. -/
def carry {size : Nat} : List (Fin size) → List (BitVec 64) → Nat →
    Option (List (BitVec 64) × Nat)
  | [], _, _ => none
  | atom :: rest, words, present =>
      if contains size words atom.val then carry rest (clear words atom.val) (present - 1)
      else some (insert words atom.val, present + 1)

/-- A head absent from distinct remaining coordinates cannot occur in any of
    their positional selections. This prevents one write from changing another
    counter position's truth. -/
theorem absent_selected {size : Nat} (atom : Fin size) (rest : List (Fin size))
    (bits : List Bool) (absent : atom ∉ rest) (width : bits.length = rest.length) :
    atom ∉ SubsetCounter.selected rest bits := by
  intro member
  exact absent (FiniteMembership.selection_subset rest _
    (SubsetCounter.selected_is_selection rest bits width) atom member)

/-- Each successful tracked Boolean carry is executed by the concrete packed
    writes with the same resulting population and represented selection.

    At a false head bit, exact membership justifies setting that coordinate.
    At a true head bit, clearing it leaves the tail's selection because atom
    coordinates are distinct. Induction then performs the remaining carry.
    The count can be arbitrary here; its invariant is added in `carry_counted`.
-/
theorem carry_refines {size : Nat} (atoms : List (Fin size)) (bits : List Bool)
    (words : List (BitVec 64)) (present : Nat) (next : List Bool) (nextPresent : Nat)
    (unique : atoms.Nodup) (width : bits.length = atoms.length)
    (represented : Represents words (SubsetCounter.selected atoms bits))
    (advanced : SubsetCounter.trackedIncrement bits present = some (next, nextPresent)) :
    ∃ updated, carry atoms words present = some (updated, nextPresent) ∧
      Represents updated (SubsetCounter.selected atoms next) := by
  induction atoms generalizing bits words present next nextPresent with
  | nil =>
    cases bits with
    | nil => simp [SubsetCounter.trackedIncrement] at advanced
    | cons bit tail => simp at width
  | cons atom rest ih =>
    cases bits with
    | nil => simp at width
    | cons bit tail =>
      have tail_width : tail.length = rest.length := Nat.succ.inj width
      have tail_unique := (List.nodup_cons.mp unique).2
      have absent := absent_selected atom rest tail (List.nodup_cons.mp unique).1 tail_width
      cases bit with
      | false =>
        have current : Represents words (SubsetCounter.selected rest tail) := represented
        have head_false : contains size words atom.val = false := by
          simp [contains_eq_bit, atom.isLt, bit_member words _ current, absent]
        have same : (true :: tail, present + 1) = (next, nextPresent) := Option.some.inj advanced
        obtain ⟨rfl, rfl⟩ := Prod.mk.inj same
        exact ⟨insert words atom.val, by simp [carry, head_false],
          insert_represents words _ current atom⟩
      | true =>
        have current : Represents words (atom :: SubsetCounter.selected rest tail) := represented
        have head_true : contains size words atom.val = true := by
          simp [contains_eq_bit, atom.isLt, bit_member words _ current]
        have cleared := clear_represents words atom _ current absent
        cases carried : SubsetCounter.trackedIncrement tail (present - 1) with
        | none => simp [SubsetCounter.trackedIncrement, carried] at advanced
        | some successor =>
          have same : (false :: successor.1, successor.2) = (next, nextPresent) := by
            simpa only [SubsetCounter.trackedIncrement, carried, Option.map_some,
              Option.some.injEq] using advanced
          obtain ⟨rfl, rfl⟩ := Prod.mk.inj same
          obtain ⟨updated, packed, exact⟩ := ih tail (clear words atom.val) (present - 1)
            successor.1 successor.2 tail_unique tail_width cleared carried
          exact ⟨updated, by simpa [carry, head_true] using packed, exact⟩

/-- Under the population invariant, each ordinary counter increment performs
    the packed carry and establishes the next count and exact selected bits.
    Thus subsequent count guards need no renewed correctness assumption. -/
theorem carry_counted {size : Nat} (atoms : List (Fin size)) (bits next : List Bool)
    (words : List (BitVec 64)) (present : Nat)
    (unique : atoms.Nodup) (width : bits.length = atoms.length)
    (represented : Represents words (SubsetCounter.selected atoms bits))
    (counted : present = SubsetCounter.population bits)
    (advanced : SubsetCounter.increment bits = some next) :
    ∃ updated, carry atoms words present = some (updated, SubsetCounter.population next) ∧
      Represents updated (SubsetCounter.selected atoms next) := by
  have tracked : SubsetCounter.trackedIncrement bits present =
      some (next, SubsetCounter.population next) := by
    rw [SubsetCounter.tracked_increment_exact bits present counted, advanced]
    rfl
  exact carry_refines atoms bits words present next _ unique width represented tracked

/-- Visit packed states while the maintained population is below the number of
    selected coordinates. Fuel counts increments; insufficient fuel returns no
    completed visit list. The full candidate is never visited. -/
def walk {size : Nat} (atoms : List (Fin size)) :
    Nat → List (BitVec 64) → Nat → Option (List (List (BitVec 64)))
  | fuel, words, present =>
      if present < atoms.length then
        match fuel with
        | 0 => none
        | remaining + 1 => do
            let next ← carry atoms words present
            let tail ← walk atoms remaining next.1 next.2
            pure (words :: tail)
      else some []

/-- A completed positional walk is executed by packed writes with exactly the
    same sequence of semantic interpretations. Every visited word list retains
    its exact length and zero padding.

    The maintained population makes both loops choose the same stop branch.
    Otherwise `carry_counted` establishes the successor representation and count;
    induction supplies the remaining completed visits. Coverage is not a premise.
-/
theorem walk_refines {size : Nat} (atoms : List (Fin size)) (fuel : Nat)
    (bits : List Bool) (words : List (BitVec 64)) (present : Nat) (visited : List (List Bool))
    (unique : atoms.Nodup) (width : bits.length = atoms.length)
    (represented : Represents words (SubsetCounter.selected atoms bits))
    (counted : present = SubsetCounter.population bits)
    (completed : SubsetCounter.walk fuel bits = some visited) :
    ∃ packed, walk atoms fuel words present = some packed ∧
      packed.map (denotes size) = visited.map (fun state =>
        TightEvaluation.interpretation (FiniteMembership.candidate (SubsetCounter.selected atoms state))) ∧
      ∀ stored ∈ packed, stored.length = count64 size ∧ ZeroPadding size stored := by
  induction fuel generalizing bits words present visited with
  | zero =>
    by_cases full : SubsetCounter.full bits = true
    · have stopped : ¬ present < atoms.length := by
        rw [← width, SubsetCounter.guard_exact bits present counted]
        exact fun notFull => notFull full
      have empty : visited = [] := by simpa [SubsetCounter.walk, full] using completed.symm
      subst visited
      exact ⟨[], by simp [walk, stopped], rfl, by simp⟩
    · simp [SubsetCounter.walk, full] at completed
  | succ fuel ih =>
    by_cases full : SubsetCounter.full bits = true
    · have stopped : ¬ present < atoms.length := by
        rw [← width, SubsetCounter.guard_exact bits present counted]
        exact fun notFull => notFull full
      have empty : visited = [] := by simpa [SubsetCounter.walk, full] using completed.symm
      subst visited
      exact ⟨[], by simp [walk, stopped], rfl, by simp⟩
    · have continuing : present < atoms.length := by
        rw [← width, SubsetCounter.guard_exact bits present counted]
        exact full
      cases advanced : SubsetCounter.increment bits with
      | none => simp [SubsetCounter.walk, full, advanced] at completed
      | some next =>
        cases continued : SubsetCounter.walk fuel next with
        | none => simp [SubsetCounter.walk, full, advanced, continued] at completed
        | some tail =>
          have same : bits :: tail = visited := by
            simpa [SubsetCounter.walk, full, advanced, continued] using completed
          subst visited
          obtain ⟨updated, carried, next_represents⟩ :=
            carry_counted atoms bits next words present unique width represented counted advanced
          have next_width : next.length = atoms.length :=
            (SubsetCounter.increment_length bits next advanced).trans width
          obtain ⟨packed_tail, packed_completed, semantic_tail, shapes⟩ := ih next updated
            (SubsetCounter.population next) tail next_width next_represents rfl continued
          have current := represented_shape words _ represented
          refine ⟨words :: packed_tail, ?_, ?_, ?_⟩
          · simp [walk, continuing, carried, packed_completed]
          · simp only [List.map_cons, current.1, semantic_tail]
          · intro stored member
            rcases List.mem_cons.mp member with rfl | later
            · exact current.2
            · exact shapes stored later

/-- An all-false positional vector selects no atoms. -/
theorem selected_false {size : Nat} (atoms : List (Fin size)) :
    SubsetCounter.selected atoms (List.replicate atoms.length false) = [] := by
  induction atoms with
  | nil => rfl
  | cons atom rest ih => simpa [SubsetCounter.selected, List.replicate_succ] using ih

/-- The zeroed positional vector starts with population zero. -/
theorem population_zero (width : Nat) :
    SubsetCounter.population (List.replicate width false) = 0 := by
  induction width with
  | zero => rfl
  | succ width ih => simp [List.replicate_succ, SubsetCounter.population, ih]

/-- Starting from zero words completes at the already proved counter bound.
    Its semantic sequence and every physical representation invariant are
    consequences of the packed transition, not assumptions about enumeration. -/
theorem empty_walk_refines {size : Nat} (atoms : List (Fin size)) (unique : atoms.Nodup) :
    ∃ packed, walk atoms (2 ^ atoms.length - 1) (pack size []) 0 = some packed ∧
      packed.map (denotes size) = (SubsetCounter.states atoms.length).map (fun state =>
        TightEvaluation.interpretation (FiniteMembership.candidate (SubsetCounter.selected atoms state))) ∧
      ∀ stored ∈ packed, stored.length = count64 size ∧ ZeroPadding size stored := by
  have initial : Represents (pack size [])
      (SubsetCounter.selected atoms (List.replicate atoms.length false)) := by
    rw [selected_false]
    exact pack_represents []
  exact walk_refines atoms _ _ _ 0 _ unique (List.length_replicate ..) initial
    (population_zero atoms.length).symm (SubsetCounter.states_completed atoms.length)

/-- Execute the bounded packed walk. The distinct-coordinate premise provides
    completion; the returned words are computed, not semantically chosen. -/
def states {size : Nat} (atoms : List (Fin size)) (unique : atoms.Nodup) :
    List (List (BitVec 64)) :=
  (walk atoms (2 ^ atoms.length - 1) (pack size []) 0).get (by
    obtain ⟨packed, completed, _⟩ := empty_walk_refines atoms unique
    simp [completed])

/-- The represented visit sequence is exactly the existing proper-subset
    counter's sequence. This also fixes its order and number of logical visits. -/
theorem states_semantics {size : Nat} (atoms : List (Fin size)) (unique : atoms.Nodup) :
    (states atoms unique).map (denotes size) = (SubsetCounter.states atoms.length).map
      (fun state => TightEvaluation.interpretation
        (FiniteMembership.candidate (SubsetCounter.selected atoms state))) := by
  obtain ⟨packed, completed, semantic, _⟩ := empty_walk_refines atoms unique
  have actual : walk atoms (2 ^ atoms.length - 1) (pack size []) 0 = some (states atoms unique) :=
    (Option.some_get _).symm
  have same := Option.some.inj (completed.symm.trans actual)
  exact same ▸ semantic

/-- Every emitted packed subset has exact storage and zero unused bits. -/
theorem states_shape {size : Nat} (atoms : List (Fin size)) (unique : atoms.Nodup)
    (words : List (BitVec 64)) (visited : words ∈ states atoms unique) :
    words.length = count64 size ∧ ZeroPadding size words := by
  obtain ⟨packed, completed, _, shapes⟩ := empty_walk_refines atoms unique
  have actual : walk atoms (2 ^ atoms.length - 1) (pack size []) 0 = some (states atoms unique) :=
    (Option.some_get _).symm
  have same := Option.some.inj (completed.symm.trans actual)
  exact shapes words (same ▸ visited)

/-- The actual packed carry visits exactly the proper semantic subsets of the
    supplied distinct atoms. Positional coverage comes from `SubsetCounter`;
    `states_semantics` supplies its proved connection to the mutable words. -/
theorem proper_iff_visited {size : Nat} (atoms : List (Fin size)) (unique : atoms.Nodup)
    (J : Atoms (Fin size)) :
    Ferraris.ProperSub J (TightEvaluation.interpretation (FiniteMembership.candidate atoms)) ↔
      ∃ words ∈ states atoms unique, denotes size words = J := by
  have semantic := states_semantics atoms unique
  have same_members := congrArg (fun interpretations => J ∈ interpretations) semantic
  simp only [List.mem_map] at same_members
  rw [same_members]
  exact SubsetCounter.proper_iff_visited atoms unique J

/-- Boolean atom truth read from one packed interpretation. -/
def truth {size : Nat} (words : List (BitVec 64)) (atom : Fin size) : Bool :=
  contains size words atom.val

/-- Search the computed packed visits for a model of the frozen reduct. -/
def hasCountermodel {size : Nat} (atoms : List (Fin size)) (unique : atoms.Nodup)
    (table : List (DagSharing.Node (Fin size))) (roots : List Nat) : Bool :=
  (states atoms unique).any (fun words => TightEvaluation.rootsTrue
    (ReductEvaluation.values (FiniteMembership.candidate atoms) (truth words) table) roots)

/-- Packed search succeeds exactly when the existing finite membership search
    does. Soundness uses proved properness of every emitted packed state;
    completeness uses the packed state's constructive coverage witness.
    The computed reduct evaluator supplies satisfaction, not an oracle premise. -/
theorem countermodel_search_iff {size : Nat} (atoms : List (Fin size)) (unique : atoms.Nodup)
    (table : List (DagSharing.Node (Fin size))) (roots : List Nat) :
    hasCountermodel atoms unique table roots = true ↔
      FiniteMembership.hasCountermodel atoms table roots = true := by
  rw [FiniteMembership.has_countermodel_iff]
  simp only [hasCountermodel, List.any_eq_true]
  constructor
  · rintro ⟨words, visited, modeled⟩
    have proper := (proper_iff_visited atoms unique (denotes size words)).mpr ⟨words, visited, rfl⟩
    have model := (ReductEvaluation.roots_true_iff (FiniteMembership.candidate atoms)
      (truth words) table roots).mp modeled
    exact ⟨denotes size words, proper, model⟩
  · rintro ⟨J, proper, modeled⟩
    obtain ⟨words, visited, represents⟩ := (proper_iff_visited atoms unique J).mp proper
    refine ⟨words, visited, ?_⟩
    apply (ReductEvaluation.roots_true_iff (FiniteMembership.candidate atoms)
      (truth words) table roots).mpr
    change Ferraris.Models (denotes size words) _
    rw [represents]
    exact modeled

/-- Original satisfaction followed by the proved packed proper-subset search.
    This finite decision has no machine resource-interruption channel. -/
def check {size : Nat} (atoms : List (Fin size)) (unique : atoms.Nodup)
    (table : List (DagSharing.Node (Fin size))) (roots : List Nat) : Bool :=
  TightEvaluation.rootsTrue (TightEvaluation.values (FiniteMembership.candidate atoms) table) roots &&
    !hasCountermodel atoms unique table roots

/-- The computed checker using packed counter writes accepts exactly the
    answer sets of the denoted formula theory. This composes the established
    representation/coverage bridge with existing original and reduct semantics. -/
theorem check_iff_answer_set {size : Nat} (atoms : List (Fin size)) (unique : atoms.Nodup)
    (table : List (DagSharing.Node (Fin size))) (roots : List Nat) :
    check atoms unique table roots = true ↔
      Ferraris.Stable (TightEvaluation.interpretation (FiniteMembership.candidate atoms))
        (DagSharing.assertions table roots) := by
  have equivalent := countermodel_search_iff atoms unique table roots
  have same : hasCountermodel atoms unique table roots =
      FiniteMembership.hasCountermodel atoms table roots := by
    cases packed : hasCountermodel atoms unique table roots <;>
      cases reference : FiniteMembership.hasCountermodel atoms table roots <;> simp_all
  simpa only [check, same, FiniteMembership.check] using
    FiniteMembership.check_iff_answer_set atoms table roots

/-- Scan the entire finite atom universe in increasing order, retaining exactly
    the original packed candidate's true coordinates. This is the selected-ID
    producer used by the final packed entry point. -/
def selectedAtoms (size : Nat) (words : List (BitVec 64)) : List (Fin size) :=
  (List.finRange size).filter (truth words)

/-- The universe scan constructs distinct coordinates; no caller-supplied
    uniqueness witness is needed by the packed entry point. -/
theorem selected_atoms_nodup (size : Nat) (words : List (BitVec 64)) :
    (selectedAtoms size words).Nodup := by
  have distinct : (List.finRange size).Nodup := by
    apply List.pairwise_iff_getElem.mpr
    intro first second firstBound secondBound before
    have different : first ≠ second := Nat.ne_of_lt before
    simpa only [List.getElem_finRange, ne_eq, Fin.ext_iff, Fin.val_cast] using different
  exact distinct.filter _

/-- Filtering retains the concrete universe scan's increasing atom order. -/
theorem selected_atoms_ordered (size : Nat) (words : List (BitVec 64)) :
    (selectedAtoms size words).Pairwise (· < ·) := by
  have increasing : (List.finRange size).Pairwise (· < ·) := by
    apply List.pairwise_iff_getElem.mpr
    intro first second firstBound secondBound before
    simpa only [List.getElem_finRange, Fin.lt_def, Fin.val_cast] using before
  exact increasing.filter _

/-- The produced selection's Boolean membership is exactly the original packed
    membership at every bounded coordinate. Coverage follows from finRange;
    no selection-completeness premise is assumed. -/
theorem selected_atoms_truth (size : Nat) (words : List (BitVec 64)) :
    FiniteMembership.candidate (selectedAtoms size words) = truth words := by
  funext atom
  simp [FiniteMembership.candidate, selectedAtoms]

/-- A represented input supplies the same semantic candidate after the universe
    scan. Its existing invariant also justifies each bounded packed read through
    `coordinate_stored`; the scan does not validate malformed input storage. -/
theorem selected_atoms_representation {size : Nat} (words : List (BitVec 64))
    (chosen : List (Fin size)) (represented : Represents words chosen) :
    TightEvaluation.interpretation (FiniteMembership.candidate (selectedAtoms size words)) =
      TightEvaluation.interpretation (FiniteMembership.candidate chosen) := by
  rw [selected_atoms_truth]
  exact (represented_shape words chosen represented).1

/-- Starting from packed truth, construct its distinct selected coordinates and
    execute the packed subset checker. The source representation is immutable. -/
def checkPacked (size : Nat) (words : List (BitVec 64))
    (table : List (DagSharing.Node (Fin size))) (roots : List Nat) : Bool :=
  check (selectedAtoms size words) (selected_atoms_nodup size words) table roots

/-- The complete authored packed checker accepts exactly the answer sets of its
    input packed interpretation. Selected-ID coverage and distinctness are now
    computed and proved, not caller obligations. This total mathematical law
    uses the existing denotation even for malformed storage; runtime admission
    and memory-safe reads still require the explicit representation invariant. -/
theorem check_packed_iff_answer_set (size : Nat) (words : List (BitVec 64))
    (table : List (DagSharing.Node (Fin size))) (roots : List Nat) :
    checkPacked size words table roots = true ↔
      Ferraris.Stable (denotes size words) (DagSharing.assertions table roots) := by
  have exact := check_iff_answer_set (selectedAtoms size words)
    (selected_atoms_nodup size words) table roots
  rw [selected_atoms_truth] at exact
  exact exact

/-- A carry across a physical word boundary preserves the selected population. -/
theorem carry_crosses_word_boundary :
    carry ([63, 64] : List (Fin 65)) (pack 65 [63]) 1 = some (pack 65 [64], 1) := by decide

end Zetesis.Refinement.PackedSubsets
