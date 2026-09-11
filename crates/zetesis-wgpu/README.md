# zetesis-wgpu

Bounded GPU primitives for grounding and answer-set checking. CPU and GPU
operations share the same reduct obligations; device scheduling does not establish a different
semantics. The [execution chapter](../../docs/book/architecture/execution.md)
explains their place in ordinary solving, and the
[parallel library guide](../../docs/book/rust/parallel.md) describes composition.

## Choose an operation

| Operation | Subject and result | Obligation outside this operation |
| --- | --- | --- |
| `GpuOracle` | Complete `GroundProgram` and candidate seeds; least closures, constraint failures and gate agreement | Complete candidate enumeration and source admission |
| `GpuLazyOracle` | Relational source instances and frozen per-candidate worlds; synchronized closure rounds | Complete source coverage and final acceptance accounting |
| `GpuFormulaOracle` | Original formula DAG and ordered candidate interpretations; rejection, completed refutation or a residual query | Exact completion of every residual before answer-set acceptance |
| `GpuTightOracle` | A checked `TightPlan` and interpretations; original satisfaction and ranked support | Plan applicability, candidate enumeration and complete result accounting |
| `GpuAggregateOracle` | An `AggregateGpuPlan` and Group-bound eligibility occurrences; count, sum, sum-plus, minimum and maximum | Source completeness, head permission and reduct minimality |
| `GpuRelationExecutor` | One immutable typed relation and equality queries; ordered row masks | Complete pattern matching, source coverage and answer-set checking |

The tight, native aggregate and relation operations are explicit library
experiments;
ordinary solver dispatch does not currently select them. A successful primitive
benchmark is not a complete source-language solve.

Construct an oracle with `GpuOptions` and an explicit `GpuSelection` where
reproducibility requires a particular backend. Selection distinguishes physical
adapters from fallback/software devices and retains adapter metadata. Consult
[public API](src/lib.rs) for exact constructors, limits and result types.

## Exact completion and failure

A formula propagation fixed point is not itself an answer-set certificate.
`FormulaVerdict::Residual` requires the exact host completion path. Candidate
ordering, original theory identity and the frozen candidate are preserved across
that boundary. Device results cannot silently change the subject being checked.

The static operation receives a complete ground program; `check_batch` performs
no lazy tuple discovery. Lazy execution instead admits bounded source instances
against immutable world snapshots. Source rounds and their coverage barriers
remain part of correctness, even when instances are shared across candidates.

Adapter, allocation, limit, cancellation, timeout, validation and device failures
remain failures or incomplete work. They are never converted into UNSAT results.
Resident plans and transport buffers retain explicit identity and capacity
contracts. Statistics distinguish submitted work from completely validated
results; payload accounting is not a measurement of process RSS or physical bus
traffic. Read each operation's rustdoc before reusing residency after a failure.

## Native numeric aggregates

Aggregate preparation coalesces complete tuple identities and retains original
and frozen eligibility separately. Empty extrema carry presence explicitly;
no ordinary integer stands for an empty minimum or maximum. The numeric GPU
profile requires representable measured values and guards. Unsupported numeric
plans return a capability failure rather than changing the source semantics.

One 64-lane workgroup reduces each occurrence with strided folds and a shared
addition tree. Signed sums require separately safe complete positive and negative
carriers; cancellation in the final sum cannot justify an overflowing
intermediate. Integer execution uses neither floating point nor optional subgroup
operations. Actual packing, synchronization and readback remain executable
refinement obligations; the Lean arithmetic laws alone do not verify WGSL.

## Immutable relation selection

`GpuRelationExecutor::prepare` uploads the equality-ID columns of a borrowed
`zetesis_core::relation::Relation`. Its prepared view exclusively borrows the
executor and retains the exact relation owner. A filter accepts queries from
that owner and produces one packed row mask per query occurrence. Empty
conjunctions retain every row; missing dictionary values retain none.

The kernel evaluates 64 consecutive row positions per workgroup. Unique writers
pack the flags into ordered mask words; reconstruction preserves local row
identity and the relation's original catalog mapping. Equality IDs provide no
numeric or ASP term ordering. The complete tuple matcher remains responsible for
patterns and binding. Preparation, filtering and reconstruction each have
explicit limits; the enclosing caller accounts for simultaneously retained views.
These operations do not replace ordinary source grounding.

The core `Relation::select_mask` producer returns the same low-bit-first row
layout for CPU consumers, directly from the shared equality predicate. Device
qualification compares both producers and an independent typed-row reference.
Layout and owner checks alone do not establish correct device membership.

## Validate a physical backend

Portable checks exercise planning and host failure boundaries. Physical Metal
qualification is selected explicitly:

```sh
cargo test --locked -p zetesis-wgpu --all-features --test hardware_formula metal -- --ignored --nocapture
cargo test --locked -p zetesis-wgpu --all-features --test hardware_aggregate metal -- --ignored --nocapture
cargo test --locked -p zetesis-wgpu --all-features --lib metal_aggregate -- --ignored --nocapture
cargo test --locked -p zetesis-wgpu --all-features --test hardware_relation -- --ignored --nocapture --test-threads=1 --exact metal_relation_masks_match_typed_rows metal_relation_refusals_preserve_prepared_view
```

The [test sources](tests) contain the separate static, lazy, tight, formula,
aggregate and relation controls. Vulkan tests use their explicit Vulkan filters;
a Metal pass does not qualify Vulkan. The repository's `scripts/check.sh coverage --metal`
checks the selected physical groups with the matching instrumented binaries and
keeps CPU-only CLI coverage separate. See [Contributing](../../CONTRIBUTING.md)
for the complete gate discipline.
