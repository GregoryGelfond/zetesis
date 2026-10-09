# Source programs and grounding

Source processing and solving are separate library capabilities. themelios owns
syntax, logical terms, program structure and source provenance.
`zetesis-themelios` checks the supported semantic profile and lowers it into
relational templates or a finite formula theory. It also retains objective and
observation data associated with that representation.

Text and canonical logical input meet at the same formula preparation compiler.
`prepare_formula` parses and raises checked source; `prepare_program_formula`
accepts `Arc<Program>` after bounded logical inspection. Both retain the original
canonical program, prepare the same scoped IR, and expose eager, hybrid and
adaptive materialization through `PreparedFormula`. The schedules keep their
existing applicability and cumulative budgets; none performs answer search.

The [source API](../rust/source.md#validate-input-and-control-preparation)
distinguishes validation without grounding, declaration purposes and cooperative
preparation control. Successful materialization retains the same complete-support
and reduct obligations.

Successful parsing is only the first boundary. Profile admission, binding
safety, arithmetic evaluation and resource limits can still fail. A frontend
syntax diagnostic, an unsupported zetesis construct, and an exceeded resource
ceiling have distinct meanings; none denotes an inconsistent program.

Possible support also supplies substitutions for source validation. A rule whose
body contains `not p` cannot contribute an answer-set atom when `p` is a fact,
but removing that rule from support can hide a later arithmetic error. For
example, `p. d(2147483647). h(X) :- d(X), not p. k(X+1) :- h(X).` still reaches
overflow during admission. Such pruning requires a certificate preserving
admission throughout the affected consumers, not only answer-set equivalence.
The current grounder retains those support rows.

The [checked source-preparation example](../rust/source.md) follows
`prepare_formula`, the retained analysis basis, `ground` and a complete CPU
collection. It contrasts two Boolean source families whose different counting
identities must survive materialization.

## Preserving source identity

`ProgramSite` carries an original canonical `StatementId` and optional real
source coordinates. IDs are assigned before normalization and remain tied to the
retained original program through generated rules, objectives and delayed work.
Constructed statements have identity without invented spans. Parsed coordinates
remain diagnostic evidence, including merged source occurrences. Keyed-constraint
rewriting uses this explicit statement association through canonical analysis
merges; ambiguous multi-statement projections remain written.

themelios's canonical choice collections preserve multiplicity through their
public `Identity` classification: repeated atomic elements merge by content,
while Boolean entries remain distinct. The formula compiler consumes those
counted entries directly. Provenance identifies original source evidence; it
does not determine counting identity or require a fabricated span for a
constructed value.

Each Boolean literal/condition pool product alternative receives a distinct
occurrence key before grounding. Grounding witnesses of that expanded occurrence
share its key. Separate enclosing rule groups remain separate. The original
written-element family is retained independently for arithmetic definedness and
warning handling, so changing a counting key cannot suppress required validation.
The [finite-occurrence laws](../lean/correspondence.md#representation-and-source-laws)
state the mathematical key and activity contract; Rust expansion and lowering
remain separate correspondence obligations.

Source and syntax limits still apply before raising and canonicalization. Typed
formula inspection bounds logical structure and retained provenance before
normalization. Both receipts retain the original canonical program in addition
to prepared/analysis structures; source receipts also retain original bytes.
Sharing the caller's `Arc` avoids copying that input but keeps its entire
allocation alive through preparation, grounding and retained typed failures.
Expansion budgets bound subsequent retained work. These logical limits exclude
allocator overhead and are not process-memory measurements.

At the formula boundary, tuple activity and atom permission remain independent.
For `{a}.1#count{1:#true:a;1:b}1.`, `a` activates the shared tuple through its
Boolean occurrence; an eligible selected `b` activates that same tuple through
an atomic occurrence. The tuple contributes once, while only positive atomic
head occurrences can supply atom permission. Neither a true Boolean nor a
satisfied bound supplies support for an atom in its condition. The exact three
answers `{a}`, `{b}` and `{a,b}` are covered by the maintained
[Boolean element contracts](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/integration/boolean_element_contracts.rs).

`BooleanHeadElements.coalesced_group_in_context` proves preservation for an
assumed keyed row family and covering atomic permissions;
`boolean_group_in_context` establishes that a Boolean-only measured head filters
the context's answers without supplying new atom support. Their
[proof boundary](../lean/correspondence.md) leaves source occurrence assignment,
Rust formula construction and execution refinement open. These are obligations
of zetesis's source bridge and solver; this architectural account is separate
from teaching themelios's parsing and logical-program APIs.

## Shared aggregate guards

Multiple written bounds on one completed nonnegative tuple contribution set use
[`append_aggregate_family`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-ferraris/src/aggregate/family.rs).
The source bridge evaluates every bound in order, retains its numeric or logical
comparison meaning, then validates the existing DAG and tuple conditions once.
One threshold table supplies the numeric roots; canonical remapping and the
original conjunction retain both original and frozen-reduct truth. Logical
constant guards still affect that conjunction and exclude optional numeric count
capture. A false bound never suppresses evaluation of a later bound. The enclosing
rule retains its source origins.

The preservation obligation is pointwise: each grouped guard root must have the
same original and frozen-reduct truth as its separately compiled counterpart
over the same completed contributions. Conjunction then preserves the written
bound group. [`Thresholds.threshold_query_exact`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/Thresholds.lean)
proves the classical nonnegative recurrence; it does not prove the shared Rust
DAG or its Ferraris equivalence. The scalar-versus-family regressions check both
truth relations, while that implementation refinement remains open.

This grouping applies to written guard lists of at least two entries whose
completed numeric contributions are all nonnegative, including counts and choice
bounds. Extrema and single-guard lists retain their existing compilation routes.
Any negative contribution retains scalar compilation because the scalar subset
ceiling applies independently to each guard; the family API instead has a
cumulative subset ceiling. Assignment proposal families already use the family
primitive and share the same source-side call and receipt handling.

An assignment aggregate's retained family is identified by its source occurrence
and the inherited variables read by its element tuples and conditions. Other
bindings do not duplicate that family. The assignment value and its written
guards still select the appropriate formula in each rule instance. These
families belong to one completed support snapshot; they never carry results
across growing support rounds. Complete tuple identity and eligibility formulas
remain unchanged, including their frozen-reduct meaning.

Count assignments coalesce complete tuple keys before constructing proposals.
Each distinct key contributes one, regardless of its first field or how many
witnesses produce it. On success,
[`formula_assignment::counts`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_assignment/counts.rs)
constructs exactly the ordered values `0..=N` for `N` keys, using
checked increments in one bounded buffer. Weighted sums retain subset-sum
construction. This count carrier still includes unrealizable values: the
original aggregate equality decides truth, and proposal traversal preserves
required source warnings and errors. Value, storage, work and cancellation
checks remain active.

Independent compilations share the ground builder's `FormulaNodes` owner. It
retains paired node and operand buffers and the extent of committed topology
validation; subsequent compilers inspect only the unchecked suffix. Reading
borrows complete child rows. Checked logical appends extend a fully validated
prefix; raw ingress remains unchecked. Append transactions own both suffixes. Rollback or
detachment restores both lengths and the checkpoint's validation frontier;
validation discovered through newly appended arena cells is discarded with them.
Canonical remapping cannot give replacement nodes removed validation. There is
no second formula representation or cached aggregate result in this mechanism.
Every call still checks its node and operand ceilings, tuple conditions and
control. Raw paired constructors retain full validation.

Final source admission consumes that same owner through
`FormulaNodes::prepare_admission`, retaining topology evidence until its fixed
`TheoryAdmission::admit` route runs. The core supplies the remaining scan charge:
`2N + R` for fully checked topology and `2N + E + R` otherwise, where `N` counts
nodes, `E` counts all logical child occurrences and `R` counts roots. Both routes
independently recount occurrences, scan atom IDs and validate roots; the saved
work is only the repeated child traversal. The source charges this amount before
publication with its existing work and cancellation checks. All dimension,
arena, atom-universe and root bounds remain independent. Raw re-entry discards
the frontier and receives full validation. The additional consuming-owner
correspondence depends on append, rollback and detach invariants; it is not
established merely by the raw-constructor extraction proof.

Aggregate work, node, operand and state ceilings apply to the whole shared
family, and its work is also bounded by
remaining formula work. The source charges guard preparation, weight inspection,
remapping and accepted compiler work, including a refused compiler prefix.
Collecting bounds before compilation changes the operation order and numerical
work cutoffs; a particularly tight former per-guard quota need not admit the
family. A refusal remains a typed failure, never an inconsistency conclusion.
Guard metadata and successfully returned root capacity remain charged together
through remapping. `AggregateFamilyBuild::root_storage_bytes()` reports the root
vector header and its actual capacity, excluding the DAG and compiler scratch.
Failed compiler transactions expose work receipts but no returned-root allocation
receipt; their internal transient allocations remain bounded by the aggregate
API's guard, operand and state ceilings rather than source support-peak
measurements.

## Canonical values and independent meanings

Source grounding computes with one canonical term and atom vocabulary. Bindings
retain term identifiers, relation columns retain those same identifiers, and
generated arithmetic values and compound terms enter that vocabulary through a
checked writer. Joins compare identities within the vocabulary; ordered
comparisons still use ASP term order. Identifier order has no logical meaning.

Comparing distinct canonical compound terms can reuse a shared immutable
constructor name. The descriptor comparison admits its exact slice identity;
equal storage needs no repeated byte scan. Kind, sign, arity and child terms
still determine the result. Different storage uses the ordinary content
comparison, and ingress values keep their existing checked comparison path.

Preparation admits static constants, signed predicates and constructor names
before the first support round. Compiled expressions and patterns keep component
coordinates and variable slots; they do not retain a second copy of those values.
`TemplateComponents` stores this metadata over the vocabulary used by generated
terms. A component coordinate identifies an occurrence in one preparation. It is
neither a portable term identifier nor evidence of semantic equality with another
occurrence.

Joining and emission borrow descriptions from the committed source prefix while
the writer admits generated identities. The prefix remains immutable during that
operation. Work and storage checks cover both retained component metadata and
temporary argument frames. Preparing and grounding separately carries the same
resource history across the boundary.

An early head check reads only the variables and constants used by that head.
If a required variable is not yet bound, the join continues. Otherwise a borrowed
pattern lookup checks canonical identity and discovery, followed by the separate
support test. It does not validate or copy unrelated binding slots. The later
insertion still checks each argument's logical limits before publication.

Several views can name the same atom without making the same claim about it:

| View | Meaning of membership |
| --- | --- |
| Completed support | The atom belongs to the computed upper bound on possible atoms. |
| Emitted theory | A local formula coordinate denotes the atom. |
| Objective condition | A local condition coordinate queries the atom in a candidate. |
| Projection | The atom participates in the program's projection policy. |
| Interpretation | The atom is true in this interpretation. |

These views retain their own occurrence maps over shared payload. Creating an
identity establishes none of these memberships. In particular, admitting a term
after support completion cannot add a support row, and a support-only atom cannot
shift the local coordinates of an already emitted formula.

Objective templates likewise retain variable slots, canonical constants and
pattern metadata over the source vocabulary. Their priorities and contribution
conditions remain separate semantic data. A constant does not become an active
contribution merely because its identifier exists.

The source writer is sealed before these views are published. A published view
keeps the shared prefix alive, including source terms it does not select. This
avoids payload copying but is a retention tradeoff: releasing eager lookup
indexes does not necessarily release all unselected source payload. Named storage
limits count shared payload once and include the live view metadata and
replacement overlap; they are not bounds on process RSS.

Hybrid checking borrows an immutable lookup over the admitted vocabulary. Each
checker owns its mutable bindings and workspace accounting. A missing identity
is an admission/refinement failure, never evidence that a literal is false.
This makes the remaining completeness obligation explicit: admission must have
visited every computed identity that the retained source checks can require.

## Eager and lazy execution

For admitted relational programs, **eager** grounding explicitly compiles a
complete bounded graph. This is useful when repeated candidate checks amortize
its construction. `GroundProgram::compile` is the operation that creates this
graph; constructing a `Program` or `Seed` does not silently invoke it.

**Lazy** relational execution retains source templates and streams relevant
instances during positive inference. It can avoid storing the complete ground
program. Candidate generation still owes coverage of all relevant gate choices,
and each completed closure owes coverage of all instances enabled in its final
snapshot. Laziness moves and limits materialization; it does not remove these
obligations or guarantee smaller memory use on every input.

The finite formula path materializes a bounded theory. Automatic admission can
first separate [terminal positive definitions](#terminal-definition-analysis),
ground the base and reconstruct those definitions from each verified base answer.
Explicit eager admission materializes the complete theory. An explicit
**hybrid formula** profile instead retains every producer and streams eligible
integrity constraints. It currently runs on CPU with indexed joins. Objectives
are scored only after the original constraint check completes. Constraints
containing aggregates, projected atoms or conditional
scopes remain eager; ordinary atom/scalar constraints use the shared binding
and evaluation operations. Existing eager entry points remain unchanged.

Hybrid admission still completes possible support and original source-family
arithmetic validation. It visits the eligible constraint instances to retain
their atom identities, without retaining their complete formula DAGs. Coherence
and support guards remain in the core, including guards that force unsupported
atoms false. Candidate truth cannot suppress a required admission error.

The execution composition is:

```text
admit complete support, arithmetic and atom identities
materialize all producers and ineligible constraints as the core
walk the core's candidate regions:
    narrow the region under the core and existing candidate restrictions
    exclude templates whose signed predicates cannot supply a sure body
    select held positive rows from completed possible support
    join the selected rows through the existing binding operation
    filter substitutions by their checked scalar conditions
    read each body against the region's lower and upper bounds
    any certainly true body -> refute this original candidate region
    otherwise               -> continue splitting or core membership checking
for each surviving answer of the core:
    check the streamed constraint instances against that answer
    violation  -> reject this proposal
    complete   -> accept an answer of the original program
    stopped    -> retain an incomplete outcome
    score the accepted answer when objectives are present
    use only accepted incumbents to restrict remaining candidates
return the requested answer selection, preserving all optimal ties when requested
```

Exhausting the core with completed checks enumerates the original answer family.
For optimal selection, exhaustive search under sound incumbent bounds instead
establishes the optimum and its requested ties. A core answer alone establishes
neither conclusion. The
[constraint-filtering law](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/streamed-constraints.md)
justifies this composition around the reduct. Its premises still require correct
source instances, original truth and complete coverage; it does not verify the
Rust cursor or backend implementation.

The early test has a narrower conclusion than satisfaction. In a region
`L ⊆ S ⊆ U`, a positive or double-negated atom is certainly true when it is
in `L`; a default-negated atom is certainly true when its atom is outside `U`.
A conjunction of such literals, after its scalar guards pass, violates its
integrity constraint for every interpretation in the region. One witness
therefore suffices to reject that region. Finding no witness returns
`NotRefuted`; final original-constraint checking remains required. The
[region-refutation laws](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/StreamedRegions.lean)
make the applicability and coverage argument explicit.

Streaming also permits consequences before a body becomes certainly true. If
exactly one body occurrence is open and every other literal is true, the open
literal must be false. Thus `:- p, not q.` holds q when p is held; `:- p, q.`
cuts q in the same situation. Positive and double-negated pivots are cut;
default-negated pivots are held. Several open occurrences remaining after source
normalization are conservatively left undecided, even when they name the same atom.

Candidate closure alternates formula propagation with source deductions:

```text
begin one source allowance for the candidate region
repeat:
    close the original formulas and candidate restrictions over the region
    stream joins of original constraints whose scalar guards pass
    if a body is certainly true: refute the region
    if one body occurrence is open: make that literal false in the region
    otherwise: select the next split, or submit the complete candidate
```

Each consequence decides a previously open atom, so the region's atom count
bounds the number of consequences. All source passes share one allowance;
continuing after a consequence does not renew it. An interruption leaves the
region unresolved. These consequences restrict candidates; they provide no
support for atoms and do not replace final original satisfaction or reduct
membership.

Within the admitted constructor-and-comparison fragment, one rule scan may
produce a bounded batch of deductions. Each remains valid after the region
narrows, so the solver consumes them one at a time with formula propagation
between them. Exact held/cut masks authenticate this reuse; a sibling region
starts fresh. A completed scan with no deduction is reused only while every
possible atom read has unchanged bounds. If one atom changes in a signed
predicate, fixed constants or constructor shapes can establish that none of a
constraint's occurrences can read it. Variables remain unconstrained for this
test; several changed atoms retain predicate-level invalidation. These charged
checks use the existing packed masks and predicate dependencies, without a table
of ground constraints. After a completed scan found no deduction, one newly held
atom can also restrict the next scan to its positive occurrences. Changes are
accumulated until that rule is scanned; a second distinct change, a cut, a
negative reader or generated bindings retains a full scan. Every matching
occurrence and the ordinary body test remain required. Potentially failing
arithmetic retains the sequential scan. The
[batch and dependency laws](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/streamed-consequences.md)
state the semantic premises separately from runtime error and storage handling.

This is an original candidate restriction. It never enters a proper-subset
search under a frozen reduct, and never replaces that membership check. The
optional existing clause-search route retains final constraint checking without
this region operation.

The [shared source owner](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_hybrid.rs)
retains prepared constraints, the closed canonical base and the support
relations (with their postings) that those constraints read; discovery and
order indexes and other relations are released when admission closes support. Each checker has its
own mutable state and borrows a checked dense atom lookup on its first candidate
operation. A prepared correspondence maps support rows to original dense atom
IDs without copying their tuples. Refutation selects held positive rows before
binding or scalar evaluation. In the admitted total fragment, consequences use
one traversal that permits at most one mapped open positive occurrence. Other
mapped positive rows must be held. Partial arithmetic and generated bindings
retain separate held-only and designated-open-occurrence traversals. A missing
correspondence conservatively retains the row without lending truth evidence.
A necessary template test also checks whether each signed predicate
has any held atom, or any cut atom for a default-negated literal. The latter
uses the whole original catalog, including unsupported negative occurrences.
These selections can retain extra work; the complete body test still establishes
each refutation. They do not filter the separate arithmetic-admission traversal.
The final candidate check separately selects mapped positive rows present in
that candidate's own model. It retains unmapped rows and checks every completed
body against the same model; earlier region decisions supply no authority.

A selected join can lend evidence that every positive occurrence in its current
body already matched a mapped, held row, or that exactly one matched occurrence
is open. In the latter case it also lends that occurrence's dense atom identity.
The cursor keeps constant-sized prefix evidence; backtracking truncates it with
the binding. Once scalar checks pass, the body check can omit those repeated
positive atom lookups.
The evidence belongs to the exact body, binding and immutable region, and ends
before cursor advance. Unmapped rows, generated bindings and head suffixes keep
ordinary lookup. Negative and double-negative literals still inspect their
truth; complete model checks never use this region evidence.

Invalidating a pending batch does not scan the atom catalog. The undelivered
entries identify exactly which deduplication slots need clearing. Cleanup is
charged and must finish before any new consequence can be delivered; interruption
retains cleanup progress without retaining authority to use the old deductions.

The core also shares an immutable mapping from rules to signed predicates and
from original atom IDs to those predicates. The first successful builder
publishes it; each checker admits its retained capacity against its own storage
ceiling. Candidate masks, dirty flags, pending deductions and workspace remain
local. Predicate references used while building the mapping are temporary.

Native persistent workers retain their checkers; scalar pulls and joined
producer rounds prepare checkers for their operation and borrow the same core
mappings. The indexed query wrapper borrows existing indexes and allocates
nothing. Ordinary rows remain borrowed,
and typed candidate lookup copies no atom. The eager builder keeps its bulk
materialization path. No channel or task is created for each instance.

A hybrid checker also retains a rule's join order and comparison schedule after
that rule first needs a scan. The plan belongs to the exact rule and completed
support owner; it carries no candidate truth. Each scan starts fresh binding,
probe and evaluation state. Preparation is charged once and retained plan storage
counts against the support-byte allowance. A failed preparation publishes no plan.
Ordinary eager joins still prepare against their current support snapshot, whose
row counts can change as the fixed point grows.

Each source model check, region-refutation check or candidate closure gets its
own work, substitution and scalar-capture ceilings. All consequence passes of
one closure share those ceilings; publishing a pass's receipt does not renew
them. Checker preparation belongs to its first check's allowance.

Checkers share a cumulative receipt, not a cumulative admission ceiling.
Charges are admitted locally before their operations and published when
preparation or a check returns, fails or unwinds, and when a checker drops.
In-flight charges may remain local; live receipt fields are independent,
monotone lower bounds. Joined receipts are exact until their fields saturate.
Zero work charges still poll cancellation. The final outcome joins workers
before publishing settled receipts. Region attempts and refutations remain
distinct from core answers checked, original answers accepted and
frozen-reduct work.

Streaming can avoid a constraint-node or root ceiling, but the finite atom
and support envelope must still fit. Retained source plans and support also
occupy memory. Early refutation avoids some complete candidates, but repeated
region joins and checker preparation can increase execution work. Reduced
formula storage therefore does not imply a time
or RSS improvement. Full demand-driven producer/atom discovery and device
constraint streaming are not implemented by this profile. See the
[checked hybrid session example](../rust/sessions.md#hybrid-formula-sessions).

## Source instances as a composition

The useful lower-level operations have logical contracts:

| Operation | Input and result | Obligation |
| --- | --- | --- |
| Bind | Join finite positive witnesses into substitutions | Preserve repeated-variable agreement and local scope |
| Filter | Evaluate scalar conditions on bound values | Never invent missing bindings or use undefined arithmetic as a value |
| Gate | Test frozen positive/negative candidate conditions | Use the candidate, not the growing consequence set |
| Project | Construct a head or constraint instance | Preserve the complete atom and its source instance |

The formula path evaluates terms as finite expression plans. In strict mode,
each operation reads the completed prefix of earlier results. The final operation uses the same
checked evaluator and returns its value directly; only intermediate results
occupy scratch storage. Storage reuse retains each executed operation's work
admission and strict first-error order. Source mode separately tracks missing
results in a transient mask bounded by the admitted expression's node count.
The mask is reserved fallibly and cleared after each evaluation; it does not consume the
cumulative scalar-payload allowance. Continued independent checks retain their
per-node work charges. The [evaluator](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_support/evaluation.rs)
clears its value prefix on success, failure and unwind, retaining at most 32 empty
integer cells and 32 empty term-ID cells between evaluations. A single constant
or variable returns its authenticated existing canonical key, including numeric
leaves, without reconstructing that term. It needs no intermediate cells or
missing mask; an unavailable partial variable still reports typed undefined
arithmetic. Workspace admission and node work remain charged. Constructing
composite results can allocate. This storage schedule has a separate
[preservation law](../lean/correspondence.md).

A positive body is joined in an order chosen once per join, before a row is
read, from what the body says: each relation's size, the variables each
occurrence binds and the variables each comparison waits on. Extending the
bound prefix one occurrence at a time, the
[criterion](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_support/order.rs)
takes first an occurrence whose variables are all bound, which is a test and
never widens the join; then an empty source, followed by the generator that
decides the most waiting comparisons. A false comparison can then prune before
deeper joins multiply the rows. Among equally decisive generators, a constant
or already-bound whole argument takes precedence over an independent domain:
that argument can use an equality posting. The remaining criteria prefer the
smaller relation, then progress toward other waiting comparisons, and
otherwise the earlier occurrence of the canonical body, which orders literals
by predicate and then by variable name rather than by their position in the
source text. A known argument is a preference, not a bound on the number of
matching rows: a skewed posting may contain the whole relation, and a comparison
may accept every row. This ordering criterion does not treat bound children
of a structured argument as a known whole argument. Exact structural probes
resolve those children later, at the selected occurrence.
Every order yields the same complete bindings, and the
semi-naive partition reads source occurrence, not this order. In the pruning
path, a comparison is checked at the depth whose occurrence binds its last
variable. A defined false comparison can prune the prefix. Complete arithmetic
evidence traversals retain false rows to establish joint definedness instead of
using that pruning. An evaluation failure met at a depth is retained until that
depth is undone and is classified only for
a complete substitution no independent comparison excludes, as the
[language reference](../reference/language.md) states. Binders, interval
checks, tuple comparisons and guards are validated on the substitutions the
comparisons leave. The order decides how early an exclusion is decided,
never whether it is.

Source-family evidence is finalized over completed support and the original
source occurrence, not one support round or normalized fragment. Evaluated
numeric division or remainder by zero omits an instance only when the same
family also contains a jointly defined instance; a defined false instance is a
witness. An entirely undefined family refuses admission, while an empty positive
join is silent. Each local element has a separate family for each fixed outer
binding. Original objective-element identities keep their pooled fragments
together without merging distinct elements. Successful owners retain one typed
warning per original statement site within the finite warning ceiling.

The separate family-evidence traversal is omitted for a restricted class of
flat constraints: every normalized fragment of the original source occurrence
contains division or remainder, and every divisor operation directly names a
canonical nonzero numeric constant. Atoms and scalar comparisons or guards may
appear in the body. Ordinary comparison heads lowered to constraints can also
qualify; generated values, local scopes and other lowered heads retain the family
pass. This classifier alone retains complete substitution checks; the separate
finite totality certificate below can justify ordinary row selection. Constant
divisors certify only the absence of zero divisors, not total arithmetic: nonnumeric operands, overflow and invalid exponents remain checked
by ordinary eager instantiation or hybrid capture. A fatal error can therefore
be reported in rule instantiation instead of support completion. Classification
reads spend grounding work; omitted joins and binding copies spend none.
Resource limits remain cumulative, so the numerical work needed for the same
source can decrease without changing the completed arithmetic verdict.

Complete-row scalar comparisons may reuse successful unary projections within
one join. Descriptors borrow the exact source expressions and are prepared when
a complete row is first reached or a finite totality attempt needs them. The existing canonical `TermTable` indexes
each distinct input; a `Binding` retains its corresponding successful result.
Value reuse alone copies no scalar payload and skips no row or comparison. Leaves,
multivariable expressions and partial evaluations use the ordinary evaluator;
failed computations are not retained, so later rows still supply their original
arithmetic checks and family evidence. Hits authenticate the canonical owner and
accessible term prefix, clear stale zero-divisor state, and spend work under the
same cancellation and live-storage boundaries as misses.

For one expression with D distinct successful inputs, the projection retains
O(D) scoped IDs and positions. Lookup takes O(log D) identity comparisons;
inserting all D values can require O(D²) position shifts in the shared sorted
coordinate index. Descriptor preparation, misses, lookup and insertion remain
cumulative grounding work, while retained metadata uses `SupportBytes`. The
[projection regressions](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_support/projections/tests.rs)
check value reuse, owner boundaries, arithmetic failures and inclusive limits.

After source-family validation, a flat constraint can establish finite totality
from completed positive columns. Every expression must be a leaf or read one
source variable, with no constructor, generator, structural capture or nested
scope. A smallest covering column supplies all possible values of that variable
in complete bindings. Each expression is checked on that whole domain, and an
arithmetic error declines this optional certificate. Resource and owner failures
remain located failures. Only success for every expression permits ordinary
prefix pruning. A reached prefix may still contain a value absent from its later
covering column; its arithmetic failures retain the ordinary deferred handling.
Evidence cursors always traverse complete rows.

Hybrid capture and later model/region checks use this same preparation and
selector. Capture runs after original family admission, with the completed
carrier and an append-capable canonical term owner. It admits successful values
from the whole covering columns, including values belonging to rows that cannot
complete the source pattern. On the first reached runtime use, the prepared rule
checks that optional certificate against its frozen owner and full completed
carrier, before any candidate row filter applies. Later cursors borrow the same
successful map read-only. A missing frozen term remains a typed failure, not an
optimization fallback. An arithmetic decline retains ordinary checked traversal;
it supplies no certificate and needs no identical totality retry. Resource and
owner failures stop preparation without publishing a partial map. Candidate
filtering establishes no new source completion or arithmetic certificate.
Preparation and retained map storage are charged once per prepared rule; each
cursor still pays for its queries, bindings and temporary domains. Inputs outside
the map use the ordinary evaluator without changing that retained map. These
lifetimes keep mutable scratch local to each check.

The same preparation seals each projection's covered input prefix. At probe
preparation, an equality between a covered unary expression and a known operand
can select the inputs whose results equal that operand. A missing known input
supplies no restriction. One restricting equality per unbound variable supplies
a necessary finite domain; all comparisons remain with the residual checks.
Multiple variables' domains meet the ordinary constant and bound-variable
restrictions through the existing `Table::select` operation. The selector keeps
original relation positions and whole-tuple aliases, then the ordinary matcher
checks every retained row. This also applies under `JoinStrategy::Indexed`;
ordinary probes continue to use their requested strategy and both paths share
one lazy table workspace. No table is prepared when every covered input remains
allowed or the opposite operand is unavailable. An empty derived domain uses an
empty probe directly.

Domain derivation scans the existing input/result coordinates, without another
value dictionary or reverse-result cache. Two transient leased buffers hold
borrowed input references and their variable ranges. Preparation, domain scans,
mask construction and all refused prefixes spend the same cumulative grounding
work; tables, masks and lists share `SupportBytes`. Numerical work cutoffs can
change, and optional preparation can itself refuse. These operations establish
no runtime speedup or default-budget admission for an arbitrary program. The
[computed-domain regressions](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_support/projections/domains/tests.rs)
check exact selected bindings, typed equality, evidence coverage and refusal
boundaries. The [Lean correspondence](../lean/correspondence.md) states the
separate obligations connecting finite coverage, table selection and reduct
preservation.

Each formula join owns one reusable expression workspace. Prefix checks, binding
generators and final filters borrow it in sequence; pending generators do not
retain another workspace. Strict evaluation stops at its first fault. Source
evaluation uses the same checked scalar operations but continues independent
branches within the reached phase after a numeric zero divisor, so an independent overflow, type error
or invalid exponent remains fatal. It does not evaluate a parent whose operand
is undefined. Body and condition selection still precede head and consequent
evaluation. An omitted body does not enter those later phases or objective
fields; a defined false body does not enter head or consequent evaluation.
The relational comparison exclusion above remains separate from
rejection by a binder, interval, tuple comparison or aggregate guard; those
rejections cannot hide required arithmetic in other fields. Closed constants
and post-solve observations retain their strict checks. The [caller regressions](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_support/evaluation/scratch/tests/callers.rs)
check these actual consumers as well as their values and failure boundaries.

Formula bindings retain source variable identities in optional slots. A pending
producer or an unrelated component variable is absent; numeric zero remains an
ordinary value. Body-prefix views borrow only the original body scope, excluding
head-only generated slots. Every scalar and atom reader checks availability and
returns a located failure for a missing required input. Local joins cannot bind
an absent outer input. Generator backtracking clears exhausted outputs before an
earlier input changes. Owning frame capacities, including optional cells, are
charged separately from copied value payloads.

Single-slot transfers copy authenticated canonical IDs between the existing
frames without creating temporary ownership handles. Source authority, slot
presence and destination checks retain their specified order. Comparisons whose
two operands are immutable variable or constant leaves likewise borrow their
values from one reader. General expressions retain owned keys while later
evaluation may extend the vocabulary. Both routes retain arithmetic diagnostics,
workspace admission and interruption checks.

Support generation and rule instantiation consume ordinary completed bindings
through the same borrowed-row mechanism. Head admission finishes before the join
resumes its suspended undo; it does not need a second assignment vector. A nested
join reads the borrowed outer prefix to initialize its own scoped frame. Arithmetic generators that
retain a continuation still own their frames. Both routes apply the same scalar
filters and source-family checks; their allocation and work costs differ.

The [binding implementation](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_binding.rs)
and [scope laws](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/BindingScopes.lean)
state these distinct responsibilities. The laws prove restriction and checked
read properties for abstract partial assignments; concrete source compilation,
arithmetic error order and Rust execution remain correspondence obligations.

Formula construction has an explicit ownership boundary. Source instantiation
consumes the source IR and owns the completed support catalog and its snapshot
while it emits formulas and activates objectives. Eager grounding releases that
support after its consumers finish; hybrid grounding retains it with nonempty
streamed constraint plans. An empty streamed plan releases both. A consuming
builder then adds coherence and support guards.
It passes nodes and roots into theory validation and retains the ordered atoms
and origins for the compiled owner. Interning indexes, producer tables and
aggregate caches remain construction scratch.
The [finalizer](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_ground.rs)
preserves vector order and cumulative work/resource charges. Releasing completed
scratch reduces overlap between phases; it does not establish a lower earlier
construction peak or process RSS. The [ownership chapter](ownership.md) relates
these lifetimes to prepared views and execution state.

The final formula catalog uses the shared core
[`AtomInterner`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-core/src/catalog/interner.rs).
It owns one canonical term/atom authority. Authenticated tuple queries reuse that
store's exact identity index, then an integer-key inverse translates identity to
discovery. A known same-authority atom ID avoids tuple lookup altogether. An
interned row without a discovery entry remains absent from this population.

A separate semantic AVL view, grouped by predicate, provides typed enumeration,
foreign input lookup and insertion placement. It does not make canonical IDs
into semantic ranks. Insertion plans both index updates in reusable scratch and
admits all capacity and publication work before publishing discovery. Complete
canonical rows can survive a refusal without entering either discovery index.
Both views retain only metadata over the same authoritative payload; their
actual capacities and growth overlap are charged.

Prepared head patterns retain candidate locations for their predicate's column
block and discovery subtree. Each insertion checks that those locations still
name the same signed predicate; a changed directory uses ordinary lookup.
Tuple identity is checked afresh, so no absence result survives an insertion.
This reuses predicate preparation across a family without another atom store.
The larger preparation headers remain included in resource accounting.

A committed prefix supplies the count-plan collector's exact dense occurrence
view. The first commit transfers the pending ID map; later commits move only
pending IDs after borrowed views end. Finalization transfers that mapping and
shares the canonical prefix with an immutable `AtomCatalog`. Possible support and emitted atoms retain
distinct populations, and commitment establishes no truth.

Publication extends the current segment directories when their snapshot is
exclusively owned. Existing entries move only when a directory grows its
capacity; successive small publications therefore take amortized linear
directory work. A retained snapshot instead requires a fresh directory, leaving
the older readable prefix unchanged. Canonical payload is shared in both cases.
All work and reservations precede visible publication. Refusal may retain spare
capacity, which remains charged, but exposes no partial prefix. This local reuse
introduces no shared execution queue or synchronization between workers.

The located source
[adapter](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_ground/atoms.rs)
charges lookup, canonical import, index construction and commit against cumulative
formula work; exact refusal cutoffs can change with these operations.
`FormulaLimits::max_atom_storage_bytes` independently bounds the shared source
authority during publication: canonical payload, indexes, current prefix
directories and scratch, including named growth overlap. Its payload includes
identities outside the emitted occurrence map; unrelated support indexes and
query buffers retain their own bound. A refusal preserves existing discovery IDs,
although complete canonical components and capacity can remain.
`FormulaResource::AtomStorageBytes` reports this storage ceiling;
`FormulaFailure::AtomCatalog` preserves canonical faults, while vector reservation
failure retains its `TryReserveError` in `FormulaFailure::AtomAllocation`.
The separate scalar-byte budget bounds cumulative source expansion. Neither
measure is process RSS.

An atom in a **possible support relation** is a witness available to source
enumeration. It is not thereby true in a candidate, and an aggregate's proposed
result is not thereby its evaluated result. The emitted formula must retain the
original activation and equality conditions. Structured positive witnesses can
bind local variables before dependent arithmetic is checked. Positive-witness
matching does not invert arithmetic or introduce a global guessed value universe.

A checked `AtomKey` borrows a pattern and its current binding. Support and delta
membership use that full typed identity without making a temporary atom. Formula
head admission validates a borrowed projection of constants and assigned
variables. The same immutable coordinates serve discovery lookup and canonical
row interning before publishing a discovery position. No argument vector,
replacement assignment or owned atom is needed. The projection preserves repeated
arguments and accounts for its fixed preparation metadata. Constant resolution
may still read the canonical segment directory; borrowing avoids copying, not
all lookup work.
Missing required inputs still produce a located failure; an incomplete head
prefix defers the membership check. Membership does not discharge authored-body
validation. Existing support, current delta and formula atoms retain distinct
roles even though they share the identity operation.

### Keyed constraints

Before completion, preparation reads the program's keyed relations, the
choice rules `1 { p(K, V) : c(V) } 1 :- b(K).` that are their relation's only
producer, and asks every constraint that reads such a value only to compare
it as the one atom the key admits, in the two patterns the
[source guide](../rust/source.md#constraints-over-keyed-values) states with
their meaning arguments. The transformation is per rule and changes no
answer set; what it changes is the grounding: a disequality over a product
of a demanded value with every value the key admits becomes a negated lookup
of the demanded atom, and the product is never formed. The asked statements
keep the written constraint's provenance and take the place of its compiled
rules; the rest of the program is compiled once. The analysis is bounded by
the key work ceiling and the term work remaining, charged to the term work,
and a stop, reported on the admitted formula and by the CLI, leaves the
constraints not yet asked as written.

### Completed possible support

Formula grounding grows possible support by complete rounds. Each round uses an
immutable snapshot, exhausts the admitted positive joins and binding proposals,
and collects new positive head atoms. Aggregate and conditional truth remains in
the emitted formulas; it does not prune possible producers. A proposed aggregate
assignment value retains its original equality.

An ordinary nonempty head with a checked aggregate binding plan can reuse a
completed support continuation when the next positive binding agrees on every
relational input read by that continuation. The compiler's
[`continuation_inputs`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_assignment_ir.rs)
summary covers generators, scalar filters, arithmetic, negative literals,
aggregate tuples and conditions, and the head. It excludes generated targets
and reads confined to the positive witness patterns. Thus different anonymous
witnesses can share head production without treating their atoms as true.
Rich heads and unplanned scopes retain their existing traversal.

Reuse is local to one rule and immutable support snapshot. Positive matching,
row restrictions and prefix checks still run before the comparison. Only an
exhausted continuation qualifies: all its selected heads have been visited,
and its defined-witness and first-error evidence remains in the join. The
existing cursor frame owns the comparison inputs; no additional key map or
binding snapshot is retained. A different or missing input restarts the
continuation. Every support round creates fresh joins. Final formula emission
continues to visit every original witness, preserving its activation, aggregate
equalities and provenance.

Final formula construction can also share a continuation, with a different
observation: it retains every positive activation. For an ordinary nonempty head
with the checked plan, flat positive patterns and total prefix comparisons,
[`formula_factor::continuations`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_factor/continuations.rs)
groups consecutive bindings with equal continuation inputs. It forms a
disjunction of their whole positive-witness conjunctions and conjoins that guard
with each shared continuation's remaining body. Each resulting implication and
producer keeps the original head and source origins. Correlations between atoms
within a witness, aggregate equality, and negative conditions remain formulas;
possible support does not establish their truth.

The first selected row completes body filters and head arithmetic before witness
atoms are published. Empty or entirely rejected continuations introduce none;
rejected rows retain scoped body validation. Lookahead uses the existing matcher,
row restrictions and a lent base frame, while the active proposal keeps its own
comparison certificate. A differing key starts another run, including when an
earlier key reappears. One retained proposal row and the formula guard replace
repeated proposal products; there is no global grouping table. Partial prefix
evaluation and other unchecked shapes keep complete traversal. All ordinary
storage, work and interruption checks remain active, and an unfinished run
cannot produce an admitted theory. The finite factorization law preserves
original and arbitrary frozen-reduct truth; source scope and runtime admission
remain separate implementation obligations.

Preparing the summary scans the source footprint twice. With V outer variables,
S plan steps and F source-footprint entries, its worst-case work is O(V(S + F));
all inspections are charged to source term work. Its retained slot indices are
charged once to `ScalarBytes`. Comparing a new frame costs O(V + K) for K retained
input indices, with canonical scope checks and the simultaneous old/new frame
storage admitted through the existing work and byte limits. The compiler's
source-footprint extraction and this producer-continuation correspondence are
executable obligations, not a Lean-verified Rust implementation. The
[support continuation regressions](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_support/continuations.rs)
compare full support, formula graphs, diagnostics and provenance with reuse
disabled.

For a normalized positive-flat program without objective declarations, a private
[producer plan](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_support/producers.rs)
checks every original IR occurrence against the rule certificate also used by
optional domain guards. Scheduling separately excludes objectives, including
empty declarations; domain guards permit them. The plan checks signed predicate
names and arities, positive dependency edges and the existing themelios SCC order. Canonical source
rules may coalesce duplicates; plan slots still refer to the original IR rule
array and preserve its provenance. The plan borrows that preparation and caches
positive body occurrence IDs. Reverse signed-predicate postings mark original
producer IDs in a packed active set. Bootstrap visits zero-input producers;
later rounds visit only producers affected by the previous round's newly
published predicates. Selected producers retain their original order and the
same first-new binding partitions. All selected scans finish before new atoms
are published. An empty active set still admits the final support round but
needs no new catalog/query snapshot.

Plan construction and traversal consume the cumulative formula work allowance.
Its header, occurrence arrays, borrowed predicate postings, packed active set
and temporary construction arrays count
against `max_support_bytes` beside the live catalog and query views. Preparation
uses checked signature searches, and charges each dependency validation one
logarithmic lookup in the upstream graph plus the edges it walks, so the plan
costs O(B log N) for B body occurrences over N predicates. Wake lookup,
posting visits and packed-set reads/writes are also charged. Temporary graph metadata is released after preparation, and
the plan is released before completed support is returned. These
checks qualify support scheduling, not satisfiability, unique-answer claims or
source-to-Rust semantic refinement. Richer programs keep the existing schedule.
`GroundingWork.support_producer_visits` counts charged entries into actual rule
variant traversal; `support_snapshot_preparations` counts charged growing
snapshot attempts, including attempts that subsequently fail. These optional
fields do not retroactively turn missing historical observations into zeros.
The [producer scheduling laws](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/producer-scheduling.md)
state complete input registration and published old-head history as explicit
premises. Fewer visits do not by themselves establish a timing improvement.

Structural positive patterns stage borrowed canonical subterms before changing
bindings. Each join owns one reusable capture buffer under its existing support
storage lease; capacity growth consumes scalar-byte reservation, while actual
retained capacity and replacement overlap remain under `max_support_bytes`.
A mismatch or refusal clears the staged captures, and a successful match clears
them after committing bindings to the ordinary undo trail. This changes neither
candidate-row selection nor constructor, constant and repeated-variable checks.

After capture, lazy constraint checkers can prepare
[`PatternRows`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_support/prepared/pattern_rows.rs)
over completed support. The ordinary structural matcher selects source row
positions using an empty incoming binding, preserving constructors, constants
and repeated-variable relationships. Later bindings add equalities, so they
cannot make an excluded row match. Each probe chooses this selection or a
smaller existing equality posting, then runs the ordinary matcher and body
checks. Original rows and canonical terms keep their existing owners; the
selection carries no candidate truth. A full, nonselective list is discarded.
Preparation visits each source row and charges temporary and retained capacity.
Selections live with the checker's prepared rules; independent checkers prepare
and retain their own selections. The
[correspondence boundary](../lean/correspondence.md) distinguishes the necessary
selection law from executable matching, ownership and resource obligations.

Ordinary positive atom heads with positive flat or structural witnesses and pure
scalar checks or generators use [delta joins](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_support/delta.rs).
Flat ordinary `not` and `not not` atoms may occur between these inputs. They
provide no bindings and do not test truth during possible-support discovery;
their arguments use the existing admitted bindings and scalar evaluation.
Their truth remains in the original emitted formula and its frozen reduct.
Each new tuple combination has one first source occurrence that selects a newly
appended row. Earlier occurrences select old rows; later occurrences select all
current rows. This partitions the combinations even when one predicate occurs
several times or cardinality planning changes execution order:

```text
for pivot in PositiveSourceOccurrences(rule):
    inputs = MapOccurrences(rule, occurrence =>
        oldRows       if occurrence < pivot
        newRows       if occurrence = pivot
        currentRows   if occurrence > pivot)
    proposals = Union(proposals, JoinAndEvaluate(rule, inputs))
```

Each variant supplies these populations to initial join preparation. Ordering
and comparison readiness are built once from its old/new/current row counts;
the cursor does not first prepare an unrestricted order and then replace it.

An ordinary producer without positive inputs runs once, even if it has negative
non-input atoms. Changing such atoms' possible presence cannot enable another
head proposal, because support generation ignores their truth. Aggregate,
conditional, projected and nonnormal producers retain full-round traversal.
Structural occurrences use the original whole-row capture and the same nested
constructor, constant, repeated-variable and anonymous-pattern checks. Bound
structural postings intersect the old/new interval by actual relation row position.
This per-rule certificate does not require the whole program to be positive:
objectives or richer rules elsewhere leave eligible normal rules eligible, while
the separate whole-program wake schedule retains its narrower applicability.
Every selected input still passes the same typed tuple matcher and scalar evaluator.
An empty proposal set establishes completion only after every required variant
and conservative producer has finished. Final formula emission visits all
complete authored-body families and preserves their definedness evidence,
including jointly defined false instances. Independent relational exclusions
and empty joins supply no arithmetic failure. Possible support membership alone
cannot conceal a required arithmetic error or establish an all-undefined family.

The preservation argument concerns possible heads, not answer-set truth.
Removing these flat negative non-inputs leaves the positive occurrence order,
binding generation, scalar validity and head projection unchanged. Old
combinations have already proposed their heads; each new combination belongs to
one delta partition. `NormalSupport.propose_gate_independent` states the
corresponding gate-invariance law for mathematical normalized rules. Connecting
the compiler's typed instances and checked expressions to that law remains a
representation obligation. Scheduling fewer old combinations can change the
charged work and the first bounded refusal; it does not change the cumulative
generated-value population or turn a failed attempt into completed support.

The [support builder](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_support.rs)
returns `CompletedCatalog` only after an entire round adds no atom. Its
`CompletedSupport` view borrows the same authoritative catalog used by final
formula grounding. Intermediate snapshots have no completion capability, and a
round, work, value or storage failure returns an error before objective activation.
A finite snapshot during growth does not establish that value generation will
terminate.

Before publishing a round, the builder orders its new support positions by
typed atom identity. Dense selections filter the canonical catalog's existing
semantic ordering index; sparse selections use in-place comparison sorting.
The dense route is selected only when the complete indexed population is at
most twice the selected population, the selection has at least two entries,
and its prospective temporary-storage envelope fits the current allowance.
Actual allocator capacity is checked during execution and may still cause a
refusal. Thus a chain adding one atom per round does not rescan its history.
A failed chosen operation remains a failure; it does not trigger a retry through
the other strategy.

The selection contains exactly this round's newly supported discoveries. It can
include an older discovery that was not previously supported, so neither an ID
suffix nor the global support bitset is a substitute. The dense route uses a
temporary membership mask and position vector, retaining the caller's selection
until every replacement write is admitted. Both routes preserve the same typed
publication order and use the same canonical payload.

The shared [source-activity module](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_source_activity.rs)
uses this completed view for objective and projection eligibility. Acyclic
producer dependencies are evaluated in order. Unresolved components begin with
optional atoms from complete possible support, then aggregate every producer's
activity in complete rounds. Each changing round resolves at least one optional
atom as required or absent; established information cannot be retracted. The
old and new activity tables are admitted together. This finite refinement does
not enumerate answer sets or decide whether optional literals can hold together.

Rich producer bodies share the original scoped aggregate, conditional and
projection lowering. A three-state fold covers their original Boolean truth;
an optional atom and its negation remain optional. Actual costs still test the
original model through the objective query, with weight, priority and complete
tuple resolved from one binding. Source activity neither rewrites that model's
theory nor evaluates its frozen reduct.

An independently established absent producer contributes no objective row or
redundant zero-cost priority slot. For example, a required `a` makes the proposed
`n(0)` impossible in `n(N):-N=#count{1:a}`; a downstream observer of that absent
row need not publish its priority. This can shorten the raw cost vector while
preserving costs aligned by priority, the complete answer family and optimum
ties. Other conservative carriers still retain zero slots when the abstraction
does not establish absence.

The semantic bridge requires more than an empty delta. Complete typed rule and
value generation must ensure that restricting an original model to any closed
support carrier preserves its frozen reduct. Answer-set minimality then rules
out atoms outside that carrier. [SourceSupport](../lean/theorems.md) and the
[proof guide](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/source-support.md)
state this premise explicitly; successful Rust completion is not a proof of the
source-to-reduct correspondence.

Ordinary normal rules with at least two flat positive body atoms can reuse
their [matched rows](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_support/witnesses.rs).
The join lends the exact relation occurrences and identifies the prefix left
unchanged by its last advance. A prepared head projects arguments from those
rows into the existing canonical atom store. Repeated variables, constants and
strong negation keep their original meaning. The same projection serves
possible-support discovery and formula emission; the latter still visits every
producer, even when its head is already known.

The [formula consumer](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_ground/producer.rs)
constructs atomic formulas and balanced groups of two or three conjuncts only
after a complete selected binding exists. Unchanged prefixes retain their
subformulas. Eligible non-prefix groups of two or three atomic witnesses also
retain reached ordered row-position tuples and their canonical formula IDs.
Each map belongs to one group with fixed source occurrences in one immutable
join snapshot; row positions from another owner cannot identify its atoms.
A hit reuses the existing subformula. A miss constructs it through the same
canonical node index. No possible row product is precomputed, and every complete
producer still publishes its implication, root and producer metadata.

Cursor and group metadata are linear in body width. The maps additionally retain
space linear in the number of distinct reached subtuples across eligible groups.
A sorted map with `m` entries takes logarithmic lookup work and can shift `m`
entries on insertion, giving quadratic total movement in the worst insertion
order. Key reads, searches, movement, retained capacity and replacement peaks
consume the existing work and storage allowances; reuse still checks
cancellation. Arithmetic, default negation and local generators retain their
existing construction path.

These are representation and execution changes, not an inference that possible
support is true. All producer conditions and source origins remain represented.
Finite conjunction and frozen-reduct laws justify regrouping; the join's exact
row identity and complete producer coverage are separate implementation
obligations. Semantic differential tests check both original truth and frozen
truth for arbitrary interpretations. They do not establish a source-to-Rust
formal refinement.

Formula nodes use an exact-key index with the shared deterministic word hash. The separate
node sequence establishes dense IDs and output order; hash-table iteration never
participates in formula construction. Complete-key equality preserves node
identity under hash collisions. Collision probes consume the work allowance,
and node ceilings are checked before a new ID is published.

Completed formula theories always include double-negated necessary support
guards. These guards strengthen candidate checks while preserving reduct subset
freedom; they are mathematically redundant, not a selectable construction option.
Construction uses one
[`Metadata` owner](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_ground/metadata.rs)
with flat atom headers and shared append arenas. Each header identifies its
producer sequence, ordered source locations and first atom occurrence. Producer
links preserve insertion order and repetitions. Origin links preserve full source
identity and span order, with duplicates removed when encountered. Collecting
metadata creates no formula nodes early. The final support stage uses complete
producer sequences from this same owner.

Atoms with one recorded nonconstant producer can share necessary conditions.
For each such body, the compiler reads only its direct conjunction children;
other formulas stay opaque singleton conditions. It groups exact child-node
identities and replaces the selected family of guards
`not not (h -> AND conditions)` by guards
`not not ((OR requiring heads) -> condition)`. Both families require every
incidence's true head to have a true condition in the original interpretation.
Double negation freezes precisely that predicate for every tested interpretation,
so the conjunction of roots has the same original and arbitrary frozen truth.
Ordinary roots and shared body prefixes are unchanged. No positive support is
created by the guard disjunction. Multiple recorded producers, even duplicates,
and constant or missing producers retain their per-atom guard construction.

A deterministic shape check precedes incidence-row and formula publication. It
requires fewer estimated guard nodes, no more logical child occurrences and no
more roots; these estimates precede canonical interning and do not promise less
work for every input. Before allocating the directory, a charged catalog scan
counts eligible heads and direct incidences. Empty conjunction rows decline
sharing, and a necessary operand bound rejects families that cannot fit the
existing guard price even with perfect sharing. This precheck does not detect all
unprofitable families. They use the existing guard path after charging their
planning work. Planning can exhaust work or live workspace on a program admitted
by the original path; the compiler reports that typed refusal without refunding
work or hiding cancellation.

The shared plan uses a directory over the fixed node prefix, ordered group headers
and one complete incidence arena. Directory entries explicitly distinguish absent
groups from present indices in one machine word. The coordinate passes take
O(P + A + I) visits and O(P + I) temporary cells, for P prefix nodes, A emitted
atoms and I direct incidences. Semantic node lookup and evidence union add their
own charged comparisons, movement, copies and temporary evidence. New buffers use
the existing live workspace allowance, including growth overlap. Repeated
incidences are visited and charged; retained native operands count against the
ordinary operand ceiling.

Unselected guards keep their atom order; shared groups follow in first-incidence
order. Each shared root retains the sorted union of all participating heads'
producer sites and first occurrences. Root identity and multiplicity can change,
while every original source site remains represented. Counting, filling or
publication failure returns an admission refusal, never a partial theory.

This removes each atom's intermediate heap buffers. It adds a link word per
entry, and origin insertion can still inspect all locations associated with an
atom. The final public `formula_origins()` view owns a vector per root. Once root
and origin limits admit a guard, its ordered locations are copied directly into
that final vector. The shared arena remains live during this output assembly;
lower peak memory or faster solving does not follow from allocation count alone.
Grounding work includes shared arena relocation, origin comparisons and inserts,
producer traversal, and the final evidence traversal and copy. Allocation failure
and either origin ceiling remain located admission failures, never UNSAT.


### Optional final-rule domain guards

Eager formula preparation can request a domain attempt through the library's
`with_domain_analysis` method. The library leaves it off and the ordinary
command requests it, independently of `Indexed` or `Table`; it changes the
rows possible-support completion and final instantiation read, never the
original formula/reduct semantics. The source guide provides a
[checked on/off example](../rust/source.md#optional-domains-during-final-instantiation).

The private applicability check covers the exact normalized whole source and
its original positive flat rule occurrences, body comparisons included. It
excludes computed or generated terms in rule atoms, negative rule-body literals,
structural/local scopes in rules and richer producers; a favorable dependency
projection cannot qualify. The analyzer borrows that
same immutable Program until final instantiation ends. Normalized statement
deduplication does not merge the rule occurrences or their provenance.

Objective declarations may accompany those rules. They rank answer sets and do
not produce argument values, so the domain analysis ignores their restrictions
while retaining the complete normalized source. Guards narrow only rule joins;
objective grounding, arithmetic checks, tuple identity and scoring keep their
existing paths. This permission does not extend the profile to choice or
aggregate producers. Producer scheduling retains its separate applicability
conditions.

The source analyzer intersects the positive argument domains binding one head
variable within each producer, then unions the results of alternative producers.
For `p(X) :- a(X), b(X).`, the bound on `p/1` is therefore the intersection of
the bounds on `a/1` and `b/1`. This information propagates to downstream rules.
Unknown is unrestricted: another finite input can still bound the conjunction.
Inflationary passes continue to a fixed point, including recursive dependencies;
a stopped analysis publishes no partial finite bounds. Argument domains do not
assert correlations between different columns of a relation.

Every complete binding must belong to the upper domain of each mandatory
positive argument. Intersecting those domains for one source variable remains
necessary, including repeated occurrences. A comparison that reads one
variable alone is decided on that variable's value, so every value it is
defined and false at is removed from the candidates as well: the candidates
that remain are exactly the values the exclusion rule leaves, decided before
any row is read. A value the comparison cannot evaluate stays a candidate, so
the join reaches it and refuses as the language reference requires. The candidates
are prepared once per rule, with the analysis; a guard resolves them into a
snapshot's dictionary, for every completion round and the final one, and
only where the candidates are fewer than the argument's domain, since a
relation offers no value outside it. Unknown contributes no
restriction. A global Unknown/Stopped analysis or an inapplicable program
keeps complete fallback. These are upper bounds on source bindings, not facts
about candidate truth or answer-set membership.

The guard builder retains borrowed source symbols for the meets, converts one
atomic value at a time through the existing compiler, and resolves it through
the completed support owner's sole equality dictionary. It retains only IDs and
original rule/row/column ownership. Both join strategies offer their original
rows to the same guard before binding and deeper probes. An Indexed posting can
include rows that fail another bound column; Table intersects its equalities
before offering rows. Thus offered-row and guard-rejection counts need not
match across strategies, even when complete bindings do.

`DomainProducers.transfer_monotone` and `derivation_covered` state the abstract
producer contract: enlarging inputs cannot shrink proposals, and every finite
producer derivation belongs to a closed upper-bound assignment. Concrete source
extraction and completed Rust iteration remain correspondence obligations.
`DomainBindings.complete_binding_survives` states the necessary-meet law under
explicit argument coverage, and `kept_binding_survives` the narrowed-candidate
law for the bindings the exclusion rule keeps. `guarded_continuations_exact` preserves the ordered
complete result list, allowing a locally matching row with no complete
continuation to disappear. The [domain-binding guide](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/domain-bindings.md)
keeps analyzer soundness, source/IR correspondence, dictionary identity and
recursive Rust enumeration as separate obligations. The laws are not an
end-to-end proof of the source grounder or its resource failures. Arithmetic
validation stays on the existing complete path for the excluded profiles.

Applicability, analysis, bridge and guard work consume the remaining cumulative
formula budget. Analysis has separate finite logical populations and bounded
standard allocations, outside the named support/guard byte allowance and
without a new caller-control API. Rule guards account their named scratch and
actual capacities alongside live support/table/query owners; failed work and
observed capacity remain in receipts. Optional analysis may cost more work or
storage than it saves. Its observations establish activity and completion scope,
not elapsed-time, memory or scalability improvement.

### Terminal definition analysis

The domain library separately recognizes positive definitions whose predicates
are never read by a logical rule or constraint. Every producer of a selected
predicate must qualify; complementary strong negation blocks selection because
coherence introduces a dependency. This is a structural property, independent
of predicate names, constants, filenames and `#show` declarations. Choices may
remain in the base program. Their possible atoms are an upper bound, even when
an exact-one rule selects a single value in each answer set.

For this one-layer class, the
[`TerminalDefinitions` laws](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/TerminalDefinitions.lean)
give each base answer set a unique extension: add all heads whose positive
bodies hold in that answer. The
[`terminal::analyze` API](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-domain/src/terminal.rs)
classifies the source. Adaptive formula admission then checks complete
source-to-IR correspondence before removing any producer from the base.
Every producer must match a complete flat positive source rule, including its
typed constants and one consistent variable mapping, and every definition must
be matched by some producer. A producer's parsed origin nominates the
definition at the same place among that origin's definitions, and only a
miss falls back to trying every definition; a definition left unmatched is
tried against every producer. The certificate is the same as checking every
pair, at a cost linear in producers and definitions when compilation keeps
their order, and a shared origin alone never establishes a match. A
dependency projection cannot supply this certificate. Objectives and explicit
projection currently exclude this schedule; ordinary eager materialization
remains available.

After correspondence is established, a physical policy defers a complete group
of producers sharing predicate name, arity and sign only if at least one lowered
producer has a variable. Body-only and anonymous variables count. All producers
of that signed signature then defer together, including ground facts and rules.
Ground-only groups remain in the base, even when their rules have nonempty
positive bodies. The domain classification remains mathematically general; this
policy chooses a subset for reconstruction. The remaining source is analyzed as
the actual base. If no group is selected, ordinary materialization retains the
original preparation and its cumulative charges.

```text
prepare original source and certify terminal producer groups
select complete groups with at least one variable-bearing producer
ground the base using the original remaining allowance
close its canonical storage without retaining possible-support indexes
for each answer verified against the base reduct:
    select exactly its true base rows
    join true rows for each terminal definition
    union the resulting heads with the base answer
    publish the full answer, then evaluate its observations
```

The [source bridge](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_terminal.rs)
retains the original program separately from the base theory and its analysis.
Reconstruction shares immutable term and tuple storage, but starts with a fresh
truth selection for every answer. Its joins use the shared whole-argument matcher
and borrowed typed keys. It neither imports a second value universe nor treats
possible support as truth. Duplicate witnesses coalesce only at head publication.
The base selection authenticates the original closed catalog once and discovers
its existing IDs in the model's semantic order, using the ordinary discovery
indexes without repeated tuple interning or semantic lookup. Only selected base
atoms become discovered; every later derived head still uses ordinary insertion.
The first reconstruction prepares the retained head and body patterns, in their
original order. Later answers reuse those patterns while initializing their own
bindings and row cursors. Preparation is charged once; its retained capacity is
counted on every call. A failed preparation publishes no partial plan.

The automatic formula route chooses this schedule over an eager base, and the
lazy formula route over a hybrid base, whose eligible constraints are streamed
and checked before an answer is reconstructed. Explicit eager grounding still
materializes the complete theory. Base membership uses the selected CPU or GPU executor. Reconstruction
currently runs on the host. Each reconstruction starts from source admission's
account under a per-answer allowance of work and substitutions — the headroom
the formula ceilings left after grounding — whatever earlier answers used: the
number of answers is bounded by enumeration limits, time and cancellation, not
by a cumulative grounding ceiling. Session totals are reported. A refused extension
publishes no original answer and cannot establish exhausted enumeration.

### Relation rows and vector operations

A relation row is one complete typed tuple. Formula support's
[`SupportCatalog`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_support/relations.rs)
owns each possible atom once, with append-only, predicate-local row identities.
A checked AVL index of those identities supports membership checks without
another atom collection. The core relation owner retains its equality dictionary
and argument columns across growth rounds. A dictionary entry refers to one
whole canonical term; it does not copy that term. Each column has a
`BTreeMap<u32, Vec<usize>>` from dictionary IDs to original row positions;
insertion extends only the new row's postings. An immutable snapshot borrows
these existing columns and postings. Row and equality IDs survive append, while
queries remain bound to one particular immutable view. Numeric ID order is not
ASP term order.

Round publication opens one checked catalog append session for each contiguous
predicate run. The session reuses the one-row transaction's equality IDs and
tentative dictionary patches; scalar insertion uses the same transaction engine.
It lends each newly committed row's equality IDs directly to posting insertion,
without rebuilding a relation view. A duplicate keeps its original row and adds
no posting. Each row still admits all comparisons, reservations and publication
work before its indivisible commit. Earlier committed rows survive a later
refusal; the support build then fails, without claiming completed support.
Session capacities are charged separately from retained catalog storage,
including actual growth overlap, and are released at the end of the run.
Reusing these allocations removes repeated preparation, not dictionary searches
or per-row index maintenance.

An insertion beyond the relation's current greatest tuple reuses the published
rightmost index path. Each row still undergoes the typed maximum comparison;
numeric IDs do not establish this order. The relation and atom interner share
the same checked path mechanism. Only changed links and balances are published,
with rotation and path-refresh work admitted before the row commits. Interior
insertions or failed path preparation invalidate the retained path. This reuses
the existing mutation buffer without another tuple collection or lookup index.

The append dictionary also retains a sparse inverse from canonical term identity
to local equality ID. Tuple insertion and equality queries share that lookup.
It authenticates the term's vocabulary and readable prefix before probing the
inverse. A hit returns the established local ID; an eligible miss proves absence
from this relation's dictionary, not absence from the vocabulary or falsity.
Foreign and ingress terms use typed comparison, as do valid query terms outside
the supplied canonical prefix. The relation reader must still cover every
retained row. Static sorted dictionaries retain their own immutable lookup.

Support keeps posting lists only for the columns a join can bind: a column
holding a constant, or a variable that occurs again in the same rule, body or
head (factorization binds head variables before it probes the body), in some
join occurrence of its predicate. The rule is syntactic, and keeps a superset of
the postings its two readers consult — probes, which choose the shortest posting
among the columns a pattern binds, and totality domains, which read a column's
keys. For rules with an ordinary head and a flat body of atoms, comparisons and
bindings it keeps no column that holds only a variable occurring once, and no
column of a negated atom, which is decided by exact lookup once safety has bound
its variables (those variables still count as occurrences for the other
columns). It still keeps one kind no reader consults there: columns whose
variable's only other occurrence consumes it (in a comparison or the head)
without binding it first. A choice or conditional head, a projection and every
nested frame index every column they name. A predicate no join reads, such as a
terminal definition under eager grounding or a predicate a flat rule reads only
under negation, keeps none, and neither does a column of a flat rule holding a
variable that occurs once. The demand is
computed from the compiled rules before support grows, and its storage counts
toward the support byte ceiling once, separately from the relation indexes that
grow during publication. A column without postings is absent, not
empty: a probe still resolves its equality and narrows by its other columns, and
a totality certificate declines it as a domain of distinct values.

For a positive witness, the selector resolves known whole-column equalities and
chooses the shortest posting list. Equal-length lists retain the first known
column's list. The matcher then checks the complete tuple in original row order,
including repeated variables and structured terms. Without a known equality,
the selector offers all relation rows; a missing bound key selects none:

```text
rows = ShortestPosting(relation, KnownEqualities(pattern, binding))
bindings = FilterMap(MatchWholeTuple(pattern, binding), rows)
```

A structural argument such as `f(X,g(Y))` supplies an exact equality when all
its named variables are bound and it contains no anonymous position. The cursor
resolves its constructors through the canonical vocabulary's existing lookup,
including name, sign, arity and ordered children. It then uses the same
whole-column posting as an already bound flat argument. A missing constructor
identity selects no rows from that snapshot. Partial arguments and anonymous
positions retain the ordinary matcher; one exact argument can restrict a tuple
whose other arguments remain partial. The full matcher still checks every
offered row.

These queries admit no terms and change no incoming binding. Append-side queries
borrow a fresh lookup from the writer; completed support uses its retained
read-only lookup. Neither retains negative results across writer growth. A
cursor reuses leased term-ID frames and child-index scratch, clearing provisional
IDs after every result. Existing support-byte and work limits cover this scratch,
the temporary append lookup header and the lookup's local child capacity.
Refusal and cancellation remain typed failures. Eliminated matcher visits are
no longer charged; the additional exact-query work is charged where performed.

A cursor also retains at most one successful resolution per actual structured
source argument. Its immutable argument nodes name the input slots; one scoped
term-ID frame stores the last complete inputs and result. Reuse requires every
current input to be present and canonically identical. The fresh whole-capture
slot addresses the record directly, and the record checks the actual source
argument; ambiguous capture coordinates in manually assembled IR decline reuse.
No absence, relation row or logical activation is cached.

These records belong to one join, its source plan and its fixed support snapshot.
The append vocabulary grows monotonically; the ordinary checked argument view
still validates the result against the current reader prefix. Backtracking owns
the incoming slots and does not publish retained results into them. A changed
input invalidates the record before replacement, and a complete successful
replacement alone makes it reusable. Refusal leaves no partial valid record.
Preparation, dependency checks, ID transfers and retained capacity are charged;
only the constructor reconstruction and lookup actually avoided are omitted.

Each cursor resolves a positive occurrence to its borrowed snapshot relation on
first entry and retains that reference across backtracking. Row access and later
probes use the same immutable row owner, without searching the predicate directory
again. References remain local to the cursor's snapshot; a new support round
resolves its own relations, including predicates newly inserted in the directory.

The indexed selector folds known equalities directly into its shortest posting.
It uses the core relation's checked single-equality resolver, shared with owned
`Query` construction. Flat probes need no temporary key or equality vectors;
structural probes supply their resolved argument frame. Within this resolver,
every later variable slot and column is still checked after a missing value;
interruption returns no partial posting. Per-step work admission and completed
failure prefixes remain observable. Owned public queries retain their existing
frame and equality-vector capacity receipts. The cursor's resolved-reference
capacity is charged with its other retained join storage.

This describes witness selection, not complete grounding. Scope, generators,
scalar guards and candidate gates retain their separate contracts. The
[bounded posting diagnostic](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/README.md#inspect-posting-selectivity)
compares intersections with an independent full-row scan. It is compiled only
for tests and does not replace production selection. Its counters measure
selectivity; integer comparisons and complete tuple probes have different costs.

The catalog cannot grow while its snapshot is borrowed. Once a round finishes,
the snapshot drops before new atoms are appended. A failed tuple or posting
extension returns no usable support owner. The completed final snapshot supplies
formula emission and objective eligibility. The core catalog uses the shared
checked AVL implementation for row order, new dictionary-value placement and
foreign-value lookup. Nodes contain only IDs and links; typed comparisons inspect
their actual descriptor/text prefixes. An insertion resolves existing values
through the inverse, and plans genuinely new dictionary leaves in a bounded
metadata overlay. Repeated new values share the same tentative local ID.
Row membership, dictionary representatives, inverse entries and columns publish
together after all fallible checks. No historical sorted row or dictionary
sequence is shifted. If a tuple
introduces `a` new values into a dictionary of size `d`, tentative patches occupy
O(a log d) cells; the checked overlay lookups can use O(a² log² d) metadata work.
Typed comparisons, node inspection, append copies and posting construction
consume the grounding work budget. Inverse lookup uses fixed integer keys with
expected constant-time hash lookup; internal hash probes are not individually
cancellable. Reservations, relocation and publication are admitted before they
run. Named storage includes addressable key/value capacity and conservative
growth overlap, excluding opaque hash-control and allocator storage. It is not
a process-memory bound. Snapshot construction visits predicates without
revisiting their rows.

Scalar reduct closure uses the same per-predicate catalog. Before each round it
prepares a complete ordered ID view for each changed extent and reuses that view
for the round's joins. The view is a stack of sorted runs of row
IDs. The first preparation of a relation traverses its O(n) row IDs into one
run. A later preparation promotes the previous run to a level, merges the top
two levels while the newer is at least half the older, so the levels shrink
geometrically and number O(log n), and sorts the `d` rows appended since into
a new run. A merge of two runs costs their combined length in charged
comparisons and copies, and each row is merged O(log n) times over a whole
derivation, so the views cost O(n log n) charged work in all and no
preparation copies the extent; a bound-prefix window is one binary search per
run. The levels are the rows present before the last appending preparation and
the run is what it added, both borrowable until the next one. Row access
within a run is constant time and borrows the authoritative tuple. A duplicate
or refused insertion preserves an existing prepared extent; a successful
append invalidates it.

A predicate whose every argument is bounded is held as a dense relation
instead of a catalog. Preparation infers an upper bound on each argument's
values over the admitted templates: a constant in a head contributes itself, a
head variable ranges within the intersection of the bounds of the positive
body positions binding it, gates and filters bind nothing, and the least
fixed point is finite because every value is a constant of the program. An
argument wider than the ceiling is unknown, and unknown absorbs. Where every
argument of a predicate is bounded and the product of the widths fits
`PreparationLimits::max_dense_atoms`, the relation is a bit array over the
mixed-radix index of the arguments' ranks, the first argument most
significant, each argument's values kept in canonical order so that position
order is canonical atom order. Membership is a bit test, insertion a bit set,
and the rows matching a bound prefix are one contiguous range of positions,
so a window is a scan of that range's words rather than two binary searches;
no atom is allocated or compared by value until the model is assembled, and
the model is read off the bits in order without sorting. The New rows of a
round are a second bit array cleared when the cutoff advances, over the words
the round touched, so an unchanged relation costs a round one unit whatever
its size; Old is present and not new. New-row scans intersect their prefix
window with that touched-word range: outer words are known zero. Scalar and
block scans retain the original tuple positions and body-to-head offsets;
this adds no storage. A round records a derived head of a
dense relation as a pending bit: the key is ranked once, the position is
tested against the relation, and an absent position is marked in a row of
words the closure workspace keeps for the layout, beside the catalogs the
round's joins borrow. The marks are disjoint from the relation, so their
number is the round's count of new dense atoms and the derived-atom limit is
judged as each is marked. After the round the marked words are joined into
the relation and into New, and cleared, so nothing of a round or a candidate
remains in the pending marks; every dense relation is created when the closure
starts, so that a round never changes the catalogs. `Statistics::dense_heads`
counts the heads recorded this way. Where a rule's innermost occurrence is
over a dense relation, its last argument is a variable that nothing else in
the rule mentions but the head, as the head's own last argument, every other
argument of the two patterns is a constant or bound by then, and the two
relations list that argument's values alike, the join does not bind the
occurrence's rows one by one: the rows matching the bound prefix are one
block of the relation, their heads are one block of the head's relation,
place for place, and the block of rows is joined into the head's pending marks
a word at a time, leaving out the positions the head holds or the round has
marked. No gate or filter reads the variable, so every guard was judged
before the depth was reached, and each row of the block is one binding of
the rule; the new marks are counted against the derived-atom limit. This is
the row operation of a transitive closure over a boolean matrix. The plan is
fixed at preparation for every occurrence, since the occurrence a round
visits innermost depends on the one it pivots on; a rule that fails a clause
keeps the binding of single rows, as does a join that follows membership.
`Statistics::block_steps` counts the blocks joined, whose rows are counted as
bindings and not as tuple probes. The bounds are an upper domain of every
derivable head, so a head outside them is an admitted-program invariant
violation, not a missed row, and the closure over dense relations holds
exactly the atoms the closure over catalogs would, step for step; the family
tests check this atom for atom with dense relations enabled and disabled.
Whether a predicate is laid out depends on its bounds and the ceiling alone,
never on how many tuples it holds: a relation sparse in a wide box is dense
all the same, at the cost of its words, and the ceiling is the one control.
It is `PreparationLimits::max_dense_atoms`, 16,777,216 positions by default,
and a session takes that default: nothing outside the library sets it.
Every other predicate keeps its catalog, and the preparation receipt reports
how many predicates were laid out.

Scalar closure first visits every template against empty derived truth, including
facts, zero-positive rules and constraints under the frozen candidate. In each
later round, a binding is visited at its first source occurrence containing a new
row: earlier occurrences select Old rows, that occurrence selects New rows, and
later occurrences select Current rows. These disjoint choices preserve repeated
predicates and source occurrences. The new occurrence is joined first, so its
few rows bind the variables and every other occurrence is entered through a
bound-prefix window, and only templates whose body names a predicate with new
rows are visited at all, from an index prepared once per program; a round
therefore costs the new rows times their joins, never a scan of an unchanged
relation or a visit to a rule that cannot bind. Newness uses stable per-predicate insertion
IDs, never canonical ranks, which can move when a smaller tuple is appended.
For mixed extents, the Old rows are the catalog's levels and the New rows its
newest run, each run in canonical order and each sharing the sole tuple payload
owner. The round cutoff advances after the round's joins and before its heads
are appended, so the levels of the next preparation hold exactly the Old
extent; the partition keeps only that cutoff and checks it against the runs.
All-old and all-new extents reuse every run or none.

A round completes every selected binding and constraint before advancing its
frontier and publishing pending heads. Frozen gates and pure equality filters
preserve the eligibility of old positive bindings. Their consequences are
already present, and earlier constraint triggers remain latched. Together with
bootstrap and completed earlier scans, the final no-change round establishes
complete source coverage. The final gate-carrier comparison still checks both
directions, even for a rejected candidate. Failure returns no partial closure
and discards dirty workspace history. Public `source::scan` and shared-world
source traversal retain their complete ordered scans; this scalar schedule does
not change the answer-set definition or claim device delta execution.

[`DeltaRounds`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/DeltaRounds.lean)
separates three mathematical obligations: first-new partitions cover each new
binding, old enabled heads are already in current truth, and the retained
constraint latch equals the old family's triggers. The
[reading guide](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/delta-rounds.md)
maps those premises to scalar catalog cutoffs, complete bootstrap and round
publication. These laws preserve a consequence step and constraint verdict;
they do not prove Rust row decoding, interruption handling, a termination bound
or answer-set membership. Those require their own representation and completed
execution arguments.

Completed scalar `Statistics::tuple_probes` counts source rows offered to the
whole-row matcher, including rejected rows. It excludes prefix-search comparisons
and catalog membership operations. Each probe belongs to a charged join-loop
step. `work` also includes canonical and delta-view preparation, initialization,
ID reads and writes, pivot checks, pending publication and final completion.
Fewer emitted bindings alone do not establish fewer probes or less total work;
none of these counters establishes an elapsed-time gain.

`zetesis_cpu::PreparedQueries` inspects one exact admitted `Program` once to
bound the assignment, cursor and undo buffers used by its joins, to infer its
argument bounds and to choose its dense layouts, which every candidate's
closure shares and admits first. A ground head's complete ordered tuple has a
fixed position in its dense layout. Preparation retains this position so later
firings need not rank its arguments again. Each candidate still visits the
source, checks its guards and marks its own consequences; variable and tree
heads keep their existing operations. The optional coordinate per template
adds linear storage, admitted as preparation bytes. Preparation work and bytes
have independent finite limits and a separate receipt. A
`ClosureWorkspace` retains one Program-scoped canonical tuple authority and
reusable relation, cursor/undo and old/new ID metadata between candidates.
Assignments borrow terms only within one immutable round. Per-predicate catalogs
hold membership and equality/order metadata over that authority; dense relations
hold coordinate bits over the same frozen Program vocabulary.

Final assembly publishes the selected discovery positions in semantic order,
then constructs the result `Model` without exporting owned atoms. The workspace
retains discovered identities and capacity, while resetting relation membership,
frontiers, dense truth and pending marks. No candidate truth survives completion.
Previously returned models keep their immutable prefixes. Any failed check
discards dirty workspace state before reuse. A different `PreparedQueries`
owner retires the old workspace, even for the same Program: its inferred axes
and block plans may differ. The shared immutable layout owner authenticates
that boundary. Its separately allocated header is included in preparation bytes.

The one-shot `check_view` uses the same evaluator and charges preparation plus
candidate work to its existing cumulative work limit. Reused preparation has a
separate work receipt, so exact resource cutoffs can differ while completed
closures, constraints and seed checks agree. Retained capacities are admitted
under each prepared call's current limits, including an empty program. The
one-shot empty-program path constructs no preparation or workspace and retains
its vacuous zero-round result. This removes repeated allocation and dimension
inspection; it does not establish a timing gain. Both entry points use the
same scalar delta schedule described above.

`BatchOracle` retains that preparation and a bounded set of workspaces across
independent batches. Its indexed borrowed seed producer is divided into at most
one contiguous range per configured worker. Each range has one exclusive
workspace and its candidates run sequentially; range tasks may be stolen by
Rayon. Result order and candidate occurrences are preserved without materializing
seed views. This schedule bounds owners directly, independently of worker thread
identities. It can balance uneven candidate costs differently from per-candidate
work stealing. Shared-round and static execution keep their separate algorithms
and the same nonblocking batch admission slot.

Batch preparation uses `PreparationLimits`; a preparation stop is distinct from
an individual candidate stop. `query_statistics` reports completed preparation
builds and adoptions, assigned slots retained from earlier submissions, actual
cached capacity and the latest admitted collective envelope. The envelope counts
the separately allocated preparation header once, all idle workspace capacity, and each assigned
workspace's maximum of retained capacity and its remaining per-closure allowance.
A workspace's per-closure allowance already counts the prepared queries' retained
bytes, so those bytes are subtracted before combining owners. Actual unused workspace-vector
capacity and conservative replacement overlap are included. Lowering a limit
can refuse already-retained capacity; even an empty batch checks an existing
cache, while an empty batch never creates preparation. Final result retention,
source payload, allocator/tree overhead and worker stacks remain separate. These
receipts describe bounded ownership and reuse, not timing or process RSS.

In an independent lazy CPU session, optional candidate narrowing supplies its
completed immutable `PreparedQueries` to the mandatory oracle. The oracle adopts
that exact owner only when the dense-layout policy agrees, the recorded work fits
its preparation limit, and construction completed under no wider byte allowance.
Otherwise it prepares the program through its ordinary path. Narrowing and the
oracle retain separate mutable workspaces; shared queries contain no candidate
truth or membership verdict. An optional resource-limit refusal permits ordinary
mandatory checking; cancellation and deadlines still stop the session. Empty
batches adopt nothing. `preparation_adoptions` counts adoption events; reusing
the currently retained owner adds none. `preparation_builds` counts construction
by the oracle itself.
The preparation receipt still reports the original construction work. This reuse
does not change eager, shared-round or device execution.

`zetesis_cpu::Limits::max_closure_bytes` bounds each scalar closure's canonical
authority, relation indexes/columns and prepared-order runs, dense words, pending
discovery IDs/marks, query assignment/cursor/undo capacities, and operation scratch
with conservative growth overlap. The authority counts canonical payload once;
final catalog and model-selection metadata are admitted while being assembled.
Its default is 128 MiB; zero is a zero-byte allowance. `peak_closure_bytes` is the
maximum observed named envelope of a completed check. Relation capacities are
summed as each relation is created, grows or is cleared, so reading the envelope
does not visit every relation; debug builds recompute the sum on each read.
Earlier results retained by the caller, allocator bookkeeping and Arc counters
remain outside this per-check ledger. A successful allocation can exceed its
proposed reservation before the actual-capacity check refuses it. A stopped
scalar check returns no partial `Check` or statistics. Collective worker
admission and result retention have separate owners; this is not a total
process-memory bound.

`FormulaLimits::max_support_bytes` bounds one evolving canonical support authority
and its current prefix, relation/equality metadata, postings, borrowed snapshot
objects and query capacity, including scratch and named growth overlap. Terms
and atoms are counted by that authority once; predicate relations and pending
rounds retain scoped IDs. Allocator/tree/control-runtime overhead and unrelated
grounding state remain separate. The independent cumulative source-expansion
budget still applies, and this support limit is not RSS.

Ordinary execution derives this support capacity from the shared `--memory`
allowance. Zero memory supplies zero capacity. The library's explicit
`FormulaLimits` remains available for callers that need to bound individual
operations. Work statistics retain their meaning independently of that policy.

Row identity connects relational semantics to masks, intersections and gathers.
Combining two column masks means intersecting positions in the same relation
snapshot; it must not combine values from different tuples. The bounded
[`relation` library](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-core/src/relation.rs)
provides the immutable column view used by eager formula support. It borrows canonical `AtomRef` rows or construction descriptions,
then encodes complete typed values through a dictionary of borrowed references.
It preserves row occurrences and their order, including duplicate tuples, and
keeps original catalog indices distinct from local positions. Explicit predicate
arity and row count distinguish an empty relation from a nullary tuple.

The equality columns use 8-, 16- or 32-bit cells behind one borrowed `Column`
view. Decoding returns the same `u32` dictionary identifier; it neither renumbers
values nor changes typed term order. Immutable construction chooses a sufficient
width from its dictionary. An appendable catalog widens a column before a new
identifier is published, charging the copied cells and simultaneous old/new
storage. A later refusal can retain that wider capacity while preserving every
published row. Clearing a catalog retains its reusable capacity. There is no
retained unpacked copy. Headers, indexes and allocator capacity still count, so
narrower cells alone do not establish a smaller complete owner or faster query.

The lazy source grounder retains its candidate-specific relation and world-mask
contracts; this eager support representation does not make a possible atom true.
Queries and selections borrow their exact relation owner. A selection validates
ordered positions; it does not establish complete grounding or answer-set
membership. Equality filtering is complete relative to its supplied input rows:

```text
query = ResolveEqualities(relation, known_values)
selected = Filter(AllEqualitiesHold(query), supplied_rows)
bindings = FilterMap(MatchWholeTuple(pattern, binding), selected)
```

This conjunction-filter primitive does not replace the eager grounder's shortest
posting policy. Intersecting every posting would skip additional budgeted matcher
visits and value extraction, which can change checked-failure boundaries. Such a
consumer must establish preservation of successful bindings and typed failures
for its own matching operation.

The same equality predicate can produce ordered positions or packed row bits.
[`Relation::select_mask`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-core/src/relation/selection.rs)
writes matching input positions directly into their mask words. This avoids a
position-vector intermediate when the consumer requires masks. Decoding the
result over original row order yields the same selected occurrences; distinct
rows containing equal tuples keep distinct bits. The packed output still spans
the original relation, so sparse inputs do not imply small mask storage.

An empty conjunction retains every supplied row; a missing dictionary value
retains none. Numeric ID order does not implement numeric comparison, ASP term
order or arithmetic. The existing matcher remains responsible for structural
terms, repeated variables and checked binding generation. The
[column laws](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/column-relations.md)
state the reconstruction and ordered-selection arguments and their limits.
The eager support catalog uses this representation as described above. Adoption
by other grounding consumers requires the same coherent tuple ownership and
removal of redundant stores; an experimental copy alone establishes neither a
smaller memory footprint nor faster grounding.

The formula binding planner also derives finite integer envelopes from directed
affine comparisons when ordinary binding steps cannot advance. For an inequality
`a₁X₁ + … + aₙXₙ ≤ b`, it derives an endpoint for one variable from known
endpoints of the other terms. Each round reads the previous intervals, combines
simultaneous proposals by minimum or maximum, and installs only missing
endpoints. At most two endpoints per variable can be installed. This bounds the
analysis independently of numeric tightening or contradictory cycles.

```text
intervals = EmptyEndpoints(variables)
repeat at most 2 × Count(variables):
    proposals = ReduceBounds(Map(DeriveFrom(intervals), inequalities))
    additions = MissingEndpoints(intervals, proposals)
    if Empty(additions): stop
    intervals = Install(intervals, additions)
range = FirstFiniteUnboundRange(intervals, boundVariables)
return range
```

This is the finite-envelope fallback, not the complete source planner. Existing
joins and generators remain responsible for their own scopes and dependencies.
The scheduler consumes the returned range and resumes binding. Its cursor
enumerates the resulting plan and retains every original condition.
The [implementation](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_binding_guard/envelope.rs)
uses checked coefficients and bounds, charges analysis work and storage, and
retains the original scalar conditions. A conservative interval may propose
extra rows; it may not omit a satisfying substitution.

For `V` scoped variables, `C` inequalities and at most `E` expression nodes,
one fallback uses `O(EV + CV + V)` logical storage and at most `O(CV³)` endpoint
work. Existing binding steps run first, so already resolved scopes avoid this
analysis. These bounds describe the dense implementation, not the size of its
subsequent substitution family.

The distinctions matter to parallelism. A union of several worlds' relations
can offer shared source instances, but a join can combine atoms that coexist in
no individual world. Each world must therefore recheck positive truth. Optional
world-membership masks prune a prefix only if no current world satisfies it.
They do not prune future snapshots or the candidate carrier.

See [lazy checking](../rust/parallel.md) for the injected execution contract,
and [the theorem map](../lean/theorems.md) for the corresponding coverage laws.
