# Source preparation and grounding modules

`zetesis-themelios` has two materialization boundaries. Relational admission
returns templates for lazy or eager normal-rule execution. Formula preparation
returns scoped instructions; grounding consumes them to construct a finite
Ferraris theory. Neither boundary searches for answer sets.

This chapter maps the implementation for readers extending the source language.
The [source API guide](../rust/source.md) describes the public entry points;
[grounding](grounding.md) explains their mathematical contracts.

## Follow the owned objects

```text
original source catalog
  → parsed and raised themelios program
  → bounded normalization and scope analysis
  → prepared rules, objective/projection declarations and dependency plans
  → completed possible-positive support
  → complete bindings and original formula instances
  → validated theory + atom catalog + objectives + fixed projection domain + metadata
```

The arrows describe dependencies, not independent copies of every intermediate
object. Source identities and parsed origins remain available after preparation.
Both the source-expansion budget and accepted formula-work charges continue
across preparation and grounding. Materialization resumes the retained accounting
under its configured ceilings.
An analysis projection records its own basis; its classifications do not prove
properties of source constructs that the projection erased.

Possible support uses one canonical source authority. A completed snapshot lends
immutable relations and column indexes to joins; it does not grant those atoms
truth in an answer set. Final grounding publishes a distinct dense occurrence map
over the same canonical prefix. The theory's universe and possible-support bound
remain different populations without separately interning their payload.
Objective queries borrow completed support and own bounded transient formula
scratch. Their atoms and roots cannot become program producers.
Projection declarations also borrow that support. They retain a fixed subset of
possible atoms for enumeration identity, without creating source producers or
program roots. This domain is distinct from the emitted theory's dense IDs.

`GroundingOptions` selects the positive-join execution strategy before formula
materialization. The default `Indexed` strategy probes the shortest applicable
value posting. Explicit `Table` selection prepares reusable value-support masks
for flat positive patterns over completed eager support. Growth-round snapshots
and structural patterns keep indexed probes. This changes the source of row
positions, while the existing whole-row matcher, binding schedule and original
formula emission remain shared.

The completed query workspace borrows the authoritative relations and owns its
index cache. Each join depth owns a selected row mask that borrows the relation,
so later cache insertion does not invalidate an earlier selection. Indices are
keyed by signed predicate and canonical alias scope; domains are rebuilt from
current constants and bound values. Required body validation receives the same
positive witnesses. Table membership never replaces a source atom with truth.

The workspace charges cumulative preparation/query work and simultaneous named
snapshot, index, scratch and mask capacities to the existing source limits.
Refused capacity, allocation and work remain located grounding failures; a
failed table operation is not an indexed fallback or an empty program. These
objects are not a process-memory ceiling. The source builder remains sequential:
an immutable index can serve independent callers, but selecting this strategy
launches neither Rayon grounding nor a GPU kernel. See the
[table API and its preservation boundary](../rust/finite-tables.md).

## Locate the operation

The names below are modules under
[`crates/zetesis-themelios/src`](https://github.com/GregoryGelfond/zetesis/tree/main/crates/zetesis-themelios/src).
An `_ir` suffix denotes source lowering or scope checks; a `_plan` suffix
denotes a checked schedule over existing instructions. Execution consumers retain
those instructions rather than interpreting the plan as new program semantics.

| Responsibility | Modules | Boundary |
| --- | --- | --- |
| Source catalog and includes | `bundle`, `bundle_admission` | Original files, paths, source identities and located failures |
| Diagnostic and strict-profile checks | `profile`, `diagnostic`, `source_diagnostics` | Typed refusals and source views; no recovered partial program |
| Relational templates | `compile`, `extended`, `expansion`, `fact_expansion` | Closed values, bounded constants/arithmetic/fact expansion and template admission |
| Classical negation | `coherence` | Distinct signed predicates and consistency constraints |
| Semantic value conversion | `structural_value`, `scalar_arithmetic`, `formula_value` | Typed closed values and checked arithmetic; no atom support |
| Formula preparation | `formula`, `formula::preparation`, `formula_ir` | Public owned preparation, scoped rule/objective IR and cumulative budgets |
| Source occurrences and alternatives | `formula_choice_source`, `formula_pool` | Original Boolean choice occurrences and bounded pool alternatives |
| Upstream analysis | `formula_analysis`, `formula_conditional_projection` | Bounded pool-free input and an explicit analysis basis |
| Value and pattern lowering | `formula_value_ir`, `formula_range_ir`, `formula_pattern_ir`, `formula_projection_ir` | Shared bounded value/range fold, structural captures and anonymous negative projection |
| Scalar bindings | `formula_binding_ir`, `formula_binding_guard`, `formula_binding_plan` | Safe producers, finite envelopes and dependency order; original guards remain |
| Aggregate bindings | `formula_aggregate_ir`, `formula_assignment_ir`, `formula_assignment_plan` | Element scopes, assignment applicability and required/produced inputs |
| Binding execution | `formula_binding_cursor`, `formula_assignment` | Bounded scalar/range and aggregate proposals over complete relational rows |
| Head lowering | `formula_head_ir`, `formula_choice_ir`, `formula_head_aggregate` | Signed head activity, independent permission and aggregate measure |
| Conditional lowering | `formula_conditional_ir`, `formula_consequent_ir`, `formula_conditional_head_ir` | Outer/local scopes and the admitted conditional-head profile |
| Complete joins and possible support | `formula_support`, `formula_pattern` | Transactional whole-tuple matching, snapshots, indexes and support completion |
| Positive-row query strategy | `grounding_options`, `formula_support::queries`, CPU `table` | Explicit completed-support table indices and owned row masks; shared matcher and cumulative source budgets |
| Formula emission | `formula_ground`, `formula_guard`, `formula_conditional` | Original implications, aggregate conditions and universal instances |
| Optional formula factoring | `formula_factor` | Qualified existential components with the complete-join path as fallback |
| Optional count certificates | `formula_count_plan` | Capture, derivation and emitted correspondence to the original theory |
| Source activity | `formula_source_activity` | Bounded absent/optional/required producer refinement over completed support, shared by objectives and projection |
| Original-model queries | `formula_source_activity::model_query` | Closed conditions over full original atom identities, separate from source activity and reduct formulas |
| Objective source preparation | `formula_weak`, `formula_ir::objective_scope`, `formula_objective_dependencies` | Scoped weak bodies and independently applicable objective precision plans |
| Objective materialization | `formula_ground::objectives`, `formula_ground::scoped_body` | Retained objective templates and bounded body validation without producer support |
| Projection declarations | `formula_project_ir`, `formula_ground::projection`, `metadata::projection` | Fixed complete projection domain; no change to original theory or displayed atoms |
| Observation and metadata | `metadata`, `observation` | Source directives and views of a supplied full model |
| Objective bounds | `objective_bound` | Optional score bounds with their own admission and work limits |
| Measurement | `grounding_observer` | Injected phase/work observations, separate from semantic completion |
| Public composition | `lib` | Exports, strict admission and retained source evidence |

Nested modules follow their owning module's directory: for example,
`formula_ir::objective_scope` is stored in `formula_ir/objective_scope.rs`.
The owning module declaration determines Rust visibility. Filename proximity does
not grant access or establish dependency direction.

## Keep the obligations separate

A binding plan proves readiness of inputs, not the truth of an aggregate or a
negative literal. A possible-support row proves that grounding must consider an
atom, not that the atom belongs to a candidate. An objective eligibility carrier
retains possible contribution keys; its original-model query still decides
whether a contribution is active. Formula emission and final theory validation
preserve these distinctions before any reduct check begins.

Required body evaluation and contribution selection are separate operations.
Pruning a false comparison must not hide required arithmetic on a complete
positive binding. Computations introduced solely for a head have their own
scope: a discarded body row must not acquire a head value or emit an atom.
The existing expression evaluator and binding schedule serve these scopes; their
placement determines when evaluation is required.

`formula_ground::scoped_body` shares the original body-lowering operation between
objective query construction, source activity and validation of discarded rule rows. The caller
selects the applicable scratch ceilings and retains cumulative source work.
Discarding the resulting scratch does not discard errors or charged work.

The source-activity fold computes absent, optional or required truth on the
validated body. The unresolved dependency region refines one complete previous
table in simultaneous rounds; this is not a separate evaluator per strongly
connected component. A changing round resolves an optional entry; it never
reclassifies an already known entry. The finite optional-entry count bounds the
rounds. This abstraction does not decide correlations among optional atoms, and
it does not replace original-model query evaluation or reduct satisfaction.

Both scratch and retained formula construction begin with the same canonical
constants: `FALSUM` represents bottom, and `VERUM` represents bottom implying
bottom. `Builder::initialize` admits these nodes through the ordinary checked
node operation. Boolean simplification uses these named identities. If either
admission fails, the scratch caller restores its cumulative counter owner before
returning the error.

Compiler-internal assignment tests live under `formula_ir` because they construct
both valid and invalid private IR. Public source tests cannot reach states that
scope checking rejects first. These tests complement source admission and
answer-set tests; they do not replace them or justify making private constructors
public.

When adding a construct, identify its source scope, readiness rule, complete-row
validation, possible-support contribution and original formula separately.
Then identify its source locations, bounded scratch and retained output. A change
in one stage must not silently transfer an obligation to the next.
