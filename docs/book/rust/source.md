# Preparing programs and interpreting analysis

Use `zetesis_themelios::prepare_formula` for source text or
`prepare_program_formula` for an existing canonical program. Both prepare input
for zetesis's finite formula solver and expose retained analysis before
materialization. The returned `PreparedFormula` owns the original canonical
program, optional source bytes, metadata, checked intermediate representation,
the canonical source authority and retained resource accounting. `ground(self)`
consumes that preparation and produces an `AdmittedFormula`; it computes no answer sets.
`zetesis_solve::PreparedInput::formula` then borrows that owner for a session.

```text
source       → prepare_formula         ┐
Arc<Program> → prepare_program_formula ┴→ PreparedFormula → ground → AdmittedFormula
                                             │                          │
                                        inspect analysis           borrow in a session
```

The preparation can succeed while grounding later refuses arithmetic or a
resource ceiling. Splitting the calls refreshes neither expansion budgets nor
accepted formula-work charges. Grounding resumes the same authority and workspace
accounting. Keeping the returned owner is therefore part of the contract, not
merely a convenience for avoiding another parse. The preparation can move between
threads; an observer is attached only for the materialization call and reports
work performed during that call.

Static values, predicates and constructor names are admitted during preparation.
Patterns, filters and expression instructions retain coordinates into that
vocabulary. Evaluation borrows their typed views; generated values enter the
same authority. Pattern argument positions remain distinct from term identities,
so repeated arguments and variable positions retain their meaning. An empty
support relation may coexist with a nonempty vocabulary. Neither term admission
nor a compiled pattern asserts an atom's truth.

Construct canonical programs through `zetesis`: macros let you write ASP
directly, while typed constructors suit larger applications that compose
programs from data and reusable components. Both produce the same
`zetesis::Program` accepted by the preparation APIs. Construction does not
establish solver support; admission still checks the selected language profile.

## Prepare a logical formula program

`prepare_program_formula(Arc<zetesis::Program>, ProgramAdmissionOptions,
ExpansionLimits, FormulaLimits)` accepts the canonical themelios type exposed
through `zetesis::Program` and `zetesis::program`. The adapter's
`zetesis_themelios::logical` reexport names that same type. It inspects the borrowed input, then enters the
same normalization, scope checking, analysis and preparation used by source
admission. It never renders or reparses the program. The supported formula
language includes constants, pools and intervals, choices and disjunctions,
conditional literals, admitted aggregates, objectives, `#show` and `#project`.
Binding safety, arithmetic definedness and capability limits remain enforced.
Named or parameterized parts and unresolved includes remain outside this door.

The result is the same `PreparedFormula`: use `ground()`, `ground_hybrid()` or
`ground_adaptive()` with their existing capability restrictions and cumulative
budgets. Eager and hybrid owners supply `PreparedInput::formula` and `::hybrid`.
Adaptive materialization returns either `Complete` for `::formula` or `Terminal`
for `::terminal`. Preparation and materialization do not search for
answers. A failure in a later phase still refuses the original input.
`admit_program_formula` composes preparation with eager `ground()` when no
inspection or alternate materialization is needed.

`ProgramAdmissionOptions` bounds canonical structure before copying or rewriting
it. General formula inspection includes directive and condition structure,
term/symbol interiors, provenance entries, documentation and transform-tag text.
The original `Program` stays alive through the supplied `Arc`, alongside the
prepared and analyzed representations. This shares the input allocation; it does
not eliminate the retained input's memory cost or bound earlier construction,
allocator overhead or process RSS.

`original_program()` returns the input before normalization. `ProgramSite`
contains an optional `StatementId` in that owner's canonical statement order
and an optional real `Location`; a constructed statement needs no source span.
Emitted roots, objectives, warnings and delayed work retain these sites. Typed
formula failures retain the original owner and expose `site()`,
`original_program()` and `subject()`. Observation errors expose `site()` for
resolution through the formula owner. IDs belong to that original program and
cannot be transferred between owners. Formula owners' `source()` is optional:
it returns `Some` for single-source input and `None` for this door, even if a
canonical carrier has parsed coordinates. Bundle input uses `bundle()`.
Provenance does not supply the original bytes.

For migration from the source-only APIs in `v0.2.0`, see
[the migration notes](#migrating-from-source-only-apis) below.

Source admission additionally checks original bytes, parser/raiser diagnostics,
authored counts and include identity. Canonical input cannot recover syntax or
occurrences already erased during construction. Shared preparation preserves
these source checks and the canonical program's counted-element semantics.
This common compiler boundary does not establish a parser-to-Lean or Rust
implementation proof; the [correspondence account](../lean/correspondence.md)
states the remaining obligations.

These two examples build the same exact-one choice and print its two complete
answer sets, `{chosen(1)}` and `{chosen(2)}`. With the macro, the program reads
as ASP:

```rust
# extern crate zetesis;
# extern crate zetesis_cpu;
# extern crate zetesis_solve;
# extern crate zetesis_themelios;
{{#include ../examples/formula-program-macro.rs:example}}
# mod execution {
{{#rustdoc_include ../examples/shared/formula-program.rs:hidden}}
# }
```

The constructors expose the choice elements and bounds for composition in Rust:

```rust
# extern crate zetesis;
# extern crate zetesis_cpu;
# extern crate zetesis_solve;
# extern crate zetesis_themelios;
{{#include ../examples/formula-program.rs:example}}
# mod execution {
{{#rustdoc_include ../examples/shared/formula-program.rs:hidden}}
# }
```

Both examples use this shared preparation and enumeration function. It prepares
and grounds the canonical program, then collects the complete interpretations
through a solve session. There is no display directive, so the exported atoms
are the full answer sets.

```rust
# extern crate zetesis;
# extern crate zetesis_cpu;
# extern crate zetesis_solve;
# extern crate zetesis_themelios;
mod execution {
{{#include ../examples/shared/formula-program.rs:example}}
}
```

Run either version with:

```sh
cargo run --locked -p zetesis --example book-formula-program-macro
cargo run --locked -p zetesis --example book-formula-program
```

## Validate input and control preparation

`validate_program_formula(&program, options)` inspects canonical structure and
the outer language profile without expanding intervals, evaluating arithmetic
or grounding rules. It borrows the refused statement or part. Successful
validation does not establish that later grounding will succeed.

`prepare_program_formula_with(program, ProgramFormulaOptions)` selects the
declarations to prepare and an optional shared `Cancellation` token. Its default,
`FormulaPurpose::Ordinary`, retains objectives and `#project`.
`FormulaPurpose::AnswerSets` prepares unscored enumeration of the full answer
sets: objective and projection declarations are neither normalized nor
instantiated. `#show` keeps its display meaning. Structural limits still inspect
the entire input, and every retained statement identity refers to the original
program. The ordinary CLI continues to honor objectives and projection.

The preparation token persists into eager, hybrid and adaptive materialization,
including aggregate and observation compilation. An absolute deadline belongs
to that token; moving between phases does not restart it or refresh resource
allowances. Pass the same token into the subsequent `Session` so its deadline
continues through enumeration. `FormulaFailure::interruption()` distinguishes
cancellation and deadline expiry from malformed input or exhausted resources.
Failure returns no
partial admitted program and retains the original owner for diagnosis.

Polling is cooperative. Upstream analysis and some canonical transformations
have no internal polling hook; their bounded calls are checked before and after
execution. This is not a hard real-time deadline. These operations provide
preparation controls; they do not themselves implement the themelios `Backend`
run protocol.

## Admit an existing logical program

For an owned relational input with display metadata, use
`prepare_program_relational(Arc<Program>, ProgramRelationalOptions)` and lend
its result through `PreparedInput::relational`. The same admitted program can
run with `Grounder::Lazy` or `Grounder::Eager`. Preparation expands finite facts
and compiles templates; lazy execution still performs variable joins during
candidate checking. This reuses the extended relational compiler, including
scalar constants, finite fact expansion and signed coherence.

The owner retains the original program, template statement identities and
compiled `#show` queries together. `FormulaPurpose::AnswerSets` has the same
meaning here as in formula preparation: objectives and `#project` are excluded
before evaluation, while structural input limits still apply. Preparation shares
one expansion allowance and optional cancellation token across normalization and
observation preparation. Pass that token to the session to preserve the same
deadline through solving.

This relational profile remains narrower than the formula profile. General
disjunction, bounded choices and aggregates are explicit refusals; preparation
does not silently switch them to eager or hybrid execution. Existing Lean closure
laws apply to the admitted normal program, but do not yet prove this Rust input
conversion.

These examples construct one optional selection with conditional display terms.
The macro writes the choice and `#show` directives directly:

```rust
# extern crate zetesis;
# extern crate zetesis_cpu;
# extern crate zetesis_solve;
# extern crate zetesis_themelios;
{{#include ../examples/relational-program-macro.rs:example}}
# mod execution {
{{#rustdoc_include ../examples/shared/relational-program.rs:hidden}}
# }
```

The corresponding constructors keep each statement available for composition:

```rust
# extern crate zetesis;
# extern crate zetesis_cpu;
# extern crate zetesis_solve;
# extern crate zetesis_themelios;
{{#include ../examples/relational-program.rs:example}}
# mod execution {
{{#rustdoc_include ../examples/shared/relational-program.rs:hidden}}
# }
```

Both use the following native preparation and enumeration function. The same
cancellation token covers preparation, lazy solving and observation. It returns
full atoms separately from the conditional `#show` terms and checks both
exhaustion and the actual closure route. The complete results are an empty
answer with no shown terms, and `{chosen(1)}` with the shown term `1`.

```rust
# extern crate zetesis;
# extern crate zetesis_cpu;
# extern crate zetesis_solve;
# extern crate zetesis_themelios;
mod execution {
{{#include ../examples/shared/relational-program.rs:example}}
}
```

Run either version with:

```sh
cargo run --locked -p zetesis --example book-relational-program-macro
cargo run --locked -p zetesis --example book-relational-program
```

### Borrow the strict relational input

`zetesis_themelios::admit_program(&program, ProgramAdmissionOptions)` borrows a
canonical `zetesis::Program` and compiles the strict relational (S0) profile.
The facade and the adapter's `logical` reexport expose the same upstream
themelios type, so callers can construct or raise the program with its native APIs. Admission
uses the same rule compiler and strong-negation coherence operation as `admit`,
without rendering or parsing the program.

This door accepts parameterless base-part rules, relational variables and closed
data values, default negation, equality/disequality tests and unbounded
single-atom choices. Equality does not introduce a variable absent from positive
relational bindings. Unary negation retains the shared compiler's checked
closed-operand evaluation; general arithmetic terms, generators, aggregates and
directives remain outside this door; use
[`prepare_program_formula`](#prepare-a-logical-formula-program) for the general
formula profile.

`ProgramAdmissionOptions` bounds cumulative logical nodes and UTF-8 name/string
bytes, nesting depth including symbol interiors, and canonical body elements
per rule. Its `core_limits` separately governs native template admission.
The borrowed traversal checks each rule before compilation, uses an iterative
cursor over term and symbol interiors, and returns a typed `ProgramLimit` on
refusal. S0 admission borrows statement evidence and does not traverse or count
its provenance. These ceilings govern admission of an existing value; they do not
account for its earlier construction or represent a process-memory limit.
Source admission retains its checks on original bytes and authored syntax,
including restrictions erased by canonicalization and body occurrences before
set collection. Those checks cannot be recovered from a logical program.

The returned `AdmittedProgram::program()` supplies the native program for
`PreparedInput::program`. `template_statements()` borrows the original statement
carriers in native template order; generated coherence constraints retain both
introducing statements. `ProgramAdmissionFailure` similarly borrows the actual
statement or part associated with a refusal and invents no source coordinate.
`into_program()` releases this evidence borrow and returns the independently
owned native program. Admission itself performs no grounding or answer search.

These examples construct `p :- not q.` and `q :- not p.` and print their two
canonical answer sets, `{p}` and `{q}`. The macro keeps the rules in ASP form:

```rust
# extern crate zetesis;
# extern crate zetesis_cpu;
# extern crate zetesis_solve;
# extern crate zetesis_themelios;
{{#include ../examples/program-macro.rs:example}}
# mod execution {
{{#rustdoc_include ../examples/shared/program.rs:hidden}}
# }
```

The constructors compose the same rules from atoms and negated bodies:

```rust
# extern crate zetesis;
# extern crate zetesis_cpu;
# extern crate zetesis_solve;
# extern crate zetesis_themelios;
{{#include ../examples/program.rs:example}}
# mod execution {
{{#rustdoc_include ../examples/shared/program.rs:hidden}}
# }
```

Both use this shared admission and enumeration function. The session borrows
the admitted native owner without source metadata. `symbols::atom_with` exports
each full interpretation using the session's cancellation token and a 4096-byte
ceiling per atom. The helper returns the family only after the session reports
exhausted search; an unfinished outcome is an error.

```rust
# extern crate zetesis;
# extern crate zetesis_cpu;
# extern crate zetesis_solve;
# extern crate zetesis_themelios;
mod execution {
{{#include ../examples/shared/program.rs:example}}
}
```

Run either version with:

```sh
cargo run --locked -p zetesis --example book-program-macro
cargo run --locked -p zetesis --example book-program
```

## Defer terminal positive definitions

`PreparedFormula::ground_adaptive()` returns
`FormulaMaterialization::Complete(AdmittedFormula)` or
`FormulaMaterialization::Terminal(TerminalFormula)`. Bundle preparation has the
same consuming operation. Use `PreparedInput::formula` for the first result and
`PreparedInput::terminal` for the second. The terminal session accepts automatic
grounding and reconstructs full answers before applying original observations.

`PreparedFormula::ground_lazy()` (and its bundle counterpart) defers the same
certified definitions over a hybrid base: the base's producer core is
instantiated and its eligible integrity constraints are streamed, even when none
is eligible. It returns `FormulaMaterialization::Complete(HybridFormula)` when
nothing is deferred. A terminal owner's `base()` says which base it has; a
hybrid base's session checks each core answer against the streamed constraints
before reconstructing it, runs under automatic or lazy grounding on the CPU, and
refuses an eager request. `zetesis_solve::ground_formula` and `ground_bundle`
map a requested grounder to its materialization — `eager` to `ground`, `lazy` to
`ground_lazy`, `auto` to `ground_adaptive` — as the CLI and the facade do.

A terminal definition has a positive, flat body and an ordinary head whose
predicate is read by no logical rule or constraint. All producers must qualify;
strong-negation coherence, objectives and explicit projection are checked before
partitioning. This is a general source classification, independent of a program's
name or constants. See [the grounding architecture](../architecture/grounding.md#terminal-definition-analysis)
for its exact semantic boundary.

Adaptive materialization defers a certified group only when at least one of its
complete lowered producers has a variable, including a body-only variable. A
group shares a predicate name, arity and sign; all its producers move together,
including any ground facts or rules. Groups whose producers are all ground stay
in ordinary materialization, even when their rules have nonempty bodies. If no
group passes this physical policy, the result is `Complete`. Mathematical
terminal eligibility remains unchanged.

`TerminalFormula::base_theory()` and `base_analysis()` describe the filtered base.
`analyzed_program()`, `source_analysis()` and `metadata()` retain the original
source contract. A base answer alone is not an original answer. For direct
composition, `reconstruction()` returns an independent cursor whose
`reconstruct(&base_model, &cancellation)` extends the supplied interpretation.
That lower-level operation checks exact catalog ownership, not base stability;
the ordinary session supplies the verified-base premise.

The cursor borrows canonical terms and uses private binding and row-selection
metadata. Each call starts with exactly the supplied true rows and may charge
the cursor's per-answer allowance of work and substitutions — the headroom the
formula ceilings left after source admission — whatever earlier calls used; a
work or substitution refusal reports that allowance as its limit and the call's
own charge as observed. `statistics()` reports the session totals, admission's
charge, the allowance, the latest call and the largest call. A refusal
fuses the cursor without invalidating prior returned models. Retained model
families have their own consumer-side memory limits. These named capacities are
not process RSS, and the checked mathematical extension law is not yet a proof
of the Rust source classifier or reconstruction implementation.

This runnable example handles both materialization results and collects a
complete `WorldView` through the ordinary session API. Its four interpretations
include precisely the receipts entailed by their chosen seeds.

```rust
# extern crate zetesis_core;
# extern crate zetesis_cpu;
# extern crate zetesis_solve;
# extern crate zetesis_themelios;
{{#include ../examples/terminal.rs:example}}
```

Run it with:

```sh
cargo run --locked -p zetesis-solve --no-default-features --example book-terminal
```

## Stream ordinary constraints

`PreparedFormula::ground_hybrid()` and the corresponding bundle method return
one shared `HybridFormula`. It retains the original source, the typed atom
catalog and, when any constraint is streamed, the closed canonical base with the
possible-support relations those constraints read; discovery and order indexes
and every other relation are released at admission. Closing is charged as
formula work, and its transient peak is admitted against `max_support_bytes`. Producers and constraints with aggregates,
projected atoms or conditional scopes remain in `core_theory()`. Ordinary
atom/scalar integrity constraints retain their prepared templates instead of
complete formula DAGs. This first schedule requires indexed joins and refuses
objective programs explicitly. The existing `ground()` methods remain eager.

Use `PreparedInput::hybrid(&owner)` for solving. A core answer is only a proposal:
the session checks the retained constraints before returning an answer of the
original program. A core certificate applies to the core. It does not establish
membership in the original program on its own.

For direct composition, `owner.checker(ConstraintCheckLimits { .. })` creates a
mutable checker borrowing the owner. `check(&model, &cancellation)` requires the
model's exact atom-catalog owner. It returns `Satisfied` after the required scan
completes, or `Violated { site }` when an admitted constraint body is true.
Neither result establishes reduct minimality. Wrong-owner, cancellation,
deadline, allocation and resource failures are typed separately from both
verdicts; failures preserve cumulative statistics. Equal source bytes do not
substitute for owner identity.

`check_region(&theory, &region, &cancellation)` requires the exact retained core
and a region spanning its dense atom catalog. A certainly true constraint body
returns `Refuted { site }`; otherwise it returns `NotRefuted`, which does
not assert satisfaction. The checker prepares and reuses a typed atom index and
support-row correspondence on first region use. Region checks select known-held
positive rows before binding, while retaining rows without a known correspondence.
A necessary signed-predicate test can avoid a template that cannot have a sure
body. Neither operation changes the final full-model check or arithmetic admission.
The method checks the coordinate convention, not the origin
of an arbitrary caller-created region. Use it only to restrict original
candidates, never to evaluate their frozen reducts.

Support dictionaries and equality indexes are reused. Ordinary joins lend their
current binding; generators retain their existing owned-row requirements.
Prepared constraint plans borrow the immutable source components. Each checker
owns its traversal and query workspace, so retaining a plan retains no candidate
state or mutable workspace from another checker.
Candidate truth uses borrowed typed atom keys, with no temporary formula DAG or
copied atom per instance. Original source-family arithmetic validation completes
before the hybrid owner is returned, independently of later candidate truth.
Omitted zero-divisor instances therefore retain the same located warnings, and
candidate filtering cannot conceal fatal arithmetic.

Hybrid admission preserves its original expansion-budget prefix. It also keeps
prepared constraint plans and the support relations they read, which eager
grounding can release after emission. The existing source, scalar and support ceilings still apply;
they are not a single aggregate live-memory or RSS bound. Core atoms, nodes and
roots retain the formula-theory ceilings, including coherence and unsupported
atom guards. `streamed_templates()` counts lowered templates, including pool
alternatives, and `streamed_instances()` counts scalar-selected instances visited
during admission; neither is a retained instance store.

`checker(limits)` has its own cumulative work and substitution limits, plus a
byte allowance for structural capture deltas. ID-only copies and lookups do not
consume that byte allowance.
For parallel composition, create `ConstraintAllowance::new(limits)` and
use `checker_with_allowance(&allowance, &cancellation)` for every worker and final
checker. Its clones share before-operation charges; no allowance is multiplied
by the worker count. `allowance.statistics()` is exact after those workers join;
live fields are independently observed monotone counters. Snapshot descriptors
are prepared once per checker, without copying support rows; per-operation
binding/storage ceilings remain those of the admitted source. A new checker
starts its own local receipt; a supplied shared allowance continues across
checkers. Neither repeats source admission. These statistics describe charged
work and payload, not process memory. Streaming can reduce retained constraint storage while
repeating joins; it does not remove the complete
finite atom envelope or imply a time improvement.

Shared checking also meters bound-key relation queries before their dictionary
steps. `Relation::query_attempt_with` supplies this composition boundary: the
relation enforces its own limits, then asks the enclosing operation for each
work permit. A refusal retains the accepted prefix and runs no later step. It
uses the same equality resolver as locally metered `query_attempt`.

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
before preparation. The default is 128 MiB for the support authority's canonical
payload, identity indexes and current prefix, plus relation metadata, postings,
borrowed snapshots, query capacity and construction scratch. Canonical support
payload is counted by its authority once. Allocator/tree/control-runtime overhead
and unrelated grounding state remain separate; this is not a total-memory bound. The CLI exposes the same
allowance as `--max-support-bytes` in `--help-all`. This is an admission limit;
`SolveConfig` applies after the formula owner has already been constructed.

Choose positive joins separately through the preparation's
`with_grounding_options(GroundingOptions { joins: JoinStrategy::Table })` method.
The default `Indexed` strategy probes existing value postings. After a flat
constraint establishes finite arithmetic totality, a computed equality may
supply a necessary input domain to the shared table selector under either
strategy. Unrestricted probes still follow the requested strategy. `Table`
reuses support masks for all applicable flat positive patterns over completed
eager support; structural patterns and support-growth rounds keep indexed joins. The bundle
preparation exposes the same method, and the CLI maps `--formula-joins table`
to it. The choice retains the preparation's source identity and remaining budgets.

The private query workspace owns cached indices and accounts for simultaneous
selected masks, scratch and cumulative work. These capacities compose with the
same support authority's payload charge. Allocator/tree/control-runtime overhead
and other grounding state remain separate; the limit is not RSS. An exhausted table operation returns
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
rule atoms, generators, negative rule-body literals, structured terms,
anonymous/local scopes in rules and richer heads. `NormalizedProgram` is necessary; a dependency projection never supplies
narrowing. Inapplicability keeps the existing complete path and does not create
a new source refusal.

Objectives may accompany eligible rules without disabling their domain guards.
The analysis still receives the complete normalized Program, and objective
declarations introduce no producer domains. Objective joins and scoring remain
unchanged; this does not permit richer rule heads or local scopes in rules.
Objective-local conditions keep their existing scopes and execution.

Finite argument domains constrain each rule's variable occurrences, in every
possible-support completion round and in final instantiation. Their intersection, less every value a
comparison over that variable alone is defined and false at, can reject a row
that has no complete positive continuation, before copying its new bindings or
opening deeper probes; the candidates are prepared once per rule with the
analysis, and a guard is prepared only where they are fewer than the
argument's domain. The surviving rows keep the existing matcher, original
positions and source origins. Objective and factorized component joins retain
their existing paths. Global Unknown or Stopped analysis supplies
no guards. An individually Unknown argument is unrestricted; other finite
arguments may still contribute restrictions.

Preparation admits the selected domain values once into the source vocabulary.
Each rule retains scoped term IDs and a range for each variable; an absent
range is unrestricted, while an empty range is a known empty domain. Unary
comparisons reuse those IDs. Each completion snapshot resolves the surviving
values into its own relation dictionaries, whose local IDs are not source term
IDs. This avoids rebuilding typed values during guard preparation. Candidate
metadata remains bounded by charged work outside the support-byte allowance;
canonical payload and guard workspace retain their separate storage accounting.

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
its named headers, scratch, dictionary IDs and actual vector capacity; candidate
terms remain borrowed from their canonical owner. These capacities are counted
beside support, table indices and live masks. These contracts give neither a
hard allocator/RSS limit nor cancellation/deadline polling. Guard preparation
and membership costs may outweigh avoided probes. See
[finite-domain ownership and proof boundaries](finite-tables.md#optional-argument-domain-guards)
for the exact named-capacity scope and preservation premises.

## Partial arithmetic families

Only an evaluated numeric division or remainder by zero can omit a source
instance. A nonempty original rule family needs at least one complete
substitution whose reached arithmetic is jointly defined; a defined comparison
that is false still supplies that witness. Thus `d(0;2). p(X):-d(X),1/X=1.`
keeps just the facts and reports a warning, whereas the same rule over `d(0)`
is refused. Empty positive joins are valid and silent. An ordinary comparison
over relationally bound variables, such as `X!=0`, explicitly excludes that
substitution and produces no warning for its zero divisor.

Overflow, arithmetic on a nonnumeric value and an invalid exponent remain
errors when reached. After a zero divisor, source classification continues
through independent expression branches and independent components; a later
fatal error takes precedence. An operation depending on an undefined value is
not evaluated. This source-family policy uses the same checked scalar
operations as strict evaluation; it does not change the observation evaluator's
first-failure policy. Authored closed undefined expressions are still refused
during preparation. The existing evaluation phases remain ordered: a rule body
or local condition selects an instance before head or consequent generation.
An omitted or false body does not invoke that later phase. Independent-fatal
validation applies to the branches, components and generators of the reached
phase.

The decisive traversal reads completed support, before formula emission.
It does not use comparison-pruned candidate domains, support delta partitions
or already-produced-head shortcuts as family evidence. Normalized fragments of
one original rule combine their evidence. Each nested choice, aggregate or
conditional local family is judged separately for its fixed outer binding, so
a valid family at another outer binding cannot rescue it. Alternatives of one
authored local element share evidence; distinct authored elements do not. An
undefined conditional consequent omits its whole local implication. Defined
false consequents and empty nonarithmetic witness sets retain their logical
meaning, including under default negation. The additional complete
traversal spends the existing grounding work and substitution ceilings;
exhaustion refuses admission rather than accepting partial evidence.

Successful formula and bundle owners retain typed `FormulaWarning` values and
provide `warnings()` and `warning_view()`. Zero-divisor warnings are deduplicated
by `ProgramSite` and ordered by optional source coordinate, then statement ID;
they do not count attempted rows. `site()` retains the logical subject, while
`location()` and `diagnostic()` return `None` without real source coordinates.
`FormulaLimits::max_warnings` defaults to 10,000, and a
new distinct warning beyond that bound produces `FormulaResource::Warnings`.
The CLI renders retained warnings once after admission. The
[numeric boundary](../reference/language.md#numeric-boundaries-and-refusal-meaning)
records the deliberate differences from clingo 5.8.2 and their rationale.

## Constraints over keyed values

A choice rule of the form `1 { p(K, V) : c(V) } 1 :- b(K).` holds, in every
answer set, exactly one atom `p(k, v)` for every `k` that `b` admits and no
other atom of `p`, when nothing else produces `p`. `zetesis_domain::keys`
reads these *keyed relations* from the normalized program: the key positions
are the body's variables, the value position the one variable the element's
condition binds. The analysis is syntactic and conservative; another producer,
other bounds, a second element, a value bound outside its condition or a
negated literal anywhere yields no key.

Preparation then asks a constraint over a keyed value as the one atom its key
admits. `:- G, p(k, Y), Y != t.`, with `Y` read nowhere else, `t` free of
`Y` and every key position of `k` naming its value, is prepared as `:- G, b(k), not p(k, t).`: whenever `b(k)` holds, exactly
one `p(k, y)` holds, and the written constraint fires exactly when that `y` is
not `t`, which is exactly when `p(k, t)` is absent. A column with a digit and a
carry, `:- G, p(k, Y), q(j, C), s != Y + 10*C.`, is prepared as two such
constraints demanding `s \ 10` of `p` and `s / 10` of `q`, when the facts of
the conditions binding the digit and the carry admit only digits in `0..9`
and only natural numbers, facts being all that produces them
(`zetesis_domain::FactIndex`, which reads a program's facts once for every
such question); the equation then has one solution, and for a
negative `s` no solution, in which case both forms fire. The product of the
demanded value with every value the key admits is never formed; the
[observation record](https://github.com/GregoryGelfond/zetesis/blob/main/docs/book/reference/observations/README.md#keyed-constraints-the-one-atom-the-key-admits)
measures the send-money puzzle's columns at one hundred instances each in
place of eighteen hundred forbidden combinations.

The rewrite additionally requires checked-evaluation evidence for the whole
written constraint. Bounds from fact-only positive predicates, or fact-only
conditions binding a keyed value, must establish the numeric type of each
arithmetic operand and an `i32` result at **every intermediate operation**.
The dividend `s` must also be numeric. An unknown type or bound leaves the
constraint as written. The proof is conservative: it uses intervals, does not
infer bounds from comparisons, and does not prove variable-exponent powers.
Arithmetic-free constraints need no numeric bound.

Checking the whole constraint preserves its source evaluation obligations.
For example, replacing `Y != 1` by `not p(k, 1)` must not expose `1/X` in a
substitution the written comparison excludes. Likewise, `Y + c*C` cannot be
removed when its multiplication or addition may overflow, even when the
equation is valid over mathematical integers. The original source still
governs errors, warnings and source locations when a rewrite is declined.
The [numeric boundary](../reference/language.md#numeric-boundaries-and-refusal-meaning)
states the deliberate differences from clingo 5.8.2; this optimization does
not relax that boundary or infer safety from the reference solver's behavior.

The written constraint is compiled once and its rules replaced in place by
the asked constraints', so nothing is prepared twice. The key analysis and
its readings of facts run under `FormulaLimits::max_key_work` and the term
work remaining, and their steps are charged to the term work;
`AdmittedFormula::key_analysis` says whether it completed or stopped, and a
stop leaves every constraint not yet asked as written, which changes no
answer set.

The replacement constraints are compiled under the remaining expansion budget.
Every asked statement retains the original constraint's statement identity and
any real source coordinates. A parallel owner map preserves that association
through canonical analysis; ambiguous expansions or content merges leave the
constraint unchanged. Transformation provenance remains evidence, while
`formula_origins()` names the original constraint. `keyed_constraints()` on the admitted formula
counts the constraints asked; the answer sets are the same either way, which
the contract tests state against the hand-asked program and against clingo.
A constraint outside the two patterns is left as written, and so is one
with an anonymous variable in a key position: the asked atom stands under
`not`, where `p(_, t)` holds when some key has the value `t`, so
`not p(_, t)` would forbid only that no key has it, while the written
constraint forbids a wrong value at every key.

## Inspect terminal positive definitions

`zetesis_domain::terminal::analyze(&program, limits)` identifies predicates whose
producers are all flat positive normal rules and whose atoms have no semantic
consumer elsewhere in the supplied program. It returns the original rule
carriers, borrowed from that exact program. All producers must qualify;
recursion, feedback and complementary strong-sign occurrences prevent selection.
Predicate spelling, source filenames and `#show` do not determine eligibility.

The result distinguishes a complete scan, unsupported context and a resource
stop. Only a complete scan publishes a selection, which may be empty. Limits
bound logical work and metadata populations; they do not promise fallible
allocation or cancellation. See the
[crate contract](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-domain/README.md#terminal-positive-definitions)
for the admitted profile.

The classifier alone does not change grounding. The adaptive formula API
[described above](#defer-terminal-positive-definitions) consumes its result,
establishes source-to-execution correspondence and reconstructs the full
interpretation before answer-set evidence or observation. The
[`TerminalDefinitions` laws](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/TerminalDefinitions.lean)
prove the propositional extension theorem; they do not certify those execution
steps.

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
counted occurrence family while exposing a dependency projection for analysis.
Pool expansion creates a separate Boolean occurrence for each alternative;
grounding witnesses of one expanded occurrence share its key. The explicit
tuple aggregate does not require that projection by itself.

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
`ProgramSite` values. A site identifies an original canonical statement and may
also carry a real source location. Neither identity nor location is a proof of
the root's meaning. Some generated roots have different evidence needs from written
rules, so consumers must not assume that one root always corresponds to one
written statement. The admitted owner also retains objectives, observations and
the completed projection domain separately from theory roots. Authored
`project_selection()` metadata is a declaration plan; the admitted owner's
`projection()` supplies the fixed typed domain for
[projected enumeration](sessions.md#projected-enumeration).

The example checks that emitted root evidence retains the source identity
supplied in `AdmissionOptions`. It leaves syntax traversal to themelios's API
documentation. For counted entry identity, pool expansion, tuple activity and
their proof obligations, see [source identity in grounding](../architecture/grounding.md#preserving-source-identity).

The public implementation declarations are
[`PreparedFormula`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula/preparation.rs),
[`AnalysisBasis` and `AdmittedFormula`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula.rs).

## Work and retained space

Preparation retains the original canonical program as well as bounded prepared
and analysis structure. Source syntax limits apply before raising; typed formula
input has independent logical/provenance inspection limits. Canonical counted
choice entries need no separate source-location registry or selected-rule copy.
Source metadata is collected from the original raised occurrence stream before
equal statements merge; canonical input supplies its retained declarations and
provenance. Subsequent pool expansion and formula construction consume
cumulative expansion resources. Raising and metadata admission can fail before
that work begins; preparation does not promise streaming or zero-copy admission.
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
therefore promises neither cheap grounding nor a smaller retained representation;
the hybrid schedule still completes this support before streaming constraints.

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

## Migrating from source-only APIs

Canonical programs need not have source text. The APIs below therefore distinguish
logical statement identity from optional parsed evidence. These changes require
updates to Rust consumers of `v0.2.0`; the source-only relational owners keep their
existing source accessors.

`PreparedFormula::source()` and `AdmittedFormula::source()` now return
`Option<&Source>`. Previously, a consumer could write:

```text
println!("{:?}", input.source().id());
```

Handle the absent source when accepting both input doors:

```rust,no_run
# extern crate zetesis_themelios;
# fn inspect(input: &zetesis_themelios::PreparedFormula) {
if let Some(source) = input.source() {
    println!("{:?}", source.id());
}
# }
```

Use `original_program()` when the operation needs the logical input rather than
its text. Parsed coordinates alone do not provide the original source bytes.

| Previous API | Current API |
| --- | --- |
| Formula/objective origins contain `Location` | They contain `ProgramSite`; use `statement_id()` and optional `location()` |
| Admission, expansion, formula and metadata failure fields contain `Location` | Their statement fields contain `ProgramSite`, including constant-definition sites |
| `GroundingObserver` phase callbacks take `Option<Location>` | They take `Option<ProgramSite>` |
| Hybrid constraint verdicts expose `location` | They expose `site: ProgramSite` |
| `FormulaWarning::location()` is mandatory; warnings implement `ToDiagnostic` | `location()` is optional; use `site()` or `diagnostic() -> Option<Diagnostic>` |

Resolve each `StatementId` through the original program retained by its owner.
Do not transfer IDs between programs or invent locations for constructed input.

Update exhaustive failure matches: `FormulaFailure` adds `Program` and `Logical`
and removes `ChoiceSource`; `ExpansionFailure` adds `Interrupted`. A typed-input
formula failure can wrap its cause while retaining the original program. Use
`cause()` for the underlying failure and `subject()` for its original statement,
part or program. Resource refusals remain distinct from cancellation and deadlines.

The public themelios types also use a newer shared dependency revision. Prefer
`zetesis`'s reexports, or align a direct themelios dependency with the workspace
pin. Values from separate copies of a dependency are distinct Rust types even
when their names match. The [library reference](libraries.md) identifies the
canonical reexports.

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
