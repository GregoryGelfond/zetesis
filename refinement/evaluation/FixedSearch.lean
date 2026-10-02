import SearchSemantics
import SubsetCarry
import CountermodelTrace

open Aeneas Aeneas.Std Result
open ZetesisExtract
open Zetesis Zetesis.Refinement

/-!
# The extracted proper-subset search

The positional rank decreases the number of still-unvisited proper subsets.
Queries and carries are the actual extracted functions, including typed stops.
A successful false verdict refutes every remaining positional state; a true
verdict carries a proper-subset model of the actually frozen reduct.
-/
namespace FixedCountermodelSearch

open CountermodelSemantics CountermodelTrace

/-- The packed state at a query boundary denotes the positional selection and
    carries its exact population. A stopped partial carry need not satisfy this
    invariant, because search terminates at that point. -/
structure Invariant {size : Nat} (atoms : List (Fin size)) (bits : List Bool)
    (state : State) : Prop where
  width : bits.length = atoms.length
  sizeExact : state.subset.theory.value.atoms.val = size
  represented : PackedSubsets.Represents (ScalarSubsets.raw state.subset.words)
    (SubsetCounter.selected atoms bits)
  counted : state.present.val = SubsetCounter.population bits

/-- Meaning of a returned search verdict. False covers the entire remaining
    rank interval, true gives a proper-subset reduct model, and a typed stop
    provides no semantic verdict. Actual calls retain the stopped state. -/
def Report (frozen : FrozenEvaluation) {size : Nat} (atoms : List (Fin size))
    (bits : List Bool) (outcome : Outcome) : Prop :=
  match outcome.result with
  | .Ok true =>
      Ferraris.ProperSub (TightEvaluation.interpretation (Membership.denotes outcome.subset))
        (TightEvaluation.interpretation (FiniteMembership.candidate (atoms.map Fin.val))) ∧
      Ferraris.Models (TightEvaluation.interpretation (Membership.denotes outcome.subset))
        frozen.reduct
  | .Ok false =>
      ∀ tested : List Bool, tested.length = atoms.length →
        SubsetCounter.rank bits ≤ SubsetCounter.rank tested →
        SubsetCounter.full tested ≠ true →
        ¬ Ferraris.Models (TightEvaluation.interpretation (FiniteMembership.candidate
          ((SubsetCounter.selected atoms tested).map Fin.val))) frozen.reduct
  | .Err _ => True

/-- The actual loop has a finite execution and the stated semantic verdict.
    No successful-query, successful-carry or sufficient-budget hypothesis is
    required. The current fixed-observation and sequence models still apply.

    Proof: construct the actual query outcome. True supplies a witness; a typed
    refusal stops. After false, construct the actual carry outcome. A successful
    carry preserves the representation and increases rank by one; recurse on
    the strictly smaller remaining interval. At the full state there is no
    remaining proper subset. Combine the current refutation with the suffix.
-/
theorem calls_refine (frozen : FrozenEvaluation) {size : Nat}
    (atoms : List (Fin size)) (selected : Slice Usize)
    (coordinates : selected.val.map UScalar.val = atoms.map Fin.val)
    (unique : atoms.Nodup) (bits : List Bool) (state : State)
    (invariant : Invariant atoms bits state) :
    ∃ outcome : Outcome,
      Calls frozen.program frozen.values.slice selected state outcome ∧
      Report frozen atoms bits outcome := by
  have sameLength : selected.val.length = atoms.length := by
    simpa only [List.length_map] using congrArg List.length coordinates
  have construct (remaining : Nat) :
      ∀ bits : List Bool, ∀ current : State,
        Invariant atoms bits current →
        2 ^ atoms.length - 1 - SubsetCounter.rank bits = remaining →
        ∃ outcome : Outcome,
          Calls frozen.program frozen.values.slice selected current outcome ∧
          Report frozen atoms bits outcome := by
    induction remaining using Nat.strong_induction_on with
    | h remaining ih =>
      intro bits current valid distance
      by_cases proper : current.present.val < selected.val.length
      · have stored : Membership.Represented current.subset :=
          SearchRepresentation.stored _ current.subset valid.sizeExact valid.represented
        have notFull : SubsetCounter.full bits ≠ true := by
          apply (SubsetCounter.guard_exact bits current.present.val valid.counted).mp
          simpa only [valid.width, sameLength] using proper
        obtain ⟨answer, output, middle, queried, _⟩ := SubsetQueryTotal.query_refines
          frozen.program current.subset frozen.values.slice current.values current.work
          stored frozen.ordered (frozen_covered frozen) frozen.rootsBounded
        cases answer with
        | Err reason =>
          exact ⟨⟨current.subset, output, middle, .Err reason⟩,
            Calls.queryStopped current output middle reason proper queried, trivial⟩
        | Ok accepted =>
          cases accepted with
          | true =>
            refine ⟨⟨current.subset, output, middle, .Ok true⟩,
              Calls.witness current output middle proper queried, ?_⟩
            exact ⟨SearchRepresentation.proper atoms bits current.subset unique valid.width
                notFull valid.sizeExact valid.represented,
              (query_meaning frozen current.subset current.values current.work stored
                true output middle queried).mp rfl⟩
          | false =>
            obtain ⟨nextBits, answer, updated, count, after, incremented, carried, receipt⟩ :=
              SubsetCarry.advance_typed atoms bits selected current.subset current.present middle
                coordinates unique valid.width valid.represented valid.counted proper
            cases answer with
            | Err reason =>
              exact ⟨⟨updated, output, after, .Err reason⟩,
                Calls.carryStopped current output middle updated count after reason proper queried carried, trivial⟩
            | Ok value =>
              cases value
              let next : State := ⟨updated, output, after, count⟩
              have nextInvariant : Invariant atoms nextBits next := by
                refine ⟨(SubsetCounter.increment_length bits nextBits incremented).trans
                    valid.width, ?_, receipt.2.2.2.1, receipt.2.2.2.2.1⟩
                exact (congrArg (fun theory => theory.value.atoms.val) receipt.1).trans
                  valid.sizeExact
              have ranked : SubsetCounter.rank nextBits = SubsetCounter.rank bits + 1 :=
                SubsetCounter.increment_rank bits nextBits incremented
              have shorter : 2 ^ atoms.length - 1 - SubsetCounter.rank nextBits < remaining := by
                have bound := SubsetCounter.rank_bound nextBits
                rw [nextInvariant.width] at bound
                omega
              obtain ⟨outcome, rest, suffix⟩ := ih
                (2 ^ atoms.length - 1 - SubsetCounter.rank nextBits) shorter
                nextBits next nextInvariant rfl
              refine ⟨outcome, Calls.next current next middle outcome proper queried carried rest, ?_⟩
              cases returned : outcome.result with
              | Err reason => simp only [Report, returned]
              | Ok accepted =>
                cases accepted with
                | true => simpa only [Report, returned] using suffix
                | false =>
                  simp only [Report, returned] at suffix ⊢
                  intro tested width lower nonfull
                  by_cases sameRank : SubsetCounter.rank tested = SubsetCounter.rank bits
                  · have same : tested = bits := SubsetCounter.rank_injective tested bits
                      (width.trans valid.width.symm) sameRank
                    subst tested
                    exact query_refutes frozen atoms bits current.subset current.values
                      current.work valid.sizeExact valid.represented output middle queried
                  · exact suffix tested width (by omega) nonfull
      · have atEnd : SubsetCounter.full bits = true := by
          apply (SubsetCounter.full_iff_population bits).mpr
          have bounded := SubsetCounter.population_bound bits
          rw [valid.width] at bounded
          rw [valid.width, ← valid.counted]
          have count := valid.counted
          omega
        refine ⟨⟨current.subset, current.values, current.work, .Ok false⟩,
          Calls.exhausted current (Nat.le_of_not_gt proper), ?_⟩
        intro tested width lower nonfull
        have last : SubsetCounter.rank bits + 1 = 2 ^ atoms.length := by
          simpa only [valid.width] using (SubsetCounter.full_iff bits).mp atEnd
        have bound : SubsetCounter.rank tested < 2 ^ atoms.length := by
          simpa only [width] using SubsetCounter.rank_bound tested
        have fullTested : SubsetCounter.full tested = true :=
          (SubsetCounter.full_iff tested).mpr (by rw [width]; omega)
        exact False.elim (nonfull fullTested)
  exact construct (2 ^ atoms.length - 1 - SubsetCounter.rank bits) bits state invariant rfl

end FixedCountermodelSearch
