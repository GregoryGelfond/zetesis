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

## Reuse original input across profiles

`ParsedSource` owns original bytes, their successful parse and fixed
`AdmissionOptions`. Its consuming profile operations retain that owner on
failure. This is useful when relational admission identifies a construct that
needs the finite formula profile:

```rust
# extern crate zetesis_themelios;
use zetesis_themelios::{AdmissionOptions, ExpansionLimits, FormulaLimits, ParsedSource};

let source = ParsedSource::new("1{a;b}1.".into(), AdmissionOptions::default())?;
let failure = source.admit_extended(ExpansionLimits::default()).unwrap_err();
assert!(failure.error().needs_formula_admission());
let prepared = failure.into_source()
    .prepare_formula(ExpansionLimits::default(), FormulaLimits::default())?;
let admitted = prepared.ground()?;
assert_eq!(admitted.atoms().len(), 2);
# Ok::<(), Box<dyn std::error::Error>>(())
```

The example's known bounded choice requires formula admission. General callers
must inspect `needs_formula_admission()` before choosing that alternative;
undefined arithmetic, syntax and resource refusals are not permissions to retry.
`SourceFailure::error()` exposes the unchanged typed refusal, and `into_parts()`
recovers both it and the parsed owner. The String-taking convenience functions
preserve their existing result and error types.

The automatic CLI route uses this same ownership transfer. It neither clones
the entire stdin buffer nor reparses it on profile retry. The owner retains
O(source bytes + syntax nodes) storage; a failed attempt boxes that owner once.
Each attempt still checks its profile and performs any required raising and
normalization. Source options remain fixed; supplied expansion and formula
limits apply to the requested attempt. Formula grounding consumes the resulting
preparation's remaining budget as before.

Configure eager support storage through `FormulaLimits::max_support_bytes`
before preparation. The default is 128 MiB of authored catalog, snapshot, index and
query capacity, including construction scratch; nested atom payloads, allocator/tree
overhead and unrelated state have separate bounds. The CLI exposes the same
allowance as `--max-support-bytes` in `--help-all`. This is an admission limit;
`SolveConfig` applies after the formula owner has already been constructed.

Choose positive joins separately through the preparation's
`with_grounding_options(GroundingOptions { joins: JoinStrategy::Table })` method.
The default `Indexed` strategy probes existing value postings. `Table` reuses
support masks only for flat positive patterns over completed eager support;
structural patterns and support-growth rounds keep indexed joins. The bundle
preparation exposes the same method, and the CLI maps `--formula-joins table`
to it. The choice retains the preparation's source identity and remaining budgets.

The private query workspace owns cached indices and accounts for simultaneous
selected masks, scratch and cumulative work. Named vector/object capacities are
distinct from nested atom payloads, allocator/tree/control-runtime overhead and
other grounding state; the limit is not RSS. An exhausted table operation returns
a located failure without silently choosing a different strategy. The existing
matcher and required authored-body validation remain shared. Choosing table joins
does not launch ordinary Rayon or GPU grounding; later answer-set execution has
its own policy. See [finite-table selection](finite-tables.md) for the ownership,
applicability and proof boundaries.

## Optional domains during final instantiation

`PreparedFormula::with_domain_analysis(Some(DomainLimits { .. }))` requests
necessary argument-domain guards; `None` is the library default. Bundle
preparation has the same method. The ordinary command requests the analysis
with its default limits. This is separate from choosing `Indexed` or `Table`
joins, and adds no language construct or CLI flag.

The consumer checks the exact normalized whole program and its original rule
occurrences. The initial profile permits ordinary positive flat rules and
constraints, whole named variables, atomic numbers, strings, positive nullary
symbols, infimum and supremum, and body comparisons. It excludes arithmetic in
atoms, generators, negative body literals, structured terms, anonymous/local
scopes and richer heads. `NormalizedProgram` is necessary; a dependency projection never supplies
narrowing. Inapplicability keeps the existing complete path and does not create
a new source refusal.

After unchanged possible-support completion, finite argument domains constrain
each final rule's variable occurrences. Their intersection, less every value a
comparison over that variable alone is defined and false at, can reject a row
that has no complete positive continuation, before copying its new bindings or
opening deeper probes; a guard is prepared only where that set is narrower than
the argument's domain. The surviving rows keep the existing matcher, original
positions and source origins. Support-growth, objective and factorized component
joins retain their existing paths. Global Unknown or Stopped analysis supplies
no guards. An individually Unknown argument is unrestricted; other finite
arguments may still contribute restrictions.

This complete example compares admitted atoms, nodes, roots and provenance,
observes an actual typed `Analysis` and positive guard activity, and checks both
local widening and a stopped analysis. It materializes theories without solving
for answer sets or measuring elapsed time.

```rust
# extern crate themelios_base;
# extern crate zetesis_domain;
# extern crate zetesis_themelios;
{{#include ../examples/domain-grounding.rs:example}}
```

Run its maintained Cargo control or the example itself from the checkout root:

```sh
cargo test --locked -p zetesis-themelios --example book-domain-grounding
cargo run --locked -p zetesis-themelios --example book-domain-grounding
```

`DomainObservation` also distinguishes Disabled and Inapplicable. An Analyzed
callback borrows the exact owner's status, context, argument domains and logical
statistics; retaining a result requires copying the specific owned information
the caller needs. Detailed work reports include applicability, analysis and guard
preparation, even on a failed or stopped prefix. A completed analysis-attempt
phase can have used conservative fallback; it does not imply FixedPoint.

Analyzer `DomainLimits` bound logical populations and inspected source/symbol
bytes. Actual work, including stopped work, consumes the original cumulative
formula budget. The analyzer uses bounded standard collections with no
fallible-allocation or caller-control API; its heap is outside
`FormulaLimits::max_support_bytes`. The guard/query lease separately accounts
its named headers, scratch, temporary atomic payload and actual vector capacity
beside support, table indices and live masks. These contracts give neither a
hard allocator/RSS limit nor cancellation/deadline polling. Guard preparation
and membership costs may outweigh avoided probes. See
[finite-domain ownership and proof boundaries](finite-tables.md#optional-argument-domain-guards)
for the exact named-capacity scope and preservation premises.

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
written statement. The admitted owner also retains objectives, observations and
the completed projection domain separately from theory roots. Authored
`project_selection()` metadata is a declaration plan; the admitted owner's
`projection()` supplies the fixed typed domain for
[projected enumeration](sessions.md#projected-enumeration).

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

## Validate values before materializing them

Source normalization, fact sizing and head preflight use a shared checked symbol
walk. A validation consumer checks the value without building an owned core
value or rendering its spelling. A construction consumer creates the flat node
owner and enters the existing independent core constructor.

[`ValueNode::view()`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-core/src/structured.rs)
returns a borrowed `ValueNodeRef`. Its text and canonical-spelling measures are
shared by source validation and owned construction. The view owns no payload;
it retains node kind, constructor sign and arity, and borrows exact text. It is
not a certificate that the surrounding preorder sequence forms a valid value.

Validation retains logical node/depth bounds and the byte bound for node cells,
referenced text and canonical spelling. Construction additionally admits its
actual node capacity, validation/render frame capacity and rendered-string
capacity. Validation does not charge storage for an output that is never built;
passing it does not guarantee that later materialization fits its actual storage
budget. NUL strings, invalid arithmetic and overflowing arithmetic remain
refusals even in an inactive rule or unused constant. Complete authored-body
validation still runs after an earlier false filter.

## Diagnostic ownership

Typed refusals retain source identity and original error data. Their `Display`
implementations write to the caller's formatter and propagate its failure.
Constant-cycle messages stream the retained names in dependency order without
first joining them into another owned string. A caller can impose a byte ceiling
through its formatting sink; formatting stops at the first refused write.

The themelios human diagnostic API returns an owned rendering for each diagnostic.
zetesis keeps that canonical view. Source indexing, typed diagnostic messages and
that rendering can require temporary storage before the caller receives text;
the sink's byte ceiling does not bound those upstream allocations. Constructing
an owned diagnostic view remains distinct from streaming an already-retained
constant chain.
