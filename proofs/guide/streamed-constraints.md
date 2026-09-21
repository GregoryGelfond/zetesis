# Checking constraints without retaining their whole ground theory

[`StreamedConstraints`](../Zetesis/StreamedConstraints.lean) connects a bounded
finite scan to the existing constraint-filtering law. The retained theory may
contain arbitrary formulas. The constraints being checked separately must be
original bodies implying falsum; they cannot supply support for an atom.

## What the scan knows

The mathematical source is a finite list of admitted ground occurrences. A
function `body` gives each occurrence's formula; `test` evaluates its original
truth in one fixed candidate. The list describes the complete family without
requiring the Rust implementation to allocate that whole list. Establishing
that the source cursor covers it is a separate obligation.

`scan` consumes bounded fuel. A true body returns its occurrence as a violation.
An observed empty suffix returns `complete`. Zero fuel returns `pending` with
the unread suffix, even if that suffix is empty. Consequently the caller cannot
mistake lack of remaining fuel for observed completion.

`scan_preserves` proves the accounting invariant by induction on fuel:

1. A violation names an occurrence in the supplied source whose test is true.
2. Completion establishes that every test in the source is false.
3. A pending suffix follows a checked prefix whose tests were all false.

Each false head extends the tail's certificate or checked prefix. A true head
already supplies the required witness. The bound `source.length < fuel`
excludes a pending result: each occurrence consumes one step and observing the
empty suffix needs one more. This is a mathematical inspection bound, not the
work charged for constructing or evaluating a Rust source binding.

`scan_complete_iff` and `scan_violation_iff` then give both directions of the
completed verdict. The example `pending_can_hide_violation` checks one false
body and leaves a true one unread. Useful partial work is not satisfaction.

## Partitions and stable models

A partition covers the original occurrence list by permutation of its flattened
parts. This retains all occurrences, including multiplicity. `completed_partition`
proves that every part completes exactly when the original family is clear.
`partition_invariance` permits different chunk sizes and orders with that same
coverage. It preserves the satisfaction verdict, not the first violating
occurrence or diagnostic order. Parallel acceptance still needs every part's
completion; no law turns an unfinished partition into a false body.

The semantic connection has a pointwise premise: for every original occurrence,
its Boolean test is true exactly when the candidate satisfies its body formula.
`clear_iff_models` derives satisfaction of all original constraints from those
individual evaluations. It does not assume the desired whole-family result.

Finally, `stable_iff_completed_partition` applies
[`ConstrainedPositive.stable_append_constraints`](../Zetesis/ConstrainedPositive.lean):
the candidate is stable for the retained theory plus these constraints exactly
when it is stable for the retained theory and every covered part completes.
The retained theory need not be positive, and constraint bodies need not be
monotone. A violating constraint rejects a candidate; it does not establish
that no other retained-theory candidate is an answer set.

## Refuting a region before it becomes a candidate

[`StreamedRegions`](../Zetesis/StreamedRegions.lean) supplies a different,
sufficient test over the existing `Cube` and `FormulaBounds.Sure` definitions.
For a region with bounds `L` and `U`, every interpretation `S` under consideration
satisfies `L ⊆ S ⊆ U`. A normalized constraint body is sure when:

- every positive atom is in `L`;
- every double-negated atom is in `L`;
- every default-negated atom is absent from `U`.

`sure_antecedent_iff` connects exactly these tests to the existing
`NormalFerraris.antecedent` translation, including empty bodies. Scalar guards
must already have selected an admitted ground instance. Absence from `L` is
not enough for a default-negated atom: an undecided atom may still belong to `S`.
Double negation reads membership here because this is original truth; it is not
being replaced by a positive atom in a frozen reduct.

`held_body_satisfied` composes those literal readings with `sure_sound`: every
interpretation in the region makes the whole body true. If that body belongs to
an original integrity constraint, each interpretation violates the constraint.
`sure_occurrence_refutes` consequently excludes all answer sets of the arbitrary
retained theory plus the original constraint family from this region. One
authenticated occurrence suffices; finding it need not exhaust the family.

`scan_refutes` connects the bounded scanner to that argument:

1. `scan_preserves` establishes that the returned witness occurs in the source
   and that its sufficient test returned true.
2. The pointwise test-soundness premise establishes a sure body for that witness.
3. `sure_occurrence_refutes` excludes every original answer set in the region.

The conclusion is precisely the soundness premise of `CoverageTree.refuted`
with original Ferraris stability as its acceptance predicate. Existing
`CoverageTree.mem_outputs_iff` then accounts for such a cut inside a completed
coverage tree. This is a mathematical composition; it does not assert that the
Rust traversal constructs Lean trees or already satisfies their invariants.

The same scanner now tests a **sufficient condition**, rather than exact truth
in a fixed candidate. Completion means only that no tested body was sure. The
counterexample `complete_scan_can_miss_violation` leaves one atom undecided:
the test completes without a witness, yet the interpretation containing that
atom violates the constraint. This corresponds to `NotRefuted`, not
`Satisfied`. `pending_can_retain_answer` supplies an answer set in a region
whose scan stopped immediately, so interruption cannot authorize a cut either.
Final candidate checks still require the complete original family and exact
evaluation described above.

### Selecting possible sure witnesses

`necessary_selection_preserves_witness` permits filtering the original occurrence
list with a Boolean `keep` test when every sure body implies `keep = true`.
A retained witness still belongs to the original source; an original sure witness
cannot be filtered out because it satisfies that necessary condition. Thus the
filtered and original lists have a sure witness together. Retained occurrences
may still fail the full body test. This law does not equate interrupted scans or
their resource charges.

`sure_antecedent_iff` supplies the logical basis for positive-row selection: a
ground positive atom in a sure body must be held. Rejecting that exact unheld
row before extending a join therefore cannot discard a sure witness. A weaker
signed-predicate test may retain a template only if its required predicate has
some held atom, or some cut atom for a default-negated occurrence. Such a test
is necessary only when its searched domain contains every corresponding admitted
ground atom. Negative occurrences require the whole original catalog, including
atoms without possible positive support. Positive and double-negated occurrences
require held atoms; default-negated occurrences require cut atoms. Signed
predicate identity, the relevant occurrence's existence, and its ground atom's
membership in that domain are premises of this argument.

The theorem describes selection of ground occurrences. Rust must separately
establish that pre-binding row filters and template gates implement a necessary
selection, preserve each surviving binding's interpretation, and use the current
region's authenticated catalog coordinates. An optional source-row-to-dense-ID map
must distinguish absent correspondence from an unheld atom; conservatively
retaining an unmapped row preserves the necessary-condition direction. Neither
relation-local ordinals nor equal dimensions establish dense atom identity.
The lemma proves no Rust join refinement, arithmetic admission, or scheduling
correspondence. Admission and final exact candidate checks retain their original
complete-family obligations.

## Concrete obligations

Source lowering must establish the finite family, its original occurrence and
outer-binding scopes, complete aggregate witnesses and correct formula bodies.
Arithmetic admission remains independent of candidate truth: completed-family
warnings and refusals cannot depend on chunk size or on which candidate happens
to be checked first. The [arithmetic-family laws](../Zetesis/ArithmeticFamilies.lean)
state that separate outcome contract.

Rust must also establish cursor coverage, candidate and prepared-owner identity,
correct original evaluation, cancellation handling and completion publication.
Replaying a family charges its work again; bounded retained storage does not
bound total replay work. Cache validity, machine arithmetic, allocation, CPU/GPU
scheduling and full candidate enumeration are not proved by this module. The
finite scanner is a formal algorithm over supplied occurrences, not a verified
implementation of the source grounder or the whole solver.

For region checks, the dense coordinates must denote the same original atoms
as the prepared owner, and `L` and `U` must bound the original candidates being
searched. Exact theory identity and region width can authenticate a coordinate
convention; they do not by themselves prove how a raw region was constructed.
Every witnessed row must belong to the admitted source family, all its scalar
guards must hold, and every literal in its body must pass the appropriate
held/cut test. Unsupported local or aggregate scopes cannot be silently omitted
from that witness. The region hook is an original-candidate restriction: applying
it to subsets of a frozen reduct would require a different preservation law.
Resource refusal and cancellation retain their typed failure and accepted
charges; neither becomes a refuted region. Bounded storage does not bound the
cumulative work of replaying the source across many regions.
