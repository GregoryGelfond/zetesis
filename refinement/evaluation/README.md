# Refining formula evaluation

This optional Lean package connects traces of the evaluator extracted from
`zetesis_ferraris::oracle::evaluate` to Ferraris reduct satisfaction. It checks
the actual setup and node-step operations. It does not assume a correct node
evaluator or a correct frozen mask.

The reusable ASP theory lives in [`proofs`](../../proofs/README.md). This package
imports the existing reduct-evaluation sources directly and checks them unchanged
with the extraction backend's Lean 4.31.0. The main library retains Lean 4.33.1.
No semantic definitions are copied or replaced; object files from different Lean
versions are not mixed. `semantic-inputs.sha256` identifies the shared sources.

## Central result

`ReductTrace.completed_reduct_satisfaction` considers two completed traces over
the same stored formula table:

1. The original trace begins with empty output and interpretation `M`.
2. Its actual output becomes the immutable mask of a second trace at `J`.
3. Each returned truth is true exactly when `J` satisfies that formula's
   Ferraris reduct frozen at `M`.

The theorem derives the initial prefix, mask truth and mask coverage. It assumes
packed storage coverage, ordered child indices and the two actual step traces.
It does not require `J` to be a subset of `M`; that restriction belongs to the
subsequent minimality search. Satisfaction alone is not answer-set membership.

## Argument

| Module | Established boundary |
| --- | --- |
| `Membership` | The extracted packed query returns its declared atom's bit |
| `Iteration` | Actual slice/vector operations respect finite bounds |
| `Control` | Poll precedence and bounded work increments |
| `Step`, `Progress` | Exact node value, masked append and position alignment |
| `Setup` | Actual entry operations produce a zero cursor and empty output |
| `Specification` | A finite truth fold and its prefix laws |
| `Trace` | Actual step traces preserve exact prefixes and work counts |
| `Semantics` | Concrete folds equal the existing original/reduct folds |
| `ReductTrace` | Two completed traces establish explicit reduct satisfaction |

All five node forms retain the source's Boolean short circuits. A stop precedes
node evaluation and append, although the iterator has already fetched the node.
An exhausted iterator completes without polling or charging work.

Trace induction preserves the source table, truth prefix, limits and subset
counter. Each successful continuation adds one work unit. Successful exhaustion
returns the full value sequence; a typed stop returns a strictly shorter correct
prefix. `trace_exists` constructs a bounded trace from admitted state; it may
finish with a typed stop and is not a sufficient-budget completion guarantee.
A supplied constant control value is only a witness for that construction.

## Observation boundary

The trace is an authored execution relation whose transitions call the unchanged
generated body. Each call receives its own supplied control value. This allows
fresh observations; it does not assert that all control values describe one
runtime object or a valid shared-memory history.

The atomic external model describes the Boolean observed at one read site in
one invocation. The body polls at most once, reading cancellation at most once
and optionally expiry at most once. It does not represent immutable shared Rust
atomic storage. Only the Relaxed loads used here are modeled; unsupported model
orderings say nothing about which loads are valid in Rust.

**The trace is not yet proved equivalent to the generated whole loop.** That
loop carries its control value forward; the pinned result effect has no changing
external-read event. Repeatedly reusing the observation token would freeze its
value. A temporal/effect correspondence is still needed before these results
can establish runtime whole-loop correctness.

## Representation and trust

The two traces use the same stored table and numeric atom vocabulary. The Arc
value model does not prove Rust pointer identity or owner checks. Aeneas's vector
model records logical elements and checked indices, not allocation capacity or
allocation failure. Unused clock and synchronization fields have tokens but no
modeled operations. Reference counting, destruction, timers, concurrent memory,
machine code and GPU execution remain outside this model.

The Rust compiler, Charon and Aeneas translations, and the correspondence of
library models to Rust, remain trusted boundaries. There are no project axioms,
proof holes or native proof-evaluation shortcuts. The package does not establish
complete `FrozenReduct` refinement, subset-search correctness, source grounding
or end-to-end solver verification.

## Extraction identity and reproduction

The generated types and functions come from the production Rust method without
a new harness or a Rust source change. Two explicit normalizations are recorded:
public LLBC uses a portable extraction-destination path, and the debug name of
local 1 in `evaluate` changes from `theory` to `program` to avoid a generated
namespace collision. Operands use unchanged local IDs. Restoring the fields
restores the parsed original; operations, types, spans and Rust source are
unchanged. Regenerated Lean is retained unedited.

These are audited preprocessing steps, not verified transformations or a claim
of byte-identical raw extraction. `provenance.json` and the source/artifact
inventories identify the exact boundary. The historical
[membership package](../membership/README.md) remains unchanged; its packed-query
argument is instantiated here against the current extraction's types.

The [reproduction guide](REPRODUCING.md) gives pinned setup, strict checks and
regeneration commands. `Audit.lean` covers all authored package theorems;
`SharedAudit.lean` covers the imported semantic declarations. The records qualify
only the hashed sources. This optional package remains outside the main proof
library's declaration count. No compiler, downloaded dependency or cache is
vendored.
