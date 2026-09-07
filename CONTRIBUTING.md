# Contributing to zetesis

zetesis is a native member of the themelios, keryx and morphe estate. Its own
purpose is exact answer-set solving through the reduct, with composable parallel
and device execution. A research milestone narrows supported features; it does
not lower the standards for authored code. The MIT license names Gregory Gelfond.

## Design from the logic programmer's questions

Public semantic operations speak about programs, atoms, interpretations, answer
sets, consequences, objectives and proved conclusions. Read themelios-program and
themelios-analysis as examples: operations describe logical objects and their
properties; their implementation strategy is a lower layer. An unchecked
interpretation is not an answer set. A verified incumbent is not a proved optimum.
A stopped search is not an inconsistency result.

Reuse themelios's program, symbol, provenance and analysis vocabulary at the
source boundary. Native dense IDs, packed relations and formula DAGs are justified
execution representations with explicit validated conversions. Do not flatten
logical values into strings for internal exchange or add another ASP parser.
Keep the reviewed Git dependency pin; sibling estate checkouts are read-only
references unless the user separately authorizes a change there.

Library-first means a consumer can use the capability without clap options,
process invocation, global I/O or parsing rendered prose. Typed configurations,
models, faults, limits and outcomes belong to the capability. Human, JSON and
consumer-specific views derive from those values. A CLI maps arguments, loads
sources, selects views and handles process exit; reusable orchestration belongs
below it. The current remaining orchestration gap is recorded in the
[estate audit](docs/design/library-first-20260907.md).

Use modules within an existing capability by default. Add a crate when it creates
a useful independent dependency boundary, not merely because a diagram has
another box. Reuse one implementation across source and already-admitted input
doors. State ownership, failure semantics and costs on public operations. Keep
pure transformations separate from injected execution effects and resource
accounting; small named operations should make dependencies and parallel work
visible without excessive abstraction.

## Carry the semantic contract into Lean and Rust

Use the same mathematical distinctions in the specification, public APIs and
Lean: original theory, interpretation, satisfaction, frozen reduct, stability,
coverage and objective ordering. Explain any representation correspondence.
State an optimization's applicability and preservation law before its physical
schedule. A class certificate may justify a specialized check; candidate
restrictions never replace the original theory whose stable models are sought.

Separate three obligations: semantic laws, executable representation/refinement,
and measured backend behavior. Existing Lean laws do not certify Rust, shaders,
parsing or hardware. Do not use proof counts or passing regression tests as a
substitute for an unproved correspondence. New proofs use the pinned Lean
package, its audited axioms and source-hash record; no proof holes or hidden
assumptions. Follow the [structured proof convention](proofs/STYLE.md) and
[proof boundary](proofs/README.md).

Keep model identity distinct from its displayed projection, publication distinct
from verification, and resource exhaustion distinct from exhaustive coverage.
Refuse unsupported constructs and exceeded limits explicitly. Interchange may
lose declared source provenance; it may not silently change answer sets, costs,
output or admitted control semantics.

## Verification and review

Follow the workspace's rustfmt, pedantic Clippy, documentation and authored
unsafe-code gates. Explain local lint exceptions at their point of use. Avoid
unbounded recursion on foreign input, unchecked arithmetic and hidden allocation
or fallback behavior. Make malformed input, cancellation, partial results and
writer/device failures intelligible typed outcomes.

Use `scripts/check.sh portable` for portable Rust/Python tests, lint, strict docs
and benchmark correctness. Run `scripts/check.sh oracle` for the relevant external
clingo comparisons, `scripts/check.sh coverage` for both independent 91% line
coverage floors, and `scripts/check.sh proofs` when proof sources or records change.
See [verification instructions](README.md#build-and-check) for the exact scopes
and prerequisites. Tests should exercise semantics and failure boundaries,
including property and adversarial cases; do not mirror the implementation or
weaken gates to accommodate a feature.

Clingo is an external qualification oracle, never a production solver dependency.
Preserve original corpus inputs. Retain complete models, optimal ties, objective
priorities and completion evidence in comparisons. Match candidates and work
where a comparison claims to do so; record toolchains, binary/source identities,
resource limits and actual backend execution. CPU coverage does not qualify a
physical GPU. Performance claims need reproducible end-to-end evidence, including
grounding, solving, output, transfer and exact residual work where applicable.

The repository's [specification](docs/design/zetesis.md) describes the architecture;
[implementation status](docs/implementation.md) and dated verification records
state what is implemented and demonstrated. Update claims when behavior changes,
including limitations. Comprehensibility to an ASP/reduct practitioner is a review
criterion alongside correctness and performance.
