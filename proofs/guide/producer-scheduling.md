# Selecting affected possible-head producers

[ProducerScheduling](../Zetesis/ProducerScheduling.lean) treats original producer
occurrences as identities attached to mathematical ground rules. Equal ground
rules may belong to different occurrences. Dropping those labels gives the same
normalized program used by [NormalSupport](../Zetesis/NormalSupport.lean).

An occurrence registers input predicates covering every positive atom of every
ground rule it can instantiate. If a currently enabled body was not enabled in
the old carrier, one body atom is new, so its registered predicate changed.
`unaffected_body_old` states the contrapositive. Signature identity and complete
input registration are premises; a count of predicates cannot replace them.

`selective_step_exact` assumes every affected occurrence is selected and every
old head proposal has already been published. A selected occurrence supplies its
current proposal. An unselected occurrence is unaffected, so its current body
was already old and its head is already retained. Thus selected proposals plus
the current carrier equal the full inflationary producer step. The law permits
conservative extra selections. It does not identify equal heads, bindings or
source occurrences with each other.

The general laws do not fix the proposal function. A producer family proposes
through any `propose : Producer → Atoms → Atoms`. `Stable` says an unaffected
producer proposes nothing over the current carrier that it did not propose over
the old one; under it, `selective_step_exact_of_stable` and
`empty_selection_closed_of_stable` give the same exact step and closure.
`stable_of_reads_only` derives stability for a producer whose proposals depend
only on the carrier's atoms with registered predicates (`ReadsOnly`), provided
the carrier grows: unaffectedness makes every current atom with a registered
predicate old, growth makes every old atom current, so the producer reads equal
carriers. Without growth a lost input could enable a proposal that was not old.
Rule entries are stable without growth (`rule_stable`), so `selective_step_exact`
and `empty_selection_closed` are corollaries with their statements unchanged.

`empty_selection_closed` derives possible-head closure when that selection is
empty. Its history premise is substantive. Zero-positive-input producers have
no changed input predicate and must run at bootstrap. Constraints propose no
head; they remain obligations of original formula emission and membership.

The Rust consumer in
[formula_support/producers](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_support/producers.rs)
first checks the whole normalized positive-flat source and every original IR
occurrence. It reads the existing themelios signed dependency graph and SCC
order. Borrowed signature keys and reverse postings select original rule IDs in
a packed set. Repeated input predicates may mark a bit repeatedly but cannot
duplicate the selected occurrence. Each selected rule still uses the existing
[first-new binding partitions](delta-rounds.md).

Each round reads one fixed support snapshot. Only complete selected scans may
prepare the next wake set, and only successful catalog publication may expose
that next round. A resource failure produces no completed support owner. An
empty affected set still admits the final support round, while avoiding a new
catalog/query snapshot. Full final formula emission is separate and unchanged.

Richer source visits every producer each round, through the
[producer registration](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_support/producers/registration.rs).
A rich producer registers its read set: every atom occurrence in its body and
head element conditions, at every depth. After the first round it is traversed
only when one of those predicates gained rows, which is the general law's
selection.

For the correspondence, let `Q(p, C)` be producer `p`'s possible heads over the
fixed round snapshot `C`, before pruning heads already supported. The proposal
function supplied to `ReadsOnly` is `Q`: equal registered relations must give
equal unpruned proposals. The Rust traversal also tests membership in published
or pending support. Let `H` be that accumulated head set before a traversal,
containing `C`, and `A(p, C, H)` the proposals it retains on successful completion.
The separate pruning obligation is

```text
H ∪ A(p, C, H) = H ∪ Q(p, C).
```

It requires that the traversal introduce no head outside `Q(p, C)` and that
every omitted proposal already belong to `H` or be retained during that
traversal. Applying this equality to each successful selected traversal gives
the selected inflationary step. The general scheduling law applies to `Q`,
under its read-set, growth, wake-coverage and old-head-history premises, and
equates that step with the full one. Publication only appends, supplying the
growth premise. Neither `A` nor a union with the carrier is asserted to satisfy
`ReadsOnly`: membership pruning can read head predicates outside the registered
relations, and the carrier itself can differ on those predicates.

Read-set completeness and the pruning equality remain Rust correspondence
obligations. The registration's
[differential tests](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_support/producers/registration/tests.rs)
compare the atoms published in each round, the emitted theory, objectives and
warnings with a reference that traverses every rich producer in every round,
and assert that the selective run skipped visits. They provide evidence on
those fixtures; they do not prove either obligation for every source program.

Remaining correspondence obligations include faithful source-to-ground
instances, signed signature matching, complete reverse postings, stable original
IR IDs, packed-set traversal, binding coverage, successful publication, checked
arithmetic and resource/control behavior. The laws are not Rust refinement,
answer-set membership, termination or elapsed-time results. Work and capacity
receipts test different implementation contracts from those mathematical laws.
