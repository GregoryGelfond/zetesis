# Source parameters

Design proposal, 2026-09-07; no implementation or qualification is claimed. This
review uses the current dependency-tranche source and the unchanged themelios pin
`87c11a3f2b72b81a12fd53226941fdf95e7294d3`.

## Current capability

The frontend already resolves unannotated `#const` declarations, including
dependencies, checked arithmetic and finite structured values.
`extended::check_definitions_in` checks original parsed occurrences before raising:
duplicate source definitions, including identical ones, and either policy
annotation are refused. `extended::resolve` builds a dependency graph and evaluates
definitions in dependency order with the existing term fold and normalization.
Cycles and undefined/overflowing arithmetic have typed errors. These are native
profile boundaries; upstream represents both policy annotations.

`prepare_formula` and `prepare_bundle_formula` reach
`formula_ir::prepare`; the extended single-source and bundle doors reach
`extended::compile_owned`. Both use `extended::resolve`. Formula term-based
`#show` compilation independently calls the same resolver in
`observation/compile.rs`. Parameters must reach all these consumers, including
the metadata-only `SourceMetadata::compile`/`ObservationProgram::compile` doors,
or logical and displayed values can disagree. Objectives are normalized in the
formula path before their compilation.

Bundles already combine raised statements while preserving original files and
locations. Constants have one global scope across admitted roots and includes.
Identical resolved lexical include paths are included once; alias/redirection
refusals remain applicable. There is no sequential per-include override scope.

The ordinary CLI has no constant argument. `process::entry` parses `Options`,
and `admission::{source,bundle}` first attempt the extended profile under `auto`,
then may retry formula admission. Both attempts must receive the same parameters.
`SolveConfig`, `Session` and already-ground input have no source substitution
role; parameters cannot be applied after preparation.

## Proposed bounded API

Add an immutable frontend `SourceParameters` value (proposed name) with validated
entries consisting of a canonical `logical::symbol::Name`, an unevaluated
`logical::term::Term`, and parameter-origin evidence. A typed Rust constructor
and a bounded textual assignment constructor should produce the same value.
Use the pinned `syntax::parse::parse_term_value` with `Lexer`, `Dialect::Clingo`
and an explicit `NestingLimit`, then `logical::raise::raise_term`. Require complete
input and no parse/raise diagnostics. This existing term-value grammar admits
arithmetic and construction but excludes variables, pools, intervals and external
calls. Programmatically supplied terms need the corresponding bounded structural
validation. Split the CLI assignment envelope at its first `=` and validate its
name with the existing lexer; do not implement another ASP term parser or wrap
the input in a fabricated source directive.

Add parameter-aware preparation, extended admission, bundle and metadata methods;
retain existing signatures as empty-parameter adapters. Do not add required fields
to public `AdmissionOptions`, `BundleAdmissionOptions` or `Options`: that would
break existing struct literals and, for the admission options, their `Copy`
contract. An additive CLI invocation wrapper can flatten the existing `Options`
and collect repeated `-c/--const NAME=TERM` arguments, then call an additive
source-driver door. The binary uses that wrapper; the legacy `Options` parser
keeps its existing contract. Writer-free consumers use frontend parameters and
the existing prepared-input/session path, without clap or writers.

Make effective-definition selection a private shared operation before dependency
discovery inside the current resolver. Thread the same immutable parameter set
through each existing resolution call. Initially preserve those call sites and
their charges/error ordering; resolving once and reusing the result across
observation and logical compilation is a separate accounting change, not a
necessary part of this feature.

## Resolution contract

For the first slice, require every override name to identify one unannotated
source declaration. An undeclared name is a typed `UnknownParameter`-style
refusal, even if a same-spelled ordinary symbol occurs in a rule. This deliberate
restriction makes a misspelled or inapplicable N-queens sweep visible. It does not
claim complete clingo command-line constant compatibility.

Check every original source declaration before applying parameters. Duplicate
source definitions and annotated policies remain refused; an override must not
hide them. Reject duplicate parameter assignments, including identical repeated
values, instead of choosing an undocumented first/last value. With an empty
parameter set, retain current behavior. With a supplied value, replace that
declaration's effective right-hand side and retain both the source default and
the supplied expression as evidence. For example, `#const n=8. #const m=n+1.`
with `n=4` resolves `m` to `5`.

Discover dependencies from the selected expressions only. An override therefore
can remove a default dependency, introduce a dependency on another declared
constant, or introduce a cycle. Keep ordinary undeclared symbolic constants as
symbolic values under the existing resolver; do not treat every symbolic leaf
as a missing parameter. Continue using iterative dependency traversal and
checked evaluation, including nested symbolic substitution. An overridden
default is still parsed and profile-checked, but its discarded arithmetic is
not evaluated. That precedence, including whether an override breaks a default
cycle or suppresses `1/0`, needs explicit fresh clingo comparisons before the
new CLI advertises compatibility. Supporting `[default]`/`[override]`, duplicate
declaration precedence, or undeclared external constants is a later policy slice.

## Identity, provenance and bounds

Retain original source bytes, source identities, include edges and rule origins.
Store requested expressions and resolved effective bindings on private
prepared/admitted receipts with immutable accessors. A reproducible experiment
identity comprises original source/bundle identities, canonical parameter
configuration, effective values, frontend version/pin and relevant limits.
The source hash alone cannot identify the parameterized program. Existing
Program/Theory subjects continue to identify the actual admitted semantic object;
do not manufacture a source span for an external assignment or claim that its
configuration digest is a proof of semantic equivalence.

Use a distinct parameter origin, such as an argument index or caller label plus
an owned parameter-text catalog, for diagnostics. Resolution errors should retain
the offending parameter origin and relevant original declaration locations.
Analysis describes the effective normalized program or existing dependency
projection, with `AnalysisBasis` retained; it does not certify all parameter
instantiations of the source.

Add explicit bounds for parameter count, text/name bytes, syntax depth/nodes and
owned term payload; reject before parsing or copying beyond the corresponding
ceiling. Typed callers also require these checks. Count original declarations
under the existing `max_constants`; count parameter occurrences separately so
replacements cannot evade either bound. Charge selected-expression dependency
scans, substitution copies and evaluation through existing `TermWork` and
`ScalarBytes`; retain downstream value, template, grounding and theory ceilings.
Document extra graph/storage work and error precedence. A large `n` may pass
parameter validation and later reach a grounding limit; that is an incomplete
resource result, not UNSAT. No cache or solver algorithm change is needed.

## Acceptance and sweep boundaries

Existing evidence to extend is in `tests/extended.rs` (dependency resolution,
duplicate/policy/cycle refusals and original bytes), `tests/bundle_admission.rs`
(global declarations and include identity), `tests/structural_values.rs`
(nested constants), `tests/formula_preparation.rs`, `tests/metadata_api.rs` and
`tests/observations.rs`. Add independent propositions for empty-configuration
compatibility, default replacement, dependent defaults, parameter dependencies,
effective cycles, duplicate/unknown parameters, exact bounds, and source/bundle
diagnostic retention. Test malformed/trailing input, arithmetic errors,
constructor/string values and excluded term forms. Test one source through
extended, forced formula, automatic fallback, prepared and metadata-only doors,
including parameterized `#show`, aggregate bounds and objectives. A negative
control that omits the observation overlay must fail a display-value assertion.

Use unchanged N-queens variants 02–06, each currently declaring `#const n=8`,
with matched explicit native/reference parameters. The original `@count 92`
annotation remains exclusively the N=8 corpus contract. These annotations belong
to elenctic, the corpus author's ASP unit-testing system; they are external test
expectations. Solver execution treats them only as comments; the independent
validation tool can interpret them as test contracts. A separate sweep record
must carry N, source/configuration identities and fresh complete clingo contracts;
compare displayed-model multisets/counts and completion, and use independent
finite-ground original/frozen checks for the small semantic fixtures. Do not
claim hidden-model identity from `#show queen_at/2` output. Exhaust bounded small
sizes first, including SAT and UNSAT sizes, before selecting any larger timing
campaign. Variant 01 hardcodes `1..8`; it remains an N=8 control and is not a
parameterized variant. Its `n` override must fail as unknown under this proposal.

The blockers are the absent parameter configuration/provenance API, routing it
through every resolver consumer and automatic admission retry, and qualifying
the chosen precedence against clingo. The pinned parser/raiser/evaluator already
provide the necessary syntax and value machinery; no dependency update, source
rewrite, new grounder or N-queens corpus edit is required. No comparison or build
was run for this proposal.
