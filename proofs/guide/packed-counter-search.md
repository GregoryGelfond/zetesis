# Streaming packed countermodel search

[`PackedCounterSearch.lean`](../Zetesis/PackedCounterSearch.lean) composes the
proved [packed set/clear counter](packed-subsets.md) with
[checked streaming membership](counter-search.md). The namespace is
`Zetesis.Refinement.PackedCounterSearch`.

The executable checker stores a current packed subset and its population. It
queries one subset at a time, returns immediately when a countermodel is found,
and performs another carry only after a false query. It does not materialize a
list of visits. The original formula truth mask is computed once and passed
unchanged to every subset query.

## Structural input and exact reads

`Candidate size` contains a list of 64-bit words, a proof of its exact required
length, and a proof that all raw padding bits are zero. This is a structurally
admitted truth assignment, not an answer set. It has no Rust owner-identity or
borrowing meaning.

`Candidate.ofAtoms` uses the existing packed constructor to establish these
properties from bounded atom coordinates. Repeated coordinates denote one atom.
`Candidate.coordinate_stored` proves that every bounded coordinate read has a
stored word, so the membership accessor's total default is not used by admitted
input. Generated subset storage starts empty and is maintained by the existing
packed representation proofs.

Formula atoms are also bounded coordinates. The final correctness theorems
separately require a `DagSharing.WellFormed` table and bounds for every asserted
root. Together these establish the storage and formula-index preconditions;
they do not assume a correct evaluator, counter or final verdict.

## One mask and one current subset

`check` first calls `IndexedEvaluation.evaluate` on the original packed truth
with no mask. It checks the original roots before constructing selected atom
coordinates. An original false root returns a completed negative verdict.

Otherwise, `PackedSubsets.selectedAtoms` scans the finite universe and retains
the candidate's true coordinates. Its existing proofs establish exact membership,
ascending order and distinctness. The caller supplies no selected list or `Nodup`
premise. Search begins with zero subset words and population zero.

`query` calls checked indexed evaluation on the current packed subset with the
already computed mask, then checks the asserted roots. It never recomputes
original truth. `search` uses the actual population guard: query only while the
population is less than the number of selected coordinates. After a false query,
it invokes the existing `PackedSubsets.carry`, which clears true low positions,
sets the first false position, and updates the population. The full candidate is
never queried as a proper subset.

Fuel counts subset queries, not bit updates or formula nodes. A full state
completes even at zero fuel; a nonfull state at zero fuel returns `none`. A
checked-index failure also remains `none`, never a completed negative verdict.
The structural premises rule out those indexed failures for admitted input.

## Composition establishes the decision

`represented_truth` derives the reference selection's Boolean atom function
from the current packed representation. `query_refines` substitutes that proved
equality into the actual checked evaluator. It supplies query agreement instead
of accepting it as a premise.

`search_refines` proves equality with the existing positional stream at every
fuel allowance, including unfinished runs. The maintained population aligns the
guard; the represented truth aligns the query; `carry_counted` establishes both
invariants for the successor. An early witness requires no successor proof or
later evaluation. `check_refines` extends this result through original evaluation
and the proved selected-coordinate producer.

The remaining theorems reuse the existing `CounterSearch` completion and
membership laws. They introduce no second subset-coverage proof:

- `check_exact` and `check_completes` establish completion at `2^n - 1` queries,
  where `n` is the number of true original candidate atoms.
- `completed_check_exact` proves that every completed result is the finite
  semantic membership decision, even if a smaller allowance stops early with
  original rejection or a countermodel.
- `completed_check_iff_answer_set` makes both outcomes explicit: a completed
  verdict is true exactly when the original packed interpretation is an answer
  set of the original denoted theory.
- `check_iff_answer_set` gives that equivalence at the proved complete bound.

The exponential bound is a mathematical completion measure, not a requirement
that a machine implementation compute or store that number. It does not replace
the solver's work, allocation or cancellation controls.

## Implementation correspondence

The logical schedule follows `zetesis-ferraris/src/oracle.rs::check`: check
original satisfaction, collect selected coordinates, initialize an empty subset,
query proper subsets against a frozen mask, and carry after each completed false query.
This closes the separate packed-representation and streaming-algorithm gap in
the authored Lean models.

It is not a proof of extracted Rust. The structural candidate record does not
establish `Theory` identity, allocation success or borrowing. Concrete vector
reads and writes, machine-sized arithmetic, work and subset counters,
cancellation, and error precedence remain separate correspondence obligations.
Lean lists also do not establish constant-time array access or Rust's memory
costs. The result concerns membership of a supplied candidate, not complete
answer-set enumeration or source grounding.
