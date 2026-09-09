# Preparing source and interpreting analysis

Use `prepare_formula` when you need to inspect source analysis before requesting
eager materialization. It owns the source, checked intermediate representation,
metadata and remaining expansion budget. `ground(self)` consumes that preparation
and produces an `AdmittedFormula`; it computes no answer sets. A session borrows
that coherent owner through `PreparedInput::formula`.

```text
source → prepare_formula → PreparedFormula → ground → AdmittedFormula
                              │                           │
                         inspect analysis            borrow in a session
```

The preparation can succeed while grounding later refuses arithmetic or a
resource ceiling. Splitting the calls does not refresh the source expansion
budget. Keeping the returned owner is therefore part of the contract, not merely
a convenience for avoiding another parse.

## Know which program was analyzed

`analyzed_program()` is the exact input described by `source_analysis()`.
Read `analysis_basis()` before interpreting its properties:

| Basis | Meaning of the retained program |
| --- | --- |
| `NormalizedProgram` | The bounded pool-free normalization of admitted source |
| `DependencyProjection` | A signature/polarity projection whose structural safety and class verdicts do not certify the original source semantics |

An unknown class verdict is not evidence that the program lies outside that
class. Neither kind of analysis establishes satisfiability or authorizes an
execution profile that the admission API refuses.

The distinction is observable for the adopted Boolean choice extension:
`2{#true;#true}2.` has one empty answer set. Its two written Boolean occurrences
must stay distinct even if a set-based logical program coalesces their contents.
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
# extern crate zetesis_cli;
# extern crate zetesis_cpu;
# extern crate zetesis_themelios;
{{#include ../examples/source.rs:example}}
```

Run it from the checkout root with:

```sh
cargo run --locked -p zetesis-cli --no-default-features --example book-source
```

## Preserve evidence through materialization

`formula_origins()` associates each emitted theory root with retained original
source locations. Locations identify source spans; they are not a proof of the
root's meaning. Some generated roots have different evidence needs from written
rules, so consumers must not assume that one root always corresponds to one
written statement. The admitted owner also retains objectives and observations
separately from theory roots.

The private source catalog checks that each retained Boolean choice family
corresponds to an original enclosing rule and its written element occurrences.
It uses the pinned parser and original coordinates, and refuses correspondence
it cannot establish. In particular, merging two whole rules must not combine
their separate counting groups. This source preservation supports the subsequent
lowering; successful correspondence checking does not prove that lowering's
answer-set semantics.

At the formula boundary, tuple activity and atom permission remain independent.
For `{a}.1#count{1:#true:a;1:b}1.`, `a` activates the shared tuple through its
Boolean occurrence; an eligible selected `b` activates that same tuple through
an atomic occurrence. The tuple contributes once, while only atomic head
occurrences can supply atom permission. Neither a true Boolean nor a satisfied
bound supplies support for an atom in its condition. The exact three answers
`{a}`, `{b}` and `{a,b}` are covered by the maintained
[Boolean element contracts](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/boolean_element_contracts.rs).

The implementation declarations are
[`PreparedFormula`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula/preparation.rs),
[`AnalysisBasis`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula.rs),
and the private
[`Catalog`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_choice_source.rs).
`BooleanHeadElements.coalesced_group_in_context` proves preservation for an
assumed complete keyed row family and covering atomic permissions;
`boolean_group_in_context` establishes that a Boolean-only measured head filters
the context's answers without supplying new atom support. Their
[proof boundary](../lean/correspondence.md) leaves source occurrence assignment,
Rust formula construction and execution refinement open.

## Work and retained space

Preparation traverses and retains bounded source/analysis structure; source
catalog recovery and pool expansion consume cumulative expansion resources.
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
