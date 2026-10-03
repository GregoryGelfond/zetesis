# Admitting a finite formula theory

[`TheoryAdmission.lean`](../Zetesis/TheoryAdmission.lean) turns executable
dimension, node and root checks into the structural premises used by the reduct
evaluator. It belongs to `Zetesis.Refinement.TheoryAdmission`.

`validate` follows the ordering of `Theory::new` in
`crates/zetesis-ferraris/src/theory.rs`: dimensions and padded atom count first,
all nodes in their construction order second, asserted roots last. It reports
limit, atom, edge and root errors separately. A failure prevents subsequent
checks. Passing admission makes no claim of satisfaction or answer-set membership.

## Constructing the structural certificate

`node` checks a stored atom against the declared universe. For a connective,
both children must precede its actual node index. `node_exact` proves that these
checks express exactly `AtomBound` and `DagSharing.ValidNode`.

`scan` increments that index as it visits the list. `scan_append` gives the
composition law for consecutive segments. `scan_extends` constructs a larger
`DagSharing.WellFormed` certificate from each successful check, while
`scan_atoms` derives bounds for every stored atom. Conversely, `scan_complete`
proves that every structurally admitted table passes. Thus `scan_exact` is an
equivalence, not only an implication supplied by a caller.

`scan_refusal_exact` characterizes the other outcome. A refused scan reports
its first refused node's error: every earlier node passes its check at its
actual position, and the reported error is that node's own. A later node's
error cannot be reported in its place.

`root` checks one asserted root against the stored node count, and `rootScan`
walks the asserted roots in order. `rootScan_exact` shows that the scan accepts
exactly when every asserted root names a stored node; `rootScan_refusal_exact`
shows that a refusal is always a root refusal and occurs exactly when some
asserted root names no stored node. A root refusal does not identify which
occurrence failed.

`validate_phases` shows that `validate` is the dimension check, then `scan`,
then `rootScan` over the stored node count, each stopping at its first refusal.
`validate_exact` composes the scan with the dimension checks and root reduction.
Its result derives all evaluator indices from successful validation. The named
`maximum` is the supplied host integer maximum; the model checks
`size + 63 ≤ maximum`, corresponding to successful padded-count addition.
`word_counts_fit` also derives the smaller padding bound and both 64-bit and
32-bit word-count bounds. `export_successor_fits` shows that advancing a live
half-word cursor stays within that maximum.

`validated_reduct_exact` then applies `IndexedEvaluation`: validation supplies
the DAG and root premises, and the evaluator supplies original-mask and frozen
truth correspondence. Neither structural admission nor evaluation correctness
is an oracle assumption.

## Implementation boundary

This is an authored validation algorithm, not a newly extracted `Theory::new`.
It models already represented input lengths using natural numbers. Rust range
iteration, checked arithmetic, vectors, allocation and transfer into an immutable
`Arc` still require concrete correspondence. No theory-owner identity is created
by the Lean result, and the validator does not check interpretation storage.

The node and root predicates are exact for successful validation, and the first
refused node is identified exactly. Error kinds and check ordering are
represented, but this module does not prove source spans, rendered diagnostics
or the complete Rust failure trace. Grounding must still establish that the
proposed theory means the intended source program.
