# From Boolean tables to ranked support

[TightEvaluation](../Zetesis/TightEvaluation.lean) connects concrete finite
evaluation operations to the existing [ranked-support argument](../Zetesis/TightPlans.lean).
A reader may follow the definitions and the main claims without inspecting
every Boolean simplification. The original theory and its Ferraris reduct remain
the semantic basis; the certificate proves the proper-subset obligation for a
checked class.

## Objects and assumptions

`candidate : α → Bool` specifies the true semantic atoms of one interpretation.
The existing `DagSharing.Node` grammar supplies atoms, falsum, conjunction,
disjunction and implication. `values` walks its node table in construction order,
appending one Boolean value computed from earlier entries. `rootsTrue` checks
every listed original root. Sharing an unasserted node does not itself assert a
formula, and evaluating implication here means original classical truth, before
the reduct is formed.

An `IndexedProducer` stores a semantic head and an optional body index. `none`
explicitly means a fact's true body. `some index` reads that body's original truth
from the same candidate's table. `headSupported` reduces all matching producer
bodies by OR; it does not ask whether a body might be true in some other candidate.
The carrier scan requires support for every present atom. The carrier may contain
absent atoms and duplicates, but must contain every atom true in the candidate.

`Represents` states exact head and body-formula equalities between indexed rows and
the `TightPlans.Producer` grammar, with coverage in both directions. This is a
syntactic compiler obligation. It does not assume the Boolean evaluator's truth
or supportedness conclusion. Original producer membership and a strict positive
rank are separate assumptions of the stability certificate.

## The argument

1. `formula_value_true` proves that recursive Boolean evaluation denotes original
   formula satisfaction. The implication case checks the four child truth cases.
2. `node_value_decode` establishes the same property for one stored node, assuming
   its earlier Boolean entries are mapped from earlier formulas. In
   `values_correspond`, the named `prefixInvariant` carries that whole relationship
   through the actual fold. Thus the completed truth table is produced correctly;
   its equivalence is not an input premise.
3. `value_at`, `roots_true_iff` and `body_value_true` transport that correspondence
   to the indexed consumers. All asserted roots are tested, and both body-presence
   cases are explicit.
4. `head_support_true` moves witness rows across the exact syntactic links.
   `support_true_iff` uses complete present-atom coverage to turn the finite scan
   into the mathematical `Supported` predicate. The separate append law permits
   producer partitions to combine with OR even when heads or rows repeat.
   `head_support_group` restricts the scan to one complete head-owner group:
   every grouped supporting row is original, and every original supporting row
   belongs to the queried atom's group. Group order and duplicate multiplicity
   do not affect Boolean support; preserving occurrence counts and charged work
   remains a separate implementation obligation.
5. `computed_verdict_sound` supplies these derived root/support facts to the
   existing ranked-support theorem. `completed_computation_exact` then applies
   the existing exact residual-completion law to the original theory.

The computed verdict rejects a nonmodel, certifies a supported ranked model, or
returns residual. A failed support scan does not manufacture a countermodel.
The final theorem assumes any returned residual answer is exact and that a
completed result exists; it does not prove termination or successful completion.

`unsupported_refutes` separately justifies rejecting a completed failed support
scan when the producers cover the whole original theory. Every stable model of
that grammar is supported by `TightPlans.stable_supported`; exact indexed support
evaluation contradicts that necessity. This rejection needs no rank premise,
but it does need complete producer coverage. The result is **not stable**, which
must remain distinct from failing original satisfaction. A missing certificate,
an unfinished scan or a failure to decode its result does not meet the premise.

These laws let CPU and device support checks consume the same immutable class
certificate. They do not require the same physical schedule. Splitting producer
rows and joining support with OR preserves the mathematical result; each backend
must still establish the original truth table, exact row representation and
complete present-atom scan. A device result naming an unsupported atom must
authenticate that result against this complete certificate before it can become
a refutation instead of an exact-completion request.

## What remains outside the proof

`DagSharing` totalizes unavailable references as falsum. The Boolean fold mirrors
that totalization with false, so its correspondence theorem is total as well.
This is not admission: the runtime must still validate topological indices,
root/body bounds, complete theory identity and atom representation before use.
The Lean operations use lists; their costs do not establish array or device
complexity.

The module proves neither Rust producer extraction nor complete source grounding.
It does not verify packed words, workgroup ownership, atomic OR, synchronization,
memory/work limits, cancellation, ordered witness selection, transfer or readback.
The mathematical OR law permits a parallel reduction; it is not a proof that a
particular WGSL reduction implements that law. Existing batch-accounting laws
remain separate from physical candidate ordering and publication. Device tests,
independent semantic controls and an executable refinement argument address
different remaining obligations.
