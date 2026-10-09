# Consequences from original streamed constraints

[`StreamedConsequences`](../Zetesis/StreamedConsequences.lean) uses the existing
`Cube`, original Ferraris truth and model predicates. It is a mathematical
library over admitted grounded occurrences, independent of Rust source cursors.

A body records signed atom occurrences and a successfully evaluated scalar
guard. A false scalar result makes that instance inactive; it does not represent
an evaluation fault, missing binding or interrupted check. Occurrences retain
positions in the body list even when their atoms and signs coincide.

`UnitBody` decomposes the list into a prefix, a pivot and a suffix. The pivot's
atom is open, and each occurrence in the prefix and suffix is sure throughout
the region. The original integrity constraint must belong to the supplied
theory. `unit_literal_false` then follows in three steps:

1. Any model in the region satisfies every non-pivot occurrence.
2. If it also satisfied the pivot, the entire scalar-passing body would hold.
3. That contradicts its satisfaction of the original integrity constraint.

The three sign laws express the resulting atom decision. Falsifying a positive
or double-negated literal cuts its atom; falsifying a default-negated literal
holds its atom. This uses original truth and does not replace double negation
in a frozen reduct. `unit_preserves_models` and `unit_preserves_answers` prove
exact equality of the remaining original models and answer sets before and
after this narrowing. `all_true_refutes` handles a fully true body, including
the empty body, with no need to finish searching the rest of the source.

The test is conservative about aliases. If another occurrence reads the pivot's
open atom, that occurrence cannot be sure, whatever its sign. Therefore
`aliased_open_occurrence_blocks` declines such a unit witness. No distinct-atom
premise is silently assumed, and no complete alias-aware inference is claimed.

`SnapshotConsequence` records a signed decision that every valid interpretation
in one snapshot must obey. `unit_snapshot_consequence` connects this premise to
the original unit rule. `snapshot_consequence_survives` transports it to any
narrower region, even when its atom is no longer open. `falsifyBatch` applies a
finite list of such decisions. `contains_falsify_batch` characterizes exactly
the retained interpretations, and `batch_preserves_valid` proves preservation
from any narrower starting region. `batch_prefix_preserves_valid` states the
same result for a consumed prefix without granting completion of its suffix.
Repeated decisions need no distinctness assumption; conflicting decisions can
retain no interpretation only when the validity premises already exclude one.
`batch_preserves_answers` specializes the result to answer sets while retaining
the original satisfaction and reduct obligations.

For selective rechecking, `SameBoundsOn` requires agreement of both lower and
upper bounds for every atom in a complete dependency. Equal decided masks alone
do not suffice: an atom can switch between held and cut. `literal_sure_unchanged`
and `unit_body_unchanged` preserve the existing occurrence-based tests.
`HasConsequence` denotes either a sure body or a unit witness after successful
scalar evaluation. `consequence_unchanged` preserves this availability when
all its atom reads are unchanged. `completed_template_unchanged` lifts that law
over an explicitly covered instance family and a completed negative scan.
The dependency may overapproximate the possible grounded reads; it need not
contain every atom sharing a predicate. Excluding a changed atom therefore
requires showing that no instance can read it, for example because every relevant
occurrence has a necessary constant or constructor mismatch. The existing law
then applies directly. It does not prove a concrete occurrence matcher, its
coverage across signs and scopes, or its resource and failure behavior.

If a previously checked instance gains a consequence, `consequence_reads_change`
shows that some occurrence reads changed bounds. Otherwise the unchanged-read
law would give the same consequence before, contradicting the completed check.
`positive_changes_cover_consequences` covers the new consequences by positive
occurrences when all changed reads have that sign. A cursor restricted to a
changed positive row must still enumerate every such occurrence, including
aliases, and retain its original scalar and unit tests. The theorem does not
identify source rows or prove that a particular changed atom is now held.

These reuse laws do not prove that a concrete dirty list covers every changed
read, that a retained batch belongs to the current owner or region, or that a
cursor exhausted every required instance. Nor do they license advancing across
arithmetic faults, newly enabled joins or an incomplete scalar family. Bounds
may justify semantic reuse while observable error order or resource prefixes
still require a separate implementation argument. Stopped scans cannot publish
completed-negative knowledge merely because earlier decisions were sound.

`ConsequenceSteps` composes sound signed decisions with a monotone formula
closure that separately preserves the interpretations being sought. Its
`steps_preserve` law retains exactly those interpretations through every finite
prefix. `openCount` counts fresh atoms in a supplied finite carrier; a duplicate
carrier only enlarges the bound. Each pivot must belong to that carrier.
`falsify_decreases_open`, `steps_bounded` and `no_unbounded_steps` prove that
successful decisions cannot continue indefinitely. These laws count completed
decisions, not source visits, expression work, runtime attempts or elapsed time.
All source passes in one concrete candidate closure share a single allowance;
the mathematical progress bound grants no new allowance after a decision.

`no_unit_can_hide_violation` gives a two-open-atom body whose violating complete
interpretation survives without a unit witness. `pending_after_decision_retains_answer`
gives an original answer that remains after useful narrowing and a stopped scan.
Consequently neither no consequence nor interruption establishes exhaustion or
original satisfaction. Final complete original checks and frozen-reduct
minimality remain required, as in
[`StreamedConstraints.stable_iff_completed_partition`](../Zetesis/StreamedConstraints.lean).

The Lean statements do not establish source-to-ground correspondence, query-mode
coverage, canonical row identities, scope or borrowing, arithmetic admission,
shared-allowance settlement, cancellation, atomic decision commit or worker
scheduling. These are concrete implementation obligations. No native Rust or GPU
refinement claim follows from these semantic laws.

In particular, sharing Rust's immutable dependency mapping does not establish
its completeness or owner identity. Combining the held-only and positive-pivot
queries into one at-most-one-open traversal still owes coverage of their union,
including all-held rows, correct evidence coverage across unmapped rows and
truncation on backtracking. `UnitBody` describes a completed semantic witness;
it does not certify that traversal, its arithmetic eligibility or its resource
accounting.
