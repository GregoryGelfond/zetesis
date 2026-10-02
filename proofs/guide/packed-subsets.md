# Packed enumeration of proper subsets

[`PackedSubsets.lean`](../Zetesis/PackedSubsets.lean) proves a representation
bridge from concrete 64-bit word updates to the existing
[proper-subset counter](subset-counter.md). Its namespace is
`Zetesis.Refinement.PackedSubsets`.

The counter layer accepts distinct bounded atom coordinates, `List (Fin size)`
with an explicit `Nodup` premise. Their supplied order determines bit significance.
The final `checkPacked` entry point starts with packed candidate words and derives
those coordinates itself by scanning the finite universe in increasing order.
The subset algorithm starts with zero words, reads and updates the selected
coordinates, and maintains the number of true positions. It derives every packed
visit from those writes; it does not obtain visits by repacking an independently
enumerated family.

## Word updates preserve the representation

Set operations reuse `PackedInterpretations.insert` and `insert_exact`. The
only new word mutation is `clear`, which applies an AND with the complement of
the selected one-bit mask. `clear_exact` proves that exactly that bit becomes
false and every other raw bit, including padding, retains its value. Both set
and clear preserve the word-list length.

`Represents` records exact storage length and exact raw membership at every
natural-number coordinate. Because the represented atoms are bounded, that
membership statement also establishes zero padding. `coordinate_stored` proves
that every admitted read and write has an existing source word; the total
word accessor's zero default does not discharge a missing-storage obligation.

`clear_represents` requires its cleared atom to be absent from the remaining
selection. Otherwise two positional occurrences of one atom could share a bit,
and clearing the first would also erase the second. `absent_selected` derives
that absence from the explicit distinct-coordinate premise.

## The packed carry maintains the count

`carry` follows the selected coordinates from the least significant position.
A true bit is cleared and the supplied population decremented; the first false
bit is set and the population incremented. The function returns immediately
after that set. Overflow returns `none`.

`carry_refines` proves that a successful `SubsetCounter.trackedIncrement` is
executed by those word reads and writes with the same new population and selected
set. It permits an arbitrary supplied natural-number count and makes no claim
about machine underflow for an invalid one. `carry_counted` adds the maintained
population invariant and composes with the previously proved count-update law.
The valid decrements then correspond to clearing positions known to be true.

## A completed walk has exact coverage

`walk` uses the same strict population guard as the reference Rust loop:
continue while the population is below the number of selected coordinates.
`SubsetCounter.guard_exact` relates that guard to the full positional state.
Fuel counts increments; the full state completes without being visited, and
insufficient fuel at a nonfull state remains unfinished.

`walk_refines` derives a packed execution from the completed positional walk.
Each successor is established by `carry_counted`; every emitted word list has
exact length and zero padding. `empty_walk_refines` supplies the initial
invariant and reuses the proved bound of `2^n - 1` increments for `n` selected
atoms. `states` extracts the actually computed packed result using that proof.

`states_semantics` establishes the same ordered sequence of interpretations as
`SubsetCounter.states`. `proper_iff_visited` composes that equality with the
existing constructive coverage theorem: a semantic interpretation is a proper
subset exactly when some emitted packed word list denotes it. No coverage or
enumeration oracle appears as a premise.

`countermodel_search_iff` applies the computed reduct evaluator to these packed
visits and proves the same existential result as `FiniteMembership`'s subset
search. `check_iff_answer_set` adds original satisfaction and establishes exact
answer-set membership. It concerns the formula theory denoted by the existing
finite DAG semantics; malformed-index defaults retain that module's meaning and
are not a substitute for runtime admission.

## Concrete implementation boundary

`selectedAtoms` filters `List.finRange size` by the original candidate's packed
truth. `selected_atoms_nodup` and `selected_atoms_ordered` derive distinctness and
increasing order from that actual finite scan. `selected_atoms_truth` proves that
the resulting list's Boolean membership equals the original packed membership;
there is no supplied selected-list coverage premise. For represented inputs,
`selected_atoms_representation` relates the result to the original semantic atom
set, and `coordinate_stored` justifies every read.

`checkPacked` composes that producer with the proved counter checker.
`check_packed_iff_answer_set` establishes answer-set membership directly for the
input packed interpretation. Thus the final entry point no longer requires
caller-supplied selected coordinates or a distinctness proof. Its total
mathematical denotation uses the existing default for a missing word; it does
not replace the representation invariant required by a concrete implementation.

The corresponding Rust loop is `zetesis-ferraris/src/oracle.rs::check`. After
original satisfaction succeeds, it scans `0..theory.atom_count()` and retains
the candidate's true coordinates in
ascending order. It then initializes zero words, performs the strict population
guard, and uses OR to set a selected bit or AND-complement to clear it during
carry. The proof's set, clear, population and visit behavior match that logical
shape.

This is authored Lean, not extracted Rust. The selected-list producer is now
proved for the finite Lean scan; its correspondence to Rust's range iteration and
vector writes is still a separate implementation obligation. Original candidate
evaluation uses the produced list's membership, proved equal to packed truth;
the tested subsets read their computed packed words. Vector indexing and
mutation, allocation, host-sized arithmetic, exact theory ownership,
cancellation and resource accounting remain separate obligations.

The model materializes a finite list of visits for its coverage theorem. Rust
streams them and may return immediately with a countermodel. The theorem proves
the complete logical visit sequence and the existential membership decision;
it does not equate allocation behavior, work charges or interrupted outcomes.
A complete search has `2^n - 1` possible visits and at most `n` selected-coordinate
updates per increment. Those logical bounds are not a machine-time or memory
claim: Lean's list indexing and updates are not constant-time vector operations.

The separate [streaming search](counter-search.md) proves early termination,
reuse of a computed frozen mask and exact completed verdicts at arbitrary query
allowances. It uses the shared positional counter. The
[packed streaming proof](packed-counter-search.md) composes that control with
these writes without materializing visits; actual Rust refinement remains an
implementation obligation.

The empty candidate has no proper-subset visit. The executable boundary law
`carry_crosses_word_boundary` exercises selected coordinates 63 and 64, where a
carry changes physical words while preserving the population.
