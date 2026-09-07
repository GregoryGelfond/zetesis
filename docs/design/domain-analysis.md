# Domain analysis over themelios programs

The typed themelios `Program` is the input to semantic planning, before grounding
or solving. It retains program parts, statements, shared terms and provenance.
It is not just an intermediate used to parse text. Analysis results describe an
identified program and its context; a ground carrier or a selected execution
plan is a later, distinct representation.

`zetesis-domain` is a separate crate for this analysis. Its production boundary
uses pinned themelios program/base types and the standard library. It has no
dependency on zetesis's relational core, candidate search, reduct checker, CLI,
Rayon or wgpu. This permits future incorporation into themelios without moving
a solver dependency into the foundation. The sibling repositories and dependency
revision remain unchanged.

## Representation and ownership

An analysis borrows the exact `themelios_program::program::Program`. Its producer
and uncertainty evidence points back to original statements. It does not render
the program, reparse source, clone a second provenance system, unpool it, or
enumerate ground instances. Public values reuse themelios's signed signatures
and symbol algebra.

Source preparation is a separate responsibility. An unresolved include,
constant declaration or parameterized program part cannot be treated as though
its contextual interpretation were already known. A caller may analyze the
original program conservatively or a prepared program with retained provenance;
the result must identify which program it describes. The current source adapter's
bounded normalized program and upstream structural analysis are described in
[program-analysis.md](program-analysis.md).

The future solve integration can attach analyses and applicability witnesses to
a prepared program, then select candidate generation and reduct execution. An
analysis does not own a solve session or invoke grounding. A public typed source
admission entry point remains a separate item in the
[themelios-solve integration contract](themelios-solve-integration.md).

## Initial abstract domain

The first implementation uses a conservative argument-level domain: a finite
set of borrowed themelios symbols, including the empty set, or `Unknown`.
Each signed predicate argument has its own value domain. This loses correlations
between arguments deliberately; it does not claim every combination is possible.
The initial API returns `Unknown` for unrecorded signatures or invalid argument
positions even after a completed analysis. An explicitly recorded finite empty
domain is distinct from such absence. If unresolved program context or a global
resource stop prevents a closed conclusion, every query returns unknown information.

For ordinary positive literal heads, literal symbols contribute possible values.
A whole head variable obtains an upper bound from same-rule ordinary positive
whole-variable body positions. All producers contribute through a monotone union
and a bounded fixed-point iteration. Unioning multiple body positions is a safe
initial approximation; a later intersection can improve precision only when each
position independently covers that variable. Negative literals and comparisons
do not narrow this first analysis. Unsupported or generative head terms widen
affected positions. Choice and disjunction head positions initially widen rather
than transfer variables across element-local scopes.

Unresolved constants/includes, external or theory context, scripts, unhandled
statement forms, and non-base or parameterized parts conservatively prevent a
closed whole-program conclusion. A symbolic source name is not assumed to be
an already resolved constant. A completed abstract fixed point establishes
neither source admission nor successful finite grounding.

The initial crate is an independently callable analysis. It does not yet prune
the production solver, change a default plan, or establish a speedup. The crate's
README and tests specify the exact implemented forms and resource contract.

[Demand analysis](demand-and-magic-sets.md) is complementary: in the absence of
an explicit query, it uses `#show` as the modeler's observation seed. Demand
tracks what must be investigated for a task, while this crate bounds what may
occur globally. Candidate-scoped demand must not narrow a global domain result.

## Coverage and resource obligations

If `gamma(D)` denotes the concrete values represented by an abstract value,
each transfer must satisfy `f(gamma(D))` contained in `gamma(f#(D))`. The source
bridge must then establish that every eligible rule binding is covered. Values
found in currently known facts are not a complete domain for a predicate with
other producers. Failure to derive a value during an unfinished iteration cannot
justify its exclusion.

Bound structural inspection, term depth and payload, positions, retained values,
producer evidence, transfer work and rounds. Borrowing a deeply nested symbol
avoids cloning it but does not make comparison or traversal free. Per-position
finite-width exhaustion may widen that position. A global stop must expose its
resource and preserve soundness by making all public domain queries unknown;
it must not publish an unfinished finite map as an upper bound. The caller can
continue without the optional optimization. An analysis stop never means UNSAT.

Possible later domains include scalar categories, integer intervals, congruences,
argument equalities and bounded relational projections. Arithmetic, aggregates
and Rust callback ranges require their own transfer contracts. Aggregate bounds
must preserve complete tuple identity, signed values and empty extrema.
Negation can refine only with suitable completed or sound lower/upper evidence.
The existing lifted/filter and upper-bound Lean laws provide a semantic setting;
they do not prove this crate's source traversal, fixed point or resource handling.

## Scope and execution planning

Keep whole-program facts distinct from facts conditional on a splitting
interface or one candidate. Candidate truth cannot narrow the global possible
carrier. An atom true in every stable model may restrict outer candidates while
still being unsafe to force inside a reduct countermodel query. Domain facts are
analysis evidence, not additional source facts supplying support.

Only after a transform's coverage obligation is established should observed
domain widths, join selectivity, formula size and device costs choose its plan.
A poor estimate may slow execution; it must not change answer sets. Record the
analysis cost alongside construction, search, transfers and kernel execution.
Tests must include recursion, multiple producers, local scopes, signed symbols,
unresolved context, deep terms, widening and interrupted analysis. Small complete
instances can compare inferred coverage against an independent concrete closure
and external clingo, without making clingo a production dependency.
