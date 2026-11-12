import Zetesis.ObjectiveConditions

/-!
# Objective read sets and compact original-atom coordinates

A closed objective query observes only its atom occurrences, including those
beneath negation. Retaining every requested signed predicate therefore leaves
the query unchanged when truth outside those predicates is replaced by false.
The same predicate selection retains the complete ordered row family of every
requested positive relation. These are read-locality laws for objective values;
they do not identify a displayed projection with a complete interpretation.

An eligibility DAG may start with a compact list of original atom coordinates.
Reading a present prefix position yields the original interpretation's truth at
the coordinate stored there. Original atom positions and compact formula-node
positions remain distinct. Appending and sharing later condition nodes use the
existing ObjectiveConditionTable and DagSharing correspondences. Once the
compiler establishes pointwise condition truth, ObjectiveConditions.condition_vector
and ObjectiveTransport.cost_vector_transport already preserve complete keys,
priorities and costs; this module introduces no second scoring semantics.

The Rust selector must establish complete source coverage, signed predicate and
arity identity, increasing unique original positions and exact binary lookup.
The theorem's present-index premise is not a proof of that search. Source-owner
authentication, topological references, machine arithmetic, work and byte limits,
cancellation and refusal fallbacks remain implementation obligations. Neither
the original theory nor reduct membership is projected by these laws.
-/

namespace Zetesis.ObjectiveProjection

universe u v
variable {A : Type u} {S : Type v}
open ObjectiveConditions (Query evaluate)

/-- All atom occurrences read by a closed query. Repeated occurrences are kept;
deduplication is unnecessary for the agreement premise. Negation retains its
operand's reads, while both Boolean branches contribute their occurrences. -/
def reads : Query A → List A
  | .boolean _ => []
  | .atom atom => [atom]
  | .neg operand => reads operand
  | .conj left right => reads left ++ reads right
  | .disj left right => reads left ++ reads right

/-- Agreement on the query's read set suffices for equal original-model truth.
Induct through the finite query: an atom uses the supplied agreement, negation
uses its operand's equality, and each binary connective uses both branches.
No assumption is made about atoms outside the read set. -/
theorem query_read_locality (left right : A → Bool) (query : Query A)
    (agree : ∀ atom ∈ reads query, left atom = right atom) :
    evaluate left query = evaluate right query := by
  induction query with
  | boolean value => rfl
  | atom atom => exact agree atom (by simp [reads])
  | neg operand induction =>
    have operand_agreement : ∀ atom ∈ reads operand, left atom = right atom := by
      intro atom present
      exact agree atom (by simpa [reads] using present)
    have operand_truth : evaluate left operand = evaluate right operand :=
      induction operand_agreement
    exact congrArg Bool.not operand_truth
  | conj first second first_induction second_induction =>
    have first_agreement : ∀ atom ∈ reads first, left atom = right atom := by
      intro atom present
      exact agree atom (by simp [reads, present])
    have second_agreement : ∀ atom ∈ reads second, left atom = right atom := by
      intro atom present
      exact agree atom (by simp [reads, present])
    have first_truth : evaluate left first = evaluate right first :=
      first_induction first_agreement
    have second_truth : evaluate left second = evaluate right second :=
      second_induction second_agreement
    simp only [evaluate, first_truth, second_truth]
  | disj first second first_induction second_induction =>
    have first_agreement : ∀ atom ∈ reads first, left atom = right atom := by
      intro atom present
      exact agree atom (by simp [reads, present])
    have second_agreement : ∀ atom ∈ reads second, left atom = right atom := by
      intro atom present
      exact agree atom (by simp [reads, present])
    have first_truth : evaluate left first = evaluate right first :=
      first_induction first_agreement
    have second_truth : evaluate left second = evaluate right second :=
      second_induction second_agreement
    simp only [evaluate, first_truth, second_truth]

/-- Selecting complete requested predicates may replace truth outside them by
false without changing a query whose every atom belongs to a requested predicate.
The signature can include sign and arity; its exact representation is a caller
obligation. The claim concerns this query, not whole-model identity or stability. -/
theorem query_predicate_projection (signature : A → S) (requested : S → Bool)
    (model : A → Bool) (query : Query A)
    (covered : ∀ atom ∈ reads query, requested (signature atom) = true) :
    evaluate (fun atom => if requested (signature atom) then model atom else false) query =
      evaluate model query := by
  apply query_read_locality
  intro atom present
  have retained : requested (signature atom) = true := covered atom present
  simp [retained]

/-- Filtering by a predicate read set preserves the entire ordered row sequence
of every included predicate. Payloads, duplicates and source order remain intact.
This is complete only relative to the supplied source rows; it does not establish
that an incomplete grounding carrier contains every needed atom. -/
theorem requested_rows_preserved [DecidableEq S]
    (rows : List A) (signature : A → S) (requested : S → Bool) (target : S)
    (included : requested target = true) :
    (rows.filter (fun row => requested (signature row))).filter
        (fun row => decide (signature row = target)) =
      rows.filter (fun row => decide (signature row = target)) := by
  induction rows with
  | nil => rfl
  | cons row rest induction =>
    by_cases same_signature : signature row = target
    · simp [same_signature, included, induction]
    · cases selected : requested (signature row) <;>
        simp [same_signature, selected, induction]

/-- The mathematical truth vector of an atom-node prefix. Each entry reads its
stored original coordinate; it does not read the compact position as an atom. -/
def prefixTruth (ids : List A) (model : A → Bool) : List Bool := ids.map model

/-- A successfully located compact prefix entry reads the original atom's truth.
No injectivity or density assumption is needed for this one read. The executable
compiler separately establishes increasing unique IDs and that its checked
binary lookup supplies the stated present entry. -/
theorem compact_prefix_read (ids : List A) (model : A → Bool)
    (position : Nat) (atom : A) (present : ids[position]? = some atom) :
    (prefixTruth ids model)[position]? = some (model atom) := by
  simp only [prefixTruth, List.getElem?_map, present, Option.map_some]

end Zetesis.ObjectiveProjection
