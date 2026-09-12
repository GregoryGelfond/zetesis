# Preparing source and interpreting analysis

Use `zetesis_themelios::prepare_formula` to prepare input for zetesis's finite
formula solver and inspect the analysis it retains before eager materialization.
`zetesis_themelios` is zetesis's source bridge; this workflow does not require
constructing an upstream parser or logical program. The returned
`PreparedFormula` owns source, metadata, checked intermediate representation and
the remaining expansion budget. `ground(self)` consumes that preparation and
produces an `AdmittedFormula`; it computes no answer sets.
`zetesis_solve::PreparedInput::formula` then borrows that owner for a session.

```text
source → prepare_formula → PreparedFormula → ground → AdmittedFormula
                              │                           │
                         inspect analysis            borrow in a session
```

The preparation can succeed while grounding later refuses arithmetic or a
resource ceiling. Splitting the calls does not refresh the source expansion
budget. Keeping the returned owner is therefore part of the contract, not merely
a convenience for avoiding another parse.

Configure eager support storage through `FormulaLimits::max_support_bytes`
before preparation. The default is 128 MiB of authored catalog, snapshot, index and
query capacity, including construction scratch; nested atom payloads, allocator/tree
overhead and unrelated state have separate bounds. The CLI exposes the same
allowance as `--max-support-bytes` in `--help-all`. This is an admission limit;
`SolveConfig` applies after the formula owner has already been constructed.

## Know which program was analyzed

The analysis accessors are part of the prepared solver input's inspection
contract. `source_analysis()` describes exactly the value exposed by
`analyzed_program()`. Read `analysis_basis()` before interpreting its properties:

| Basis | Meaning of the retained program |
| --- | --- |
| `NormalizedProgram` | The bounded pool-free normalization of admitted source |
| `DependencyProjection` | A signature/polarity projection whose structural safety and class verdicts do not certify the original source semantics |

An unknown class verdict is not evidence that the program lies outside that
class. Neither kind of analysis establishes satisfiability or authorizes an
execution profile that the admission API refuses.

The distinction is observable for the adopted Boolean choice extension:
`2{#true;#true}2.` has one empty answer set. Its two written Boolean occurrences
must stay distinct in the formula solver's counting family.
By contrast, `2#count{1:#true;1:#true}2.` has no answer set: both elements name
the same complete tuple. Ordinary Boolean choices therefore retain a separate
source-occurrence family while exposing a dependency projection for analysis.
The explicit tuple aggregate does not require that projection by itself.

The following complete example checks preparation, analysis, materialization,
original source locations and the resulting families. It also inspects a located
resource refusal instead of treating failed preparation as inconsistency. The
last two source cases distinguish an empty program's one empty answer from the
empty family of an inconsistent program containing the constraint `:-.`.

```rust
# extern crate zetesis_solve;
# extern crate zetesis_cpu;
# extern crate zetesis_themelios;
{{#include ../examples/source.rs:example}}
```

Run it from the checkout root with:

```sh
cargo run --locked -p zetesis-solve --no-default-features --example book-source
```

## Preserve evidence through materialization

`formula_origins()` associates each emitted theory root with retained original
source locations. Locations identify source spans; they are not a proof of the
root's meaning. Some generated roots have different evidence needs from written
rules, so consumers must not assume that one root always corresponds to one
written statement. The admitted owner also retains objectives and observations
separately from theory roots.

The example checks that emitted root evidence retains the source identity
supplied in `AdmissionOptions`. It leaves syntax traversal to themelios's API
documentation. For the internal occurrence catalog, tuple activity and their
proof obligations, see [source identity in grounding](../architecture/grounding.md#preserving-source-identity).

The public implementation declarations are
[`PreparedFormula`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula/preparation.rs),
[`AnalysisBasis` and `AdmittedFormula`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula.rs).

## Work and retained space

Preparation traverses and retains bounded source/analysis structure; source
occurrence retention and pool expansion consume cumulative expansion resources.
Boolean choice retention charges each selected rule's source span and syntax
nodes as work, its node bound as Values, and a conservative four locations per
node as Origins before copying. Raising and metadata diagnostics retain their
precedence over these copy limits. The original occurrence stream and the
additional selected-rule copies coexist temporarily; this is not streaming
admission or a zero-copy guarantee.
`max_analysis_nodes` bounds visited structure, while `max_analysis_edges` bounds
head/dependency occurrence products before graph allocation. These are different
from source-byte limits and do not bound process RSS.

Grounding then completes possible support and enumerates admitted outer/local
bindings. Join work may grow as a product of relation sizes, and general
aggregate formula construction may be combinatorial even when the final atom
catalog is small. `FormulaLimits` separately bounds substitutions, construction
work, support rounds, caches, origin locations and final atoms/nodes/roots.
Retained space includes those structures and their logical value payloads;
allocator overhead is outside these logical counters. A successful preparation
therefore promises neither cheap grounding nor lazy formula execution.

See [source grounding](../architecture/grounding.md) for binding and coverage
invariants and [completion and resources](outcomes.md) for result accounting.
