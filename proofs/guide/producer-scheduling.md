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
Richer source remains on its existing conservative schedule.

Remaining correspondence obligations include faithful source-to-ground
instances, signed signature matching, complete reverse postings, stable original
IR IDs, packed-set traversal, binding coverage, successful publication, checked
arithmetic and resource/control behavior. The laws are not Rust refinement,
answer-set membership, termination or elapsed-time results. Work and capacity
receipts test different implementation contracts from those mathematical laws.
