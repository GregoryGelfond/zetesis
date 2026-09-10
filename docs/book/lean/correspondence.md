# Connecting proofs to implementations

The reduct is the common semantic foundation. Connecting it to an executable
solver requires several separate arguments; a theorem at one level cannot
silently stand for all of them.

| Layer | Existing mathematical or executable object | Additional correspondence required |
| --- | --- | --- |
| Semantic definition | Predicate interpretations, rules, formula trees, reduct and minimality | Relate the intended source language to these definitions |
| Source transformation | Laws over finite bindings, eligible tuples and complete families | Prove concrete cursor coverage, local scope, provenance and translation |
| Evaluation | Least-consequence laws and frozen-mask equivalence | Relate Rust DAGs, dense IDs and packed membership to their denotations |
| Scheduling | Independent-world and completed-batch laws | Prove actual buffers, snapshots, submission identities and commits implement that schedule |
| Machine execution | Checked Rust behavior and qualified WGSL paths | Establish executable refinement, arithmetic and device semantics |
| Observation | Semantic coverage and delivery laws | Connect actual output writes and counters to the retained semantic evidence |

The normal-rule and Ferraris foundations retain independent definitions.
The [NormalFerraris bridge](normal-rules.md) proves their equivalence under the
specified normalized-rule translation, including its filtered/direct-map
distinction. It does not verify `from_ground_program`, atom interning or formula
DAG construction. Likewise, `FrozenReduct` is a Rust representation of a fixed
candidate's reduct; its existence does not close the Rust-to-Lean mask
correspondence.

The head-element laws assume a correctly identified activity family. Explicit
aggregate elements use complete tuple keys; ordinary Boolean choices use original
source occurrences, with local witnesses coalesced within an occurrence. Ordinary
atomic choices distinguish their default-negation sign as well as their atom.
The Rust
source adapter checks syntax-tree and provenance correspondence before retaining
those keys. Tests cover duplicate rules, separate files and finite interpretations;
proving this adapter implements the Lean family remains a separate obligation.

The shared Rust
[`HeadLiteral`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_ir.rs)
retains the sign and operand. Its `positive_atom` operation controls producer
eligibility, while
[`Builder::head_literal`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_ground.rs)
constructs signed truth. This is the implementation distinction corresponding to
the signed activity and permission laws. The finite `M/J` tests inspect the whole
admitted theory, including necessary-support guards and candidate-frozen bounds;
they do not establish pointwise equivalence between unguarded source formulas and
every internal activity node.

[`OrderedHeadActivity`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/OrderedHeadActivity.lean)
relates selected signed tuple activity to an ordered value reduction. In Rust,
[`contribution`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_head_aggregate.rs)
borrows the complete first tuple value, and `HeadContributions` retains it for
the existing value-extremum lowering. The laws require complete-key coverage,
comparison properties and a logical empty value. They do not prove that the
Rust comparator, source join or charged value copy realizes those premises.

[`OrderedBounds`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/OrderedBounds.lean)
separates a numeric measure from a logical bound whose order against every
integer is the same. `constant_aggregate` requires a complete tuple-mask carrier
and constant comparison on every mask; it then preserves both original truth
and arbitrary frozen `M/J` queries. `measured_head_in_context` preserves the
separate positive permissions when the bound is replaced. In Rust,
[`numeric_comparison`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_ground.rs)
uses the existing logical comparator after source and tuple validation. The
concrete comparator, binding coverage and resource accounting remain executable
correspondence obligations. Excluding nonnumeric bounds from the numeric count-plan
certificate is a separate runtime admission rule, not a consequence granted by
the theorem.

[`ObjectivePriorities`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/ObjectivePriorities.lean)
keeps weight, priority, tuple and eligibility in one resolved row.
`completed_presence` assumes exact eligible binding coverage; `partition_vector`
and `partition_optima` preserve the cost vector and all optimal ties under a fixed
priority layout. The Rust
[`Preparation::specialize`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_ground/objectives.rs)
resolves each template from the same binding while retaining its positive
conditions. These laws do not prove that possible aggregate support establishes
priority presence, that the source join is complete, or that checked arithmetic
and bounded execution implement mathematical evaluation. Objective laws also do
not establish search completion or alter answer-set acceptance.

[`AggregateInvariants`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/AggregateInvariants.lean)
states when optional tuple keys cannot change a required sum or extremum.
The source [measure carrier](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_objective_dependencies/presence/flat/carrier.rs)
coalesces complete keys and classifies required and possible activity.
An invariant result is the singleton case.
[`SourceMeasures`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/SourceMeasures.lean)
proves finite carrier membership and coverage of actual active selections.
Optional keys need not be independent in an answer. The concrete source
classification, carrier reduction, checked arithmetic and unary transport remain
implementation obligations. The mathematical list enumerator is a reference
definition; it does not verify the Rust subset-sum or extremum operations.

[`IntegerEnvelopes`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/IntegerEnvelopes.lean)
proves directed bound coverage and integer floor/ceiling laws.
`filtered_bindings_exact` states that a covering envelope followed by the original
guard recovers exactly the satisfying bindings. The Rust
[envelope analysis](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_binding_guard/envelope.rs)
uses these mathematical obligations. Normalizing expressions, scheduling
endpoint inference, checking finite-width arithmetic and enumerating the
resulting intervals remain concrete refinement obligations.

[`ProjectedConditionals`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/ProjectedConditionals.lean)
separates anonymous witness disjunctions, signed source alternatives and
universal condition rows. The Rust
[`Projection`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_ir.rs)
has flat-argument and structural-witness representations. `Builder::project`
completes their witness formula before the conditional compiler applies its
sign. Structural matching uses a private binding frame whose input prefix
excludes later outer bindings. This implements the intended quantifier order;
source support completeness, frame construction and matcher correspondence
remain separate proof obligations. A resource stop cannot establish an empty
completed carrier.

## Read hypotheses as caller obligations

For a frozen mask, correctness means agreement with the fixed candidate's
classical truth, not merely matching dimensions. For lazy inference, final
source coverage means coverage at the final positive snapshot, not a successful
earlier scan. For an aggregate, a finite possible carrier is not proof that
every element is realized in a model. For a certificate, original producers and
their rank must satisfy the actual theorem's premises.

The library includes counterexamples to tempting shortcuts: positive
disjunction without a least model, cross-world joins without a common witness,
stale membership omitting newly enabled truth, and empty delivery hiding a valid
answer. These identify the exact invariants an implementation must preserve.

## What the checks establish

`lake build` checks the declared Lean package. Its
[`Audit.lean`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Audit.lean)
and maintained theorem index provide additional declaration and axiom-accounting
checks. An axiom report describes transitive logical dependencies; it is not an
audit of memory safety, allocation, integer overflow or device execution.

Rust unit tests, generated semantic comparisons, resource-boundary tests,
external clingo comparisons and physical GPU checks supply executable evidence
at their respective scopes. Agreement on finite tests is useful but not a proof
of every admitted program. A physical test must establish actual submitted work
on the requested adapter; CPU-authored readback tests alone cannot do that.

The current result is a checked mathematical library together with a tested
native solver. Describing the whole Rust/GPU solver as formally verified would
exceed the established correspondence. Progress consists of closing particular
arrows in this table while preserving explicit limits, not of replacing those
limits with a proof count.

The optional [packed-membership refinement](membership.md) checks the extracted
Rust `Interpretation::contains` body against packed-bit membership under a
storage invariant. It has a separate toolchain and explicit translation and
library-model assumptions; it does not close the whole evaluation layer above.

The [neuromorphic appendix](../appendices/neuromorphic.md) applies the same
discipline to proposed event backends: it separates existing mask and inference
laws from the unproved transport, epoch and completion correspondences.
