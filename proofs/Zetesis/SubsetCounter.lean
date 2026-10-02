import Zetesis.FiniteMembership

/-!
# A finite counter over an interpretation's subsets

The first Boolean position is the least significant bit. Increment clears a
prefix of true bits and sets the first false bit; an all-true vector overflows.
A completed walk starts at all false, visits every state before all true, and
excludes that last state. The zero-width walk is empty.

Natural-number ranks prove progress and exact coverage; they are proof measures,
not a machine-word restriction on the executable carry. Decoding positions
against a list of distinct atoms gives exactly its proper subinterpretations.
The supplied atom order fixes bit significance; no ordering on the ambient atom
type and no finite ambient universe are required.

A tracked carry also establishes its population updates and strict count guard.
This is an authored executable mathematical algorithm. Its correspondence to a
Rust packed-word loop, machine arithmetic, cancellation, allocation and bounded
outcome remains a separate refinement obligation.
-/

namespace Zetesis.SubsetCounter

universe u
variable {α : Type u}

/-- The first position contributes one; each subsequent position doubles its
weight. Natural ranks are unbounded mathematical integers. -/
def rank : List Bool → Nat
  | [] => 0
  | false :: rest => 2 * rank rest
  | true :: rest => 1 + 2 * rank rest

/-- True exactly at the full positional selection, including zero positions. -/
def full (bits : List Bool) : Bool := bits.all id

/-- Carry toward later positions. Overflow is explicit and preserves no wrapped
state that could be mistaken for a fresh empty subset. -/
def increment : List Bool → Option (List Bool)
  | [] => none
  | false :: rest => some (true :: rest)
  | true :: rest => (increment rest).map (false :: ·)

/-- A vector's rank is strictly below the size of its finite state space. -/
theorem rank_bound (bits : List Bool) : rank bits < 2 ^ bits.length := by
  induction bits with
  | nil => decide
  | cons bit rest inductionHypothesis =>
    cases bit <;> simp only [rank, List.length_cons, Nat.pow_succ] <;> omega

/-- The full vector is exactly the final rank. This connects the semantic stop
condition to the finite progress measure without imposing a word width. -/
theorem full_iff (bits : List Bool) : full bits = true ↔ rank bits + 1 = 2 ^ bits.length := by
  induction bits with
  | nil => decide
  | cons bit rest inductionHypothesis =>
    cases bit with
    | false =>
      simp only [full, List.all_cons, id_eq, Bool.false_and, Bool.false_eq_true,
        rank, List.length_cons, Nat.pow_succ]
      constructor
      · exact False.elim
      · intro impossible
        omega
    | true =>
      change full rest = true ↔ 1 + 2 * rank rest + 1 = 2 ^ (rest.length + 1)
      rw [inductionHypothesis, Nat.pow_succ]
      omega

/-- Carry overflows exactly at the full vector. No earlier state is terminal. -/
theorem increment_none_iff (bits : List Bool) : increment bits = none ↔ full bits = true := by
  induction bits with
  | nil => simp [increment, full]
  | cons bit rest inductionHypothesis =>
    cases bit <;> simp [increment, full, inductionHypothesis]

/-- Successful carry retains the number of positions. -/
theorem increment_length (bits next : List Bool) (advanced : increment bits = some next) :
    next.length = bits.length := by
  induction bits generalizing next with
  | nil => simp [increment] at advanced
  | cons bit rest inductionHypothesis =>
    cases bit with
    | false =>
      have same : true :: rest = next := Option.some.inj advanced
      exact same ▸ rfl
    | true =>
      cases carried : increment rest with
      | none => simp [increment, carried] at advanced
      | some tail =>
        have same : false :: tail = next := Option.some.inj (by
          simpa only [increment, carried, Option.map_some] using advanced)
        subst next
        simp only [List.length_cons, inductionHypothesis tail carried]

/-- Every successful carry advances by exactly one rank. Clearing the lower
true bits and carrying into the tail are the inductive binary-addition step. -/
theorem increment_rank (bits next : List Bool) (advanced : increment bits = some next) :
    rank next = rank bits + 1 := by
  induction bits generalizing next with
  | nil => simp [increment] at advanced
  | cons bit rest inductionHypothesis =>
    cases bit with
    | false =>
      have same : true :: rest = next := Option.some.inj advanced
      subst next
      simp [rank, Nat.add_comm]
    | true =>
      cases carried : increment rest with
      | none => simp [increment, carried] at advanced
      | some tail =>
        have same : false :: tail = next := Option.some.inj (by
          simpa only [increment, carried, Option.map_some] using advanced)
        subst next
        have higher : rank tail = rank rest + 1 := inductionHypothesis tail carried
        simp only [rank]
        omega

/-- Equal-width Boolean vectors with equal ranks are identical. The low bit
fixes parity, and division by two leaves equal tail ranks. -/
theorem rank_injective (left right : List Bool) (width : left.length = right.length)
    (same : rank left = rank right) : left = right := by
  induction left generalizing right with
  | nil =>
    cases right with
    | nil => rfl
    | cons bit rest => simp at width
  | cons bit rest inductionHypothesis =>
    cases right with
    | nil => simp at width
    | cons other tail =>
      have tailWidth : rest.length = tail.length := Nat.succ.inj width
      cases bit <;> cases other <;> simp only [rank] at same
      · exact congrArg (false :: ·) (inductionHypothesis tail tailWidth (by omega))
      · omega
      · omega
      · exact congrArg (true :: ·) (inductionHypothesis tail tailWidth (by omega))

/-- Number of selected positions, computed independently of the rank. -/
def population : List Bool → Nat
  | [] => 0
  | false :: rest => population rest
  | true :: rest => population rest + 1

/-- A true-bit count cannot exceed the number of available positions. -/
theorem population_bound (bits : List Bool) : population bits ≤ bits.length := by
  induction bits with
  | nil => exact Nat.le_refl 0
  | cons bit rest inductionHypothesis =>
    cases bit <;> simp only [population, List.length_cons] <;> omega

/-- The counter is full exactly when all positions contribute to its population.
This makes a maintained count usable as the stopping condition. -/
theorem full_iff_population (bits : List Bool) :
    full bits = true ↔ population bits = bits.length := by
  induction bits with
  | nil => decide
  | cons bit rest inductionHypothesis =>
    cases bit with
    | false =>
      have bound : population rest ≤ rest.length := population_bound rest
      simp only [full, List.all_cons, id_eq, Bool.false_and, Bool.false_eq_true,
        population, List.length_cons]
      constructor
      · exact False.elim
      · intro impossible
        omega
    | true =>
      change full rest = true ↔ population rest + 1 = rest.length + 1
      simpa only [Nat.add_right_cancel_iff] using inductionHypothesis

/-- Under the count invariant, the strict population test is exactly the
nonfull-state guard. The upper bound rules out a spurious count above the width. -/
theorem guard_exact (bits : List Bool) (present : Nat)
    (counted : present = population bits) : present < bits.length ↔ full bits ≠ true := by
  simp only [counted, ne_eq, full_iff_population]
  have bound : population bits ≤ bits.length := population_bound bits
  omega

/-- Carry while updating a supplied population: clear each leading true bit
and decrement; set the first false bit and increment. Overflow remains explicit.
The correspondence theorem requires the supplied count to be the actual one. -/
def trackedIncrement : List Bool → Nat → Option (List Bool × Nat)
  | [], _ => none
  | false :: rest, present => some (true :: rest, present + 1)
  | true :: rest, present =>
      (trackedIncrement rest (present - 1)).map (fun next => (false :: next.1, next.2))

/-- The actual tracked carry preserves the independent population invariant and
performs exactly the untracked carry. On a leading true bit, the invariant gives
one plus the tail's population, so the decrement cannot underflow. The false-bit
case increments the population of precisely the newly set position.

Induct over the carried prefix. Its decrements expose the tail's exact count;
the induction supplies its updated count, and the cleared prefix adds no bits.
-/
theorem tracked_increment_exact (bits : List Bool) (present : Nat)
    (counted : present = population bits) :
    trackedIncrement bits present =
      (increment bits).map (fun next => (next, population next)) := by
  subst present
  induction bits with
  | nil => rfl
  | cons bit rest inductionHypothesis =>
    cases bit with
    | false => rfl
    | true =>
      cases carried : increment rest <;>
        simp [trackedIncrement, population, inductionHypothesis, increment, carried]

/-- A successful tracked transition retains width, advances one rank and
establishes the next state's count invariant. Thus both the count guard and the
carry correspondence remain available after every step. -/
theorem tracked_increment_preserves (bits next : List Bool) (present nextPresent : Nat)
    (counted : present = population bits)
    (advanced : trackedIncrement bits present = some (next, nextPresent)) :
    increment bits = some next ∧ nextPresent = population next ∧
      next.length = bits.length ∧ rank next = rank bits + 1 := by
  rw [tracked_increment_exact bits present counted] at advanced
  cases carried : increment bits with
  | none => simp [carried] at advanced
  | some result =>
    have same : (result, population result) = (next, nextPresent) := by
      simpa only [carried, Option.map_some, Option.some.injEq] using advanced
    obtain ⟨rfl, rfl⟩ := Prod.mk.inj same
    exact ⟨rfl, rfl, increment_length bits result carried, increment_rank bits result carried⟩

/-- Visit the current state, carry, and continue; the full state is excluded.
Fuel counts successful increments. A full state is completed even with zero
fuel, whereas a nonfull state with zero fuel is explicitly unfinished. -/
def walk : Nat → List Bool → Option (List (List Bool))
  | fuel, bits =>
    if full bits then some [] else
      match fuel with
      | 0 => none
      | remaining + 1 => do
        let next ← increment bits
        let tail ← walk remaining next
        pure (bits :: tail)

/-- Enough increments complete the walk. Its ranks are exactly the consecutive
interval from the initial rank up to, but excluding, the full rank; every state
retains the initial width.

At a full vector the interval is empty. Otherwise carry supplies one larger
rank at the same width. The remaining fuel suffices for that successor, and
prepending the current state extends its exact interval by one. -/
theorem walk_complete (fuel : Nat) (bits : List Bool)
    (enough : 2 ^ bits.length ≤ rank bits + fuel + 1) :
    ∃ visited, walk fuel bits = some visited ∧
      visited.map rank = List.range' (rank bits) (2 ^ bits.length - 1 - rank bits) ∧
      ∀ state ∈ visited, state.length = bits.length := by
  induction fuel generalizing bits with
  | zero =>
    have bound : rank bits < 2 ^ bits.length := rank_bound bits
    have atEnd : full bits = true := (full_iff bits).mpr (by omega)
    have emptyInterval : 2 ^ bits.length - 1 - rank bits = 0 := by omega
    exact ⟨[], by simp [walk, atEnd], by simp [emptyInterval], by simp⟩
  | succ fuel inductionHypothesis =>
    by_cases atEnd : full bits = true
    · have lastRank : rank bits + 1 = 2 ^ bits.length := (full_iff bits).mp atEnd
      have emptyInterval : 2 ^ bits.length - 1 - rank bits = 0 := by omega
      exact ⟨[], by simp [walk, atEnd], by simp [emptyInterval], by simp⟩
    · have canAdvance : increment bits ≠ none := by
        intro overflow
        exact atEnd ((increment_none_iff bits).mp overflow)
      obtain ⟨next, advanced⟩ := Option.ne_none_iff_exists'.mp canAdvance
      have width : next.length = bits.length := increment_length bits next advanced
      have successor : rank next = rank bits + 1 := increment_rank bits next advanced
      have nextEnough : 2 ^ next.length ≤ rank next + fuel + 1 := by
        rw [width, successor]
        omega
      obtain ⟨tail, completed, exactRanks, exactWidths⟩ := inductionHypothesis next nextEnough
      have interval : 2 ^ bits.length - 1 - rank bits =
          (2 ^ next.length - 1 - rank next) + 1 := by
        have bound : rank next < 2 ^ next.length := rank_bound next
        rw [width] at bound ⊢
        omega
      refine ⟨bits :: tail, ?_, ?_, ?_⟩
      · simp [walk, atEnd, advanced, completed]
      · simp only [List.map_cons, exactRanks, interval, List.range'_succ, successor]
      · intro state member
        rcases List.mem_cons.mp member with rfl | member
        · rfl
        · exact (exactWidths state member).trans width

/-- An all-false vector starts at rank zero. -/
theorem zero_rank (width : Nat) : rank (List.replicate width false) = 0 := by
  induction width with
  | zero => rfl
  | succ width inductionHypothesis => simp [List.replicate_succ, rank, inductionHypothesis]

/-- The exact finite bound makes unfinished completion impossible. The result
is extracted from the executed walk, not chosen from a semantic coverage claim. -/
def states (width : Nat) : List (List Bool) :=
  (walk (2 ^ width - 1) (List.replicate width false)).get (by
    obtain ⟨visited, completed, _⟩ := walk_complete (2 ^ width - 1) (List.replicate width false) (by
      rw [List.length_replicate, zero_rank]
      have positive : 0 < 2 ^ width := Nat.pow_pos (by decide)
      omega)
    simp [completed])

/-- The initial counter walk completes at its explicit exponential bound. -/
theorem states_completed (width : Nat) :
    walk (2 ^ width - 1) (List.replicate width false) = some (states width) := by
  exact (Option.some_get _).symm

/-- The visited ranks are precisely zero through the predecessor of the full
rank, in increasing order. Thus no counter state is repeated or skipped. -/
theorem states_ranks (width : Nat) :
    (states width).map rank = List.range' 0 (2 ^ width - 1) := by
  obtain ⟨visited, completed, exactRanks, _⟩ :=
    walk_complete (2 ^ width - 1) (List.replicate width false) (by
      rw [List.length_replicate, zero_rank]
      have positive : 0 < 2 ^ width := Nat.pow_pos (by decide)
      omega)
  have same : visited = states width := Option.some.inj (completed.symm.trans (states_completed width))
  simpa only [same, zero_rank, List.length_replicate, Nat.sub_zero] using exactRanks

/-- Every visited state has the width of the supplied counter. -/
theorem states_width (width : Nat) (bits : List Bool) (visited : bits ∈ states width) :
    bits.length = width := by
  obtain ⟨result, completed, _, exactWidths⟩ :=
    walk_complete (2 ^ width - 1) (List.replicate width false) (by
      rw [List.length_replicate, zero_rank]
      have positive : 0 < 2 ^ width := Nat.pow_pos (by decide)
      omega)
  have same : result = states width := Option.some.inj (completed.symm.trans (states_completed width))
  have belongs : bits ∈ result := same ▸ visited
  simpa only [List.length_replicate] using exactWidths bits belongs

/-- A state is visited exactly when it has the supplied width and is not full.
Completeness obtains its rank in the proved consecutive interval, then uses
equal-width rank injectivity to identify the actual visited vector. -/
theorem states_exact (width : Nat) (bits : List Bool) :
    bits ∈ states width ↔ bits.length = width ∧ full bits ≠ true := by
  constructor
  · intro visited
    have length : bits.length = width := states_width width bits visited
    have rankMember : rank bits ∈ (states width).map rank := List.mem_map.mpr ⟨bits, visited, rfl⟩
    rw [states_ranks] at rankMember
    obtain ⟨index, inside, equal⟩ := List.mem_range'.mp rankMember
    refine ⟨length, ?_⟩
    intro atEnd
    have last : rank bits + 1 = 2 ^ width := by simpa only [length] using (full_iff bits).mp atEnd
    simp only [Nat.one_mul, Nat.zero_add] at equal
    omega
  · rintro ⟨length, notFull⟩
    have bound : rank bits < 2 ^ width := by simpa only [length] using rank_bound bits
    have beforeEnd : rank bits < 2 ^ width - 1 := by
      have notLast : rank bits + 1 ≠ 2 ^ width := by
        intro last
        exact notFull ((full_iff bits).mpr (by simpa only [length] using last))
      omega
    have rankMember : rank bits ∈ (states width).map rank := by
      rw [states_ranks]
      exact List.mem_range'.mpr ⟨rank bits, beforeEnd, by simp⟩
    obtain ⟨state, visited, equal⟩ := List.mem_map.mp rankMember
    have same : state = bits := rank_injective state bits
      ((states_width width state visited).trans length.symm) equal
    exact same ▸ visited

/-- There is one visit for each proper positional subset, including the empty
one when the candidate is nonempty. The full state contributes no visit. -/
theorem states_length (width : Nat) : (states width).length = 2 ^ width - 1 := by
  have exactLength := congrArg List.length (states_ranks width)
  simpa only [List.length_map, List.length_range'] using exactLength

/-- A completed counter never revisits a Boolean vector. Distinct ranks are
already established by the exact consecutive interval. -/
theorem states_nodup (width : Nat) : (states width).Nodup := by
  have forgetRanks (values : List (List Bool))
      (unique : (values.map rank).Nodup) : values.Nodup := by
    induction values with
    | nil => exact List.nodup_nil
    | cons head tail inductionHypothesis =>
      have split : rank head ∉ tail.map rank ∧ (tail.map rank).Nodup := by
        simpa only [List.map_cons, List.nodup_cons] using unique
      exact List.nodup_cons.mpr ⟨fun present =>
        split.1 (List.mem_map.mpr ⟨head, present, rfl⟩), inductionHypothesis split.2⟩
  apply forgetRanks
  rw [states_ranks]
  exact List.nodup_range'

/-- Select atoms position by position, keeping their supplied order. The exact
coverage theorems require equal lengths; this total operation otherwise stops
when either input ends. -/
def selected : List α → List Bool → List α
  | atom :: rest, bit :: bits => if bit then atom :: selected rest bits else selected rest bits
  | _, _ => []

/-- Each equal-width counter decoding is one of the existing finite selections.
The two bit cases are precisely omission and retention of the current atom. -/
theorem selected_is_selection (atoms : List α) (bits : List Bool)
    (width : bits.length = atoms.length) : selected atoms bits ∈ FiniteMembership.selections atoms := by
  induction atoms generalizing bits with
  | nil =>
    cases bits with
    | nil => simp [selected, FiniteMembership.selections]
    | cons bit rest => simp at width
  | cons atom rest inductionHypothesis =>
    cases bits with
    | nil => simp at width
    | cons bit tail =>
      have tailWidth : tail.length = rest.length := Nat.succ.inj width
      have tailSelected : selected rest tail ∈ FiniteMembership.selections rest :=
        inductionHypothesis tail tailWidth
      cases bit with
      | false =>
        exact List.mem_append_left _ tailSelected
      | true =>
        exact List.mem_append_right _ (List.mem_map.mpr ⟨selected rest tail, tailSelected, rfl⟩)

/-- Every finite selection has an equal-width positional representation. This
constructs the bridge from the existing selection algorithm to counter states;
no enumeration-coverage premise is supplied by a caller. -/
theorem selection_has_bits (atoms chosen : List α)
    (selection : chosen ∈ FiniteMembership.selections atoms) :
    ∃ bits, bits.length = atoms.length ∧ selected atoms bits = chosen := by
  induction atoms generalizing chosen with
  | nil =>
    have same : chosen = [] := by simpa only [FiniteMembership.selections, List.mem_singleton] using selection
    exact ⟨[], rfl, same.symm⟩
  | cons atom rest inductionHypothesis =>
    simp only [FiniteMembership.selections, List.mem_append, List.mem_map] at selection
    rcases selection with omitted | ⟨suffix, retained, rfl⟩
    · obtain ⟨bits, width, represents⟩ := inductionHypothesis chosen omitted
      exact ⟨false :: bits, congrArg Nat.succ width, represents⟩
    · obtain ⟨bits, width, represents⟩ := inductionHypothesis suffix retained
      exact ⟨true :: bits, congrArg Nat.succ width, congrArg (atom :: ·) represents⟩

/-- A full positional selection denotes the entire supplied atom list. -/
theorem selected_full (atoms : List α) (bits : List Bool)
    (width : bits.length = atoms.length) (atEnd : full bits = true) :
    selected atoms bits = atoms := by
  induction atoms generalizing bits with
  | nil => cases bits <;> simp_all [selected]
  | cons atom rest inductionHypothesis =>
    cases bits with
    | nil => simp at width
    | cons bit tail =>
      have tailWidth : tail.length = rest.length := Nat.succ.inj width
      cases bit with
      | false => simp [full] at atEnd
      | true =>
        have tailFull : full tail = true := atEnd
        exact congrArg (atom :: ·) (inductionHypothesis tail tailWidth tailFull)

/-- With distinct supplied atoms, a nonfull vector omits a semantic atom.
Without distinctness, omitting one position could retain an equal atom elsewhere.

At a false first bit, that atom cannot reappear in the selected tail. At a true
first bit, the induction supplies a missing tail atom, which distinctness keeps
different from the retained first atom. -/
theorem selected_missing (atoms : List α) (bits : List Bool)
    (unique : atoms.Nodup) (width : bits.length = atoms.length) (notFull : full bits ≠ true) :
    ∃ atom ∈ atoms, atom ∉ selected atoms bits := by
  induction atoms generalizing bits with
  | nil =>
    cases bits with
    | nil => exact False.elim (notFull rfl)
    | cons bit tail => simp at width
  | cons atom rest inductionHypothesis =>
    have headAbsent : atom ∉ rest := (List.nodup_cons.mp unique).1
    have tailUnique : rest.Nodup := (List.nodup_cons.mp unique).2
    cases bits with
    | nil => simp at width
    | cons bit tail =>
      have tailWidth : tail.length = rest.length := Nat.succ.inj width
      cases bit with
      | false =>
        have omitted : atom ∉ selected rest tail := by
          intro present
          exact headAbsent (FiniteMembership.selection_subset rest (selected rest tail)
            (selected_is_selection rest tail tailWidth) atom present)
        exact ⟨atom, List.mem_cons_self, omitted⟩
      | true =>
        have tailNotFull : full tail ≠ true := notFull
        obtain ⟨missing, present, absent⟩ := inductionHypothesis tail tailUnique tailWidth tailNotFull
        have different : missing ≠ atom := by
          intro same
          exact headAbsent (same ▸ present)
        refine ⟨missing, List.mem_cons_of_mem atom present, ?_⟩
        change missing ∉ atom :: selected rest tail
        intro member
        rcases List.mem_cons.mp member with same | stillPresent
        · exact different same
        · exact absent stillPresent

/-- Every visited positional state denotes a proper semantic subinterpretation
of the supplied distinct atoms. Width and nonfullness come from the counter;
subset inclusion comes from its connection to finite selections. -/
theorem selected_proper [DecidableEq α] (atoms : List α) (unique : atoms.Nodup)
    (bits : List Bool) (visited : bits ∈ states atoms.length) :
    Ferraris.ProperSub (TightEvaluation.interpretation (FiniteMembership.candidate (selected atoms bits)))
      (TightEvaluation.interpretation (FiniteMembership.candidate atoms)) := by
  obtain ⟨width, notFull⟩ := (states_exact atoms.length bits).mp visited
  have selection : selected atoms bits ∈ FiniteMembership.selections atoms :=
    selected_is_selection atoms bits width
  have subset : Sub
      (TightEvaluation.interpretation (FiniteMembership.candidate (selected atoms bits)))
      (TightEvaluation.interpretation (FiniteMembership.candidate atoms)) := by
    intro atom present
    exact (FiniteMembership.candidate_member atoms atom).mpr
      (FiniteMembership.selection_subset atoms (selected atoms bits) selection atom
        ((FiniteMembership.candidate_member (selected atoms bits) atom).mp present))
  obtain ⟨missing, present, absent⟩ := selected_missing atoms bits unique width notFull
  refine ⟨subset, ?_⟩
  intro reverseSubset
  exact absent ((FiniteMembership.candidate_member (selected atoms bits) missing).mp
    (reverseSubset missing ((FiniteMembership.candidate_member atoms missing).mpr present)))

/-- Every semantic proper subset has a visited counter representation. First
construct its finite selection using `FiniteMembership.selections_cover`, then
construct its Boolean positions. Properness rules out the excluded full vector.
This direction does not need distinct atoms; distinctness is needed to ensure
that every nonfull vector is a proper semantic subset. -/
theorem proper_has_state [DecidableEq α] (atoms : List α) (J : Atoms α)
    (proper : Ferraris.ProperSub J (TightEvaluation.interpretation (FiniteMembership.candidate atoms))) :
    ∃ bits ∈ states atoms.length,
      TightEvaluation.interpretation (FiniteMembership.candidate (selected atoms bits)) = J := by
  obtain ⟨chosen, selection, represents⟩ := FiniteMembership.selections_cover atoms J proper.1
  obtain ⟨bits, width, chosenBits⟩ := selection_has_bits atoms chosen selection
  have notFull : full bits ≠ true := by
    intro atEnd
    have whole : selected atoms bits = atoms := selected_full atoms bits width atEnd
    have same : TightEvaluation.interpretation (FiniteMembership.candidate atoms) = J := by
      rw [← whole, chosenBits]
      exact represents
    apply proper.2
    intro atom present
    exact same ▸ present
  refine ⟨bits, (states_exact atoms.length bits).mpr ⟨width, notFull⟩, ?_⟩
  rw [chosenBits]
  exact represents

/-- The completed binary counter represents exactly the proper subsets of a
finite interpretation supplied as distinct atoms. Its actual carry and walk
establish all coverage; there is no assumed enumeration oracle.

Soundness follows from the witnessed missing atom. Completeness constructs a
selection for any proper subset and uses the proved consecutive counter ranks.
The ambient atom type may be infinite, and its elements need not be ordered. -/
theorem proper_iff_visited [DecidableEq α] (atoms : List α) (unique : atoms.Nodup)
    (J : Atoms α) :
    Ferraris.ProperSub J (TightEvaluation.interpretation (FiniteMembership.candidate atoms)) ↔
      ∃ bits ∈ states atoms.length,
        TightEvaluation.interpretation (FiniteMembership.candidate (selected atoms bits)) = J := by
  constructor
  · exact proper_has_state atoms J
  · rintro ⟨bits, visited, same⟩
    exact same ▸ selected_proper atoms unique bits visited

/-- Searching the actual counter states with the computed reduct evaluator
finds a witness exactly when the existing finite membership search does.

Both directions transfer the same semantic proper-subset model. Counter
coverage is supplied by `proper_iff_visited`; reduct truth is supplied by
`ReductEvaluation.roots_true_iff`. Neither correspondence is assumed here.
This is equality of successful existential searches, not equality of witness
order, resource cost or short-circuited work.
-/
theorem countermodel_search_iff [DecidableEq α] (atoms : List α) (unique : atoms.Nodup)
    (table : List (DagSharing.Node α)) (roots : List Nat) :
    (states atoms.length).any (fun bits => TightEvaluation.rootsTrue
      (ReductEvaluation.values (FiniteMembership.candidate atoms)
        (FiniteMembership.candidate (selected atoms bits)) table) roots) = true ↔
      FiniteMembership.hasCountermodel atoms table roots = true := by
  rw [FiniteMembership.has_countermodel_iff]
  simp only [List.any_eq_true]
  constructor
  · rintro ⟨bits, visited, modeled⟩
    have proper : Ferraris.ProperSub
        (TightEvaluation.interpretation (FiniteMembership.candidate (selected atoms bits)))
        (TightEvaluation.interpretation (FiniteMembership.candidate atoms)) :=
      selected_proper atoms unique bits visited
    have model := (ReductEvaluation.roots_true_iff (FiniteMembership.candidate atoms)
      (FiniteMembership.candidate (selected atoms bits)) table roots).mp modeled
    exact ⟨_, proper, model⟩
  · rintro ⟨J, proper, modeled⟩
    obtain ⟨bits, visited, represents⟩ := (proper_iff_visited atoms unique J).mp proper
    refine ⟨bits, visited, ?_⟩
    apply (ReductEvaluation.roots_true_iff (FiniteMembership.candidate atoms)
      (FiniteMembership.candidate (selected atoms bits)) table roots).mpr
    rw [represents]
    exact modeled

/-- An empty candidate has no proper-subset counter visit. -/
theorem empty_counter_has_no_visits : states 0 = [] := by decide

/-- The supplied first atom changes fastest; the full candidate is omitted. -/
theorem three_atom_visit_order :
    (states 3).map (selected [2, 7, 9]) =
      [[], [2], [7], [2, 7], [9], [2, 9], [7, 9]] := by decide

/-- Fuel exhaustion before reaching the full state remains unfinished. -/
theorem short_walk_is_unfinished : walk 2 [false, false] = none := by decide

/-- Two cleared low bits and one newly set bit reduce the tracked count by one. -/
theorem carried_population_is_updated :
    trackedIncrement [true, true, false] 2 = some ([false, false, true], 1) := by decide

end Zetesis.SubsetCounter
