# Refining formula evaluation

This separately built implementation-refinement package connects the generated
`zetesis_ferraris::oracle::evaluate` and `failed_root` functions to satisfaction
of original and Ferraris reduct theories under fixed observation tokens. It
proves the generated scans and derives the frozen mask from original evaluation.

The reusable ASP theory lives in [`proofs`](../../proofs/README.md). This package
imports the existing reduct-evaluation sources directly and checks them unchanged
with the extraction backend's Lean 4.31.0. The main library retains Lean 4.33.1.
No semantic definitions are copied or replaced; object files from different Lean
versions are not mixed. `semantic-inputs.sha256` identifies the shared sources.

## Central result

[`TheorySatisfaction.completed_reduct`](TheorySatisfaction.lean) composes three
successful calls to the actual generated functions over the same stored theory:

1. Original evaluation at `M` computes the mask.
2. Evaluation at `J` uses that actual output as its immutable mask.
3. The root scan returns no failed root exactly when `J` models the asserted
   theory's Ferraris reduct frozen at `M`.

The proof derives setup, mask truth, truth-table length and root-read coverage.
Its premises are packed storage coverage, ordered child indices, roots bounded
by the stored node count, and successful call results under the fixed-token
model. `completed_original` gives the corresponding original-theory result.
Neither theorem assumes semantic correctness of the returned values. `J` need
not be a subset of `M`; satisfaction alone does not establish minimality or
answer-set membership.

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
| `FixedLoop` | The generated loop and entry function return trace-correct outcomes under fixed tokens |
| `Semantics` | Concrete folds equal the existing original/reduct folds |
| `ReductTrace`, `FixedReduct` | Two completed traces, or two successful generated calls, establish per-node reduct satisfaction |
| `RootScan` | The actual root scan returns the first false occurrence or complete success, with exact work and typed stops |
| `RootSemantics`, `TheorySatisfaction` | Generated evaluation and root scanning decide original or reduct theory satisfaction on completion |

All five node forms retain the source's Boolean short circuits. A stop precedes
node evaluation and append, although the iterator has already fetched the node.
An exhausted iterator completes without polling or charging work.

`FixedLoop` constructs the calls made by the generated loop, passing each
returned control value to the next call. It proves termination with the exact
truth prefix and work count, preserving limits and the subset counter. Successful
exhaustion returns the full value sequence; a typed stop returns a strictly
shorter correct prefix. Termination may therefore be a refusal, not successful
evaluation.

`RootScan` preserves stored root order and duplicate occurrences. It returns the
first failing node identifier, charging each tested occurrence including the
false one. A refused tick charges no next-root test and remains a typed stop.
Empty roots complete without polling or work. The checked
[examples](RootScanExample.lean) cover empty roots despite cancellation and a
repeated-root sequence whose first false identifier is not the smallest one.

## Observation boundary

The atomic external model returns the Boolean stored in its supplied token.
The body reads cancellation at most once and optionally expiry at most once.
A single invocation can use fresh observations; `FixedLoop` follows the actual
generated loop's control threading, so repeated reads return fixed values.
Only the Relaxed loads used here are modeled; other model orderings do not
assert which loads are valid in Rust.

`Trace` is broader: each body call may receive a separately supplied control
value. Fixed-loop calls form such a trace; refreshed-control traces need not be
executions of that loop. A checked [example](FixedLoopExample.lean) stops after
one node and work unit when cancellation is refreshed, while the fixed-clear
loop completes two nodes and charges two units. The pinned result effect has no
changing external-read event. Correspondence with varying Rust atomic observations,
shared-object identity and concurrent histories remains open.

## Representation and trust

Both passes use the same stored table and numeric atom vocabulary. The Arc
value model does not prove Rust pointer identity or owner checks. Aeneas's vector
model records logical elements and checked indices, not allocation capacity or
allocation failure. Unused clock and synchronization fields have tokens but no
modeled operations. Reference counting, destruction, timers, concurrent memory,
machine code and GPU execution remain outside this model.

The Rust compiler, Charon and Aeneas translations, and the correspondence of
library models to Rust, remain trusted boundaries. There are no project axioms,
proof holes or native proof-evaluation shortcuts. The package does not establish
refinement of the `FrozenReduct` struct or wrappers, subset search, source
grounding, or end-to-end solver verification.

## Extraction identity and reproduction

The generated types and functions come from the production Rust functions
without a new harness or source change. Three fields are explicitly normalized:
the LLBC extraction destination becomes portable, and local 1 in `evaluate` and
`failed_root` changes from `theory` to `program` to avoid namespace collisions.
Operands retain their local IDs. Restoring these fields restores the parsed
original; operations, types, spans and Rust source are unchanged. Regenerated
Lean is retained unedited.

These are audited preprocessing steps, not verified transformations or a claim
of byte-identical raw extraction. `provenance.json` and the source/artifact
inventories identify the exact boundary. The historical
[membership package](../membership/README.md) remains unchanged; its packed-query
argument is instantiated here against the current extraction's types.

The [reproduction guide](REPRODUCING.md) gives pinned setup, strict checks and
regeneration commands. `Audit.lean` covers all authored package theorems;
`SharedAudit.lean` covers the imported semantic declarations. The records qualify
only the hashed sources. This separately built package remains outside the main
proof library's declaration count. No compiler, downloaded dependency or cache is
vendored.
