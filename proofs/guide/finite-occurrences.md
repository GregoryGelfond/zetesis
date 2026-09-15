# Finite occurrences and their scopes

A pool denotes finite alternatives at one source occurrence. Two equal spellings
are still two choices. `FinitePools.product_complete` relates the enumerated
Cartesian product to the independent `Binds` relation: each selected value belongs
to the alternatives for that position. `value_context_complete` transports this
coverage through a supplied constructor or expression context. That context's
arithmetic and source safety have already been admitted; the theorem does not
turn a partial operation into a total one.

The consumer determines where the resulting rows belong. An ordinary head or
body occurrence produces complete rules. `original_in_context`,
`frozen_in_context`, and `stable_in_context` preserve their original and reduct
meaning under explicit complete coverage. They do not flatten multiple
disjunctive rules into one larger head.

Inside a choice, aggregate or objective element, `localOccurrences` instead
retains the original element identity while emitting its local rows. The
`local_occurrences_complete` law gives both directions of this coverage.
`local_activity_complete` then characterizes whether any emitted row activates
a complete key. Choice eligibility, aggregate measurement and objective scoring
have different subsequent operations; this common coverage result does not
identify those operations. Original Boolean choice occurrence keys remain
distinct from ordinary atom keys and complete aggregate/objective tuples.

Universal conditionals use a different quantifier order:
`ConsequentAlternatives.original_semantics` and `frozen_semantics` place an
existential alternative family inside each universal condition row. Signed
atoms keep their logical formulas. Completed scalar comparisons instead use
`GroundGuards.evaluated_guard_equivalent`: their checked Boolean result has the
same original and frozen truth. This justifies collecting scalar value
alternatives into one disjunction only after complete, successful evaluation.
It does not justify short-circuiting a required arithmetic error or letting a
comparison bind a source variable outside its admitted condition scope.

Intervals are another finite generator. `ChoiceIntervals.mem_interval` and
`product_complete` state the inclusive integer and independent-slot premises.
`FiniteValues.input_agreement` describes why a checked data plan depends only on
its declared inputs; its failure law prevents a shortened successful result.
Those laws compose with occurrence coverage when every required scope and value
has been checked. They do not justify erasing unsafe names or required errors
because another range is empty.

The Rust formula frontend distributes syntactic pools in
`formula_pool/terms.rs`, selects occurrence products in `formula_pool.rs` and
`formula_pool/local.rs`, and uses the common `formula_range_ir.rs` value fold for
nested interval expressions. Rule products, universal condition families, local
head alternatives and objective keys are composed by their respective consumers.
The fold and cursors reserve their counted work and logical copied payload before
retaining the expanded family. Their finite limits and located failures remain
runtime contracts, not consequences of the Lean statements.

Residual local pools also need a pool-free input for themelios analysis. The
bounded dependency projection preserves every signed signature and arity. It is
explicitly marked `AnalysisBasis::DependencyProjection`; it is not a semantic
rewrite of a conditional or a certificate about the complete normalized source.
The original source, rule origins and runtime formula remain separate owners.
No result here proves source parsing, Rust execution, machine arithmetic,
allocation bounds, source support completeness or general recursive termination.
