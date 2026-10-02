import Zetesis.SubsetCounter
import Zetesis.IndexedEvaluation

/-!
# Streaming countermodel search with a frozen reduct

The finite counter is consumed one state at a time. A satisfying proper subset
ends the search immediately; exhaustion alone establishes its absence. Formula
reads are checked, and the original truth mask is computed once and reused.

The completed checker is equivalent to answer-set membership on an admitted
formula table and a distinct finite candidate. Fuel exhaustion and failed reads
remain distinct from a completed Boolean result. This authored algorithm proves
neither Rust extraction nor machine budgets, allocation or cancellation.
-/

namespace Zetesis.Refinement.CounterSearch

universe u
variable {α : Type u}

/-- Query each proper counter state until a witness or complete exhaustion.
Fuel bounds visited states; a full state finishes even when no fuel remains. -/
def search (query : List Bool → Option Bool) : Nat → List Bool → Option Bool
  | fuel, bits =>
    if SubsetCounter.full bits then some false else
      match fuel with
      | 0 => none
      | remaining + 1 => do
        let witness ← query bits
        if witness then some true else
          let next ← SubsetCounter.increment bits
          search query remaining next

/-- A completed counter walk and successful queries determine the streaming
result. The computation returns the same existential answer without retaining
the list of visited states. The proof follows a carry only after a false query;
a true query needs no later evaluation. -/
theorem search_exact (query : List Bool → Option Bool) (test : List Bool → Bool)
    (fuel : Nat) (bits : List Bool) (visited : List (List Bool))
    (completed : SubsetCounter.walk fuel bits = some visited)
    (queries : ∀ state ∈ visited, query state = some (test state)) :
    search query fuel bits = some (visited.any test) := by
  induction fuel generalizing bits visited with
  | zero =>
    by_cases full : SubsetCounter.full bits = true
    · have empty : visited = [] := by simpa [SubsetCounter.walk, full] using completed.symm
      simp [search, full, empty]
    · simp [SubsetCounter.walk, full] at completed
  | succ fuel inductionHypothesis =>
    by_cases full : SubsetCounter.full bits = true
    · have empty : visited = [] := by simpa [SubsetCounter.walk, full] using completed.symm
      simp [search, full, empty]
    · cases carried : SubsetCounter.increment bits with
      | none => simp [SubsetCounter.walk, full, carried] at completed
      | some next =>
        cases walked : SubsetCounter.walk fuel next with
        | none => simp [SubsetCounter.walk, full, carried, walked] at completed
        | some tail =>
          have same : visited = bits :: tail := by
            simpa [SubsetCounter.walk, full, carried, walked] using completed.symm
          subst visited
          have current := queries bits List.mem_cons_self
          have remaining := inductionHypothesis next tail walked
            (fun state member => queries state (List.mem_cons_of_mem bits member))
          simp only [search, full, Bool.false_eq_true, ↓reduceIte, current, List.any_cons]
          cases test bits <;> simp [carried, remaining]

/-- Successful completion is unchanged by extra fuel. In particular, a
countermodel found early stays a countermodel, and exhausted search stays
exhausted. This says nothing about an unfinished run. -/
theorem search_completed_extension (query : List Bool → Option Bool)
    (fuel extra : Nat) (bits : List Bool) (result : Bool)
    (completed : search query fuel bits = some result) :
    search query (fuel + extra) bits = some result := by
  induction fuel generalizing bits with
  | zero =>
    by_cases full : SubsetCounter.full bits = true
    · cases extra <;> simpa [search, full] using completed
    · simp [search, full] at completed
  | succ fuel inductionHypothesis =>
    by_cases full : SubsetCounter.full bits = true
    · simpa [Nat.succ_add, search, full] using completed
    · cases queried : query bits with
      | none => simp [search, full, queried] at completed
      | some witness =>
        cases witness with
        | true => simpa [Nat.succ_add, search, full, queried] using completed
        | false =>
          cases carried : SubsetCounter.increment bits with
          | none => simp [search, full, queried, carried] at completed
          | some next =>
            have tail : search query fuel next = some result := by
              simpa [search, full, queried, carried] using completed
            simpa [Nat.succ_add, search, full, queried, carried] using
              inductionHypothesis next tail

/-- Evaluate one selected interpretation against a previously computed mask.
Neither the original candidate nor its mask is reconstructed per query. -/
def query [DecidableEq α] (atoms : List α) (frozen : List Bool)
    (table : List (DagSharing.Node α)) (roots : List Nat) (bits : List Bool) : Option Bool := do
  let truth ← IndexedEvaluation.evaluate
    (FiniteMembership.candidate (SubsetCounter.selected atoms bits)) (some frozen) table
  IndexedEvaluation.roots truth roots

/-- With the original computed mask, every admitted query returns the exact
frozen-reduct truth. No query-correctness premise is supplied by a caller. -/
theorem query_exact [DecidableEq α] (atoms : List α)
    (table : List (DagSharing.Node α)) (roots : List Nat) (bits : List Bool)
    (admitted : DagSharing.WellFormed table)
    (rootBounds : ∀ root ∈ roots, root < table.length) :
    query atoms (TightEvaluation.values (FiniteMembership.candidate atoms) table) table roots bits =
      some (TightEvaluation.rootsTrue
        (ReductEvaluation.values (FiniteMembership.candidate atoms)
          (FiniteMembership.candidate (SubsetCounter.selected atoms bits)) table) roots) := by
  unfold query
  rw [IndexedEvaluation.reduct_exact _ _ table admitted]
  exact IndexedEvaluation.roots_exact _ roots (by
    simpa only [ReductEvaluation.values_length] using rootBounds)

/-- Check original satisfaction, then stream proper subsets against its frozen
truth mask. `none` is unfinished or invalid indexed evaluation, never rejection.
Fuel counts subset queries only; it is not a machine-work budget. -/
def check [DecidableEq α] (fuel : Nat) (atoms : List α)
    (table : List (DagSharing.Node α)) (roots : List Nat) : Option Bool := do
  let frozen ← IndexedEvaluation.evaluate (FiniteMembership.candidate atoms) none table
  let original ← IndexedEvaluation.roots frozen roots
  if original then
    return !(← search (query atoms frozen table roots) fuel (List.replicate atoms.length false))
  else some false

/-- Extra subset-query fuel cannot change a completed membership verdict.
Original satisfaction is independent of that fuel; a completed reduct search
is unchanged by the counter's extension law. Failed reads remain failed reads.
-/
theorem check_completed_extension [DecidableEq α] (fuel extra : Nat) (atoms : List α)
    (table : List (DagSharing.Node α)) (roots : List Nat) (result : Bool)
    (completed : check fuel atoms table roots = some result) :
    check (fuel + extra) atoms table roots = some result := by
  cases frozenResult : IndexedEvaluation.evaluate (FiniteMembership.candidate atoms) none table with
  | none => simp [check, frozenResult] at completed
  | some frozen =>
    cases originalResult : IndexedEvaluation.roots frozen roots with
    | none => simp [check, frozenResult, originalResult] at completed
    | some original =>
      cases original with
      | false => simpa [check, frozenResult, originalResult] using completed
      | true =>
        cases searched : search (query atoms frozen table roots) fuel
            (List.replicate atoms.length false) with
        | none => simp [check, frozenResult, originalResult, searched] at completed
        | some witness =>
          have extended := search_completed_extension (query atoms frozen table roots)
            fuel extra (List.replicate atoms.length false) witness searched
          simpa [check, frozenResult, originalResult, searched, extended] using completed

/-- At the constructive counter bound, checked streaming membership completes
with the same result as finite semantic membership. Original evaluation proves
the mask; counter completeness proves absence of omitted proper subsets.
Distinct candidate atoms are necessary for positional properness. -/
theorem check_exact [DecidableEq α] (atoms : List α) (unique : atoms.Nodup)
    (table : List (DagSharing.Node α)) (roots : List Nat)
    (admitted : DagSharing.WellFormed table)
    (rootBounds : ∀ root ∈ roots, root < table.length) :
    check (2 ^ atoms.length - 1) atoms table roots =
      some (FiniteMembership.check atoms table roots) := by
  let frozen := TightEvaluation.values (FiniteMembership.candidate atoms) table
  let test := fun bits => TightEvaluation.rootsTrue
    (ReductEvaluation.values (FiniteMembership.candidate atoms)
      (FiniteMembership.candidate (SubsetCounter.selected atoms bits)) table) roots
  have searched := search_exact (query atoms frozen table roots) test
    (2 ^ atoms.length - 1) (List.replicate atoms.length false) (SubsetCounter.states atoms.length)
    (SubsetCounter.states_completed atoms.length)
    (fun bits _ => query_exact atoms table roots bits admitted rootBounds)
  have same : (SubsetCounter.states atoms.length).any test =
      FiniteMembership.hasCountermodel atoms table roots := by
    have equivalent := SubsetCounter.countermodel_search_iff atoms unique table roots
    change (SubsetCounter.states atoms.length).any test = true ↔
      FiniteMembership.hasCountermodel atoms table roots = true at equivalent
    exact Bool.eq_iff_iff.mpr equivalent
  have originalRoots := IndexedEvaluation.roots_exact frozen roots (by
    simpa only [frozen, TightEvaluation.values_correspond, List.length_map,
      DagSharing.meanings_length] using rootBounds)
  unfold check
  rw [IndexedEvaluation.original_exact _ table admitted]
  change (IndexedEvaluation.roots frozen roots).bind _ = _
  rw [originalRoots]
  simp only [Option.bind_some]
  rw [searched, same]
  cases original : TightEvaluation.rootsTrue frozen roots <;>
    simp only [frozen] at original <;>
    simp [FiniteMembership.check, original]

/-- The completed streaming decision accepts exactly the answer sets of the
denoted formula theory. Satisfaction and proper-subset coverage are both
derived from the preceding executable algorithms. -/
theorem check_iff_answer_set [DecidableEq α] (atoms : List α) (unique : atoms.Nodup)
    (table : List (DagSharing.Node α)) (roots : List Nat)
    (admitted : DagSharing.WellFormed table)
    (rootBounds : ∀ root ∈ roots, root < table.length) :
    check (2 ^ atoms.length - 1) atoms table roots = some true ↔
      Ferraris.Stable (TightEvaluation.interpretation (FiniteMembership.candidate atoms))
        (DagSharing.assertions table roots) := by
  rw [check_exact atoms unique table roots admitted rootBounds, Option.some.injEq]
  exact FiniteMembership.check_iff_answer_set atoms table roots

/-- Any completed verdict is exact, even when the caller supplies less fuel
than complete enumeration needs. Compare the completed run and the proved
complete run at their common extended fuel; neither verdict can change there.
This does not infer a verdict from `none`, nor require every run to complete. -/
theorem completed_check_exact [DecidableEq α] (fuel : Nat) (atoms : List α)
    (unique : atoms.Nodup) (table : List (DagSharing.Node α)) (roots : List Nat)
    (admitted : DagSharing.WellFormed table)
    (rootBounds : ∀ root ∈ roots, root < table.length) (result : Bool)
    (completed : check fuel atoms table roots = some result) :
    result = FiniteMembership.check atoms table roots := by
  have actual := check_completed_extension fuel (2 ^ atoms.length - 1)
    atoms table roots result completed
  have reference := check_completed_extension (2 ^ atoms.length - 1) fuel
    atoms table roots (FiniteMembership.check atoms table roots)
    (check_exact atoms unique table roots admitted rootBounds)
  rw [Nat.add_comm fuel] at actual
  exact Option.some.inj (actual.symm.trans reference)

/-- A completed positive decision establishes answer-set membership at any
fuel allowance. The rejection and unfinished channels remain separate. -/
theorem accepted_is_answer_set [DecidableEq α] (fuel : Nat) (atoms : List α)
    (unique : atoms.Nodup) (table : List (DagSharing.Node α)) (roots : List Nat)
    (admitted : DagSharing.WellFormed table)
    (rootBounds : ∀ root ∈ roots, root < table.length)
    (accepted : check fuel atoms table roots = some true) :
    Ferraris.Stable (TightEvaluation.interpretation (FiniteMembership.candidate atoms))
      (DagSharing.assertions table roots) := by
  have verdict := completed_check_exact fuel atoms unique table roots admitted rootBounds true accepted
  exact (FiniteMembership.check_iff_answer_set atoms table roots).mp verdict.symm

/-- A discovered witness stops before a later unavailable query is read. -/
theorem witness_stops_search :
    search (fun bits => if bits = [false, false] then some true else none)
      3 [false, false] = some true := by decide

/-- An unfinished proper-subset scan does not establish minimality. -/
theorem insufficient_fuel_is_unfinished :
    search (fun _ => some false) 2 [false, false] = none := by decide

/-- An empty candidate needs no proper-subset query, even with zero fuel. -/
theorem empty_candidate_skips_queries : search (fun _ => none) 0 [] = some false := by decide

end Zetesis.Refinement.CounterSearch
