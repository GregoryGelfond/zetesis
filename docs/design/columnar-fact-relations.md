# Facts as columnar relation rows

A fact `p(t) ← ⊤` is a body-free ground rule, and zetesis has built general rule machinery for each one on both
routes. This note normalizes facts into the representation the system already has for data, the columnar,
dictionary-encoded support relations, in four stages. Each stage is an execution choice: answers and output are
unchanged, and so are admission verdicts, except where a binding ceiling is reached by charges a stage removes or adds.
The [grounding chapter](../book/architecture/grounding.md) and the
[semantic definitions](../book/architecture/semantics.md) describe the routes it changes.

Paths are under `crates/zetesis-themelios/src/` unless another root is named.

## Status

| Part | Status |
|---|---|
| Stage 1: the formula route stops building a rule per fact (fact blocks, typed `items()`, private rule storage) | Implemented |
| The owner record's geometric growth (`reserve_amortized`), found by the heap profile | Implemented |
| Stage 1's conforming change: interval segments, column-major runs, the cell encoding behind a row accessor | Implemented; acceptance pending |
| Stage 1's acceptance run on the reviewed commits | Owed |
| Stage 2: analysis inputs without a statement per fact | Implemented; combined acceptance pending |
| Stage 3a: the lazy route's core program keeps facts as tables, representation (A) | Implemented; combined acceptance pending |
| Stage 3b: the program's facts bound in the closure | Needs its own design |
| Stage 4: the ground program's facts as a fixed-true set | Needs its own design |

The [fact path note](fact-path.md) is complementary: its Parts 2 and 4 act before a statement is recognized as facts,
and Stage 3a reads fact values' domain positions through its Part 1 table. Its withdrawn Part 3 is this design's
Stage 1.

## Problem

### Memory and time per fact

Marginal peak resident set size per fact from 16,000 to 64,000 facts, against clingo 5.8.2:

| Shape | zetesis | clingo | Ratio |
|---|---|---|---|
| `bare`: plain unary facts, lazy route | 2.80 KiB | 0.24 KiB | 11.7× |
| `fcmp`: plain facts and one comparison rule, formula route | 5.75 KiB | 0.20 KiB | 28.8× |
| `frange`: one range fact `p(0..K-1)`, formula route | 3.14 KiB | 0.27 KiB | 11.7× |
| `wide`: facts of high arity, lazy route | 4.10 KiB | 0.34 KiB | 12.1× |

At this rate a million facts need 3–6 GiB where clingo needs a quarter of one. At 16,000 facts a plain fact costs about
4.6 µs on the lazy route and about 12 µs on the formula route, against under 2 µs for clingo's whole run: zetesis is
2.5× slower on lazy-route plain facts, 3.4× on lazy-route wide facts, 6.4× on `fcmp` and 9.6× on `frange`. For
data-heavy domains this is the class where clingo is clearly stronger, and representation, not search, decides it.

### What one fact became

On the formula route, before Stage 1, each fact was raised as a themelios statement (needed once, as source
authority), then rebuilt as an analyzed statement, compiled into a `RuleIr` with an empty body and its own origins
vector, given a key-owner entry, fed to `Analysis::of`, given a producer slot, visited in support round 1 through
`Join::variant_rule` (1.38 µs), interned as a support atom and row (0.87 µs, the one part a fact needs), joined again
by exact activity (0.90 µs), and on the grounding sub-route joined once more by final emission. Of about 12 µs per
fact, roughly 8 µs is work a body-free ground atom does not need. On the lazy route each fact is a core `Template` with
its own cloned origins, admitted into `zetesis_core::Program` and visited per template by oracle preparation, by its
bootstrap round and by its source scan.

### Routes

| Step | Lazy route | Formula, direct source evaluation | Formula, grounding sub-route |
|---|---|---|---|
| Stage 1: compilation, support completion | — | yes | yes |
| Stage 1: exact activity | — | yes | — |
| Stage 1: source eligibility, final emission, terminal partition | — | — | yes |
| Stage 2 | — | yes | yes, except where the terminal partition is eligible |
| Stage 3 | yes | — | — |
| Stage 4 | — | — | yes |

A closed function or tuple value is ordinary fact data. In the source representation, `Symbol::Function` and
`Symbol::Tuple` are closed values; a `SourceTerm::Symbolic` containing either is a scalar fact argument. Stage 3a
stores every closed structured fact accepted by the relational door as table data, including closed function/tuple
term forms that the strict door converts under its existing policy.

The formula route separately tests for constructor or operator **expressions** remaining after normalization:
`SourceTerm::Function`, `Tuple`, unary/binary operations and absolute-value expressions retain the generated-rule
path. A constructor containing an interval, such as the head of `p(f(1..3)).`, needs that scoped generation. This
expression test does not inspect inside a closed `Symbol` value and does not make a closed value such as `f(1)` a
generator. Arithmetic already evaluated to a closed value follows the fact path; unresolved expressions retain the
route's existing generation or refusal policy.

### What already exists

Each signed predicate's support relation is a `relation::Catalog` (`crates/zetesis-core/src/relation/catalog.rs`) with
unique row membership over the canonical atom authority, equality-identifier columns at 8, 16 or 32 bits, a
dictionary with a canonical-to-local inverse, and ordered runs. `SupportCatalog` keeps one per predicate with postings
for demanded columns (`formula_support/relations.rs`). Fact expansion already keeps a statement's facts compact as
products over columns of scalars and `i32` intervals (`fact_expansion.rs`); before Stage 1 it materialized them one
head at a time only because each head became a rule.

### Why columns compound

Facts held as typed columns are the input several other designs want:

1. **Integer column filters.** A fact column whose values are all integers has no declining row, so a comparison
   against constants can become a scan producing a row mask over `i32` values, decided before any per-row join. A later
   design.
2. **Numeric domains.** The [numeric argument domains](numeric-argument-domains.md) analysis reads the formula route's
   facts from the fact blocks once per preparation. After the conforming change a range fact is read as one segment.
3. **Body-atom identity** names a body literal's atom from the support row its join matched, independently of how rows
   are stored, so fact predicates need no special case ([body-atom identity](body-atom-identity.md)).
4. **Device layout.** Equality-identifier columns at 8, 16 or 32 bits are a layout a device buffer can take directly.
5. **Memory**, as measured above.

## Design

### The ordering rule

Every consumer that visits the prepared rules in order visits **items** in the same order. An item is a rule or a
**fact block**: one predicate's block of fact rows, standing exactly where those facts' rules stood, its rows in
expansion order (alternatives in order, last argument fastest). A statement with one argument alternative of one row
supplies a run row; every other argument alternative becomes a product. A fact block has one of two layouts:

- a **product**: the facts of one argument alternative of one source statement. A statement whose alternatives differ
  in predicate or arity, such as `p(1;a,b).`, contributes one block per product, so every block has one predicate, as
  the acyclic plan's key of component rank and position requires;
- a **run**: consecutive eligible fact statements of one predicate, the common instance-file shape. Its rows keep their
  order, but item-level counts change: grounding observers see one rule-instantiation phase per block, not per fact.

Wherever the code discovers, stages, interns or emits a fact rule's head, the block's row is discovered, staged,
interned or emitted at the same point. This one rule makes support publication, atom numbering, root order, model
enumeration and output identical, so stages are accepted on byte-identical output, not on equal answer families alone.
The type is `FactBlock` (`formula_facts.rs`), reached through `Item::Facts`.

### Representation

- **The fact arena.** Facts travel from expansion into both routes in one columnar form, in shared vectors reserved
  fallibly and leased like the compiler's other storage, with no record per fact. A product is a predicate admitted
  once, a range of columns and a row count; each column is a range of segments, each segment a scalar cell or an `i32`
  interval. On the formula route a scalar cell is an admitted occurrence coordinate in the preparation's
  `TemplateComponents`, which resolves to a canonical value; equal values can have different occurrence coordinates.
  On the planned lazy-route form it is the closed `Value` the core program interns.
- **Runs** store one column of cells per argument plus each row's evidence range, so a row of a unary run costs one
  cell and one range.
- **Families.** A family is the facts of one statement: a range of products and a range of evidence. Its origins are
  stored once, where each fact rule used to clone them. A family records its statement, and the retained source
  program holds that statement's carrier; a fact statement's normalized provenance is the stamp of that carrier, so it
  can be re-derived exactly without a copy per family.
- **Access.** `rows()` borrows a block. Its `Row::cells()` yields opaque, `Copy` scalar occurrence coordinates in
  argument order, hiding the stored segment or run-column encoding. `Scalar::get(TemplateComponentsRef<'source>,
  limits, counters, location)` resolves each coordinate to a borrowed `TermRef<'source>` from that same preparation.
  The existing readers use this resolver; no owned `Value` is reconstructed per row, and a block can be iterated
  any number of times.
- **Items on the formula route.** The prepared rules are private storage, reached through `Prepared::items()`, the
  typed traversal (`Item::Rule` or `Item::Facts`), and `rules_without_facts()`, a separately named view for readers
  that provably never see facts, each use carrying its reason. Making the field private turned the compile errors into
  the complete reader inventory.

**Stage 1's conforming representation.** Each integer segment stores its first number, an expanded column end,
and a checked span of source-component coordinates. Admission still calls `root_scalar` for every expanded column
value, in the established order: each column's first value in argument order, the predicate, then the remaining
values of each column in reverse argument order. All root-domain admissions, scalar occurrences, work charges and
polls remain in that sequence. Only after receiving a coordinate does the compiler try to extend its current segment.
Both the number and the occurrence coordinate must be consecutive; a gap in either starts a new segment. No property
of canonical IDs is inferred from numeric adjacency. In particular, the first value of an earlier column is normally
separated from its tail by later columns' first values, so one source interval can occupy two segments.

Product scratch retains column descriptors, first-row coordinates and segments instead of an expanded coordinate slot
per value. An interval of K values therefore removes O(K) arena and scratch coordinate storage in favor of O(segments).
It **does not remove** the canonical vocabulary, scalar occurrences in `TemplateComponents`, root-domain entries, or
their admissions. These still grow with the distinct values or admitted occurrences. Stage 1 does not claim that the
whole preparation of `p(0..K-1)` uses constant memory.

A product row computes each column's mixed-radix offset, locates its segment by binary search, then selects the
certified occurrence coordinate. Reading a cell costs O(log segments) plus the existing borrowed scalar resolution;
reading every integer remains O(K), with no per-row value construction. Positive-source validation keeps its logical
cell order and work charges. A future numeric analysis can read segments directly under its own design.

Runs hold one growable scalar vector per argument and one evidence end per row. Appending a row reserves all argument
buffers before publishing any cell or row. The arena's one lease counts the outer headers and every nested capacity;
each replacement admits the old and new capacities together, geometrically when that fits and exactly otherwise.
A refusal can leave reserved capacity, including empty new column headers, but no partial row; the lease continues
to cover every retained buffer. Splitting reserves all destination headers and column buffers before moving any item,
then copies segments, columns and evidence without allocation. The original arena keeps its buffers and lease until
it is dropped, so copied storage is counted on both sides while both owners live.

Compact intervals reduce arena storage; segment descriptors and column-buffer headers can increase it for short
products or runs. Expansion `ScalarBytes` changes with the compact scratch capacities. `SupportBytes` can move in
either direction, including at replacement peaks; tight storage limits can therefore refuse earlier or later. This
conformance does not claim identical storage receipts or establish its performance acceptance without measurements.

**The forms converge.** The arena is the transport between expansion and the routes. On the formula route the support
`relation::Catalog` is the one per-fact representation through grounding, a fact row interned into it in round 1.
Stage 3a gives the lazy route the same form: a `relation::Catalog` per fact predicate over a closed atom store the
program owns. There is no separate row range per publication: round-1 atoms are sorted and deduplicated before
publication, origins attach at emission per head, and duplicate facts keep their occurrences.

### Stage 1: the formula route stops building a rule per fact (implemented)

Facts are compiled into fact blocks at their item positions; support round 1 interns their rows at the item position
with no producer slot; exact activity is keyed by component rank and item position; final emission emits fact rows
without a join. Every reader of the former rule list was assigned to `items()` or `rules_without_facts()`, and the
objective forwarding definitions count facts as competing producers.

### Stage 2: analysis inputs without a statement per fact

`analyzed_program()` and `source_analysis()` keep describing the full program. The analysis preparation runs uses a
summary, and the full analyzed program is built from the families only when it is needed.

**1. The summary.** It keeps every analyzed carrier compiled outside the fact arena plus the first fact of each
signed signature. Canonical collection combines these carriers and records two distinct identities: rewrite
eligibility in `Owners`, and the last original source statement contributing each real carrier. Representatives have
no real-carrier identity. The latter sidecar survives a content collision even when that collision makes a rewrite
ineligible. `replace_constraints` removes the asked originals and appends its one compilation's replacement carriers.
`Summary` owns this state and the exact reconstruction recipe; `FullProgram` owns the complete program and its analysis.
`ask_all` and `specialize` take `(&Summary, &Owners)`, so their positional statement and owner inputs cannot disagree.

The fact recipe records products as mixed-radix column segments: an integer interval or a leaf ordinal in the already
validated source carrier. Unchanged carriers borrow the retained `Arc<Program>` by original statement identity;
rewritten carriers retain the one normalized singleton program already produced by compilation. `Families::read`
creates a temporary resolver borrowing these atoms and symbols. It never normalizes again, exports a core value or
admits a symbol. This recipe serves exact source inspection; execution and numeric analysis still read the fact arena
through its actual catalog coordinates.

A canonical locator sequence merges all content-equal arena facts in themelios statement order, retaining contributing
family identities for merged root provenance. These are locators and evidence, not copied argument rows. The original
occurrence counts, scalar admissions, origins and item order stay separate and unchanged. Building the sequence takes
one bounded lookup per original row, O(R log U) comparisons for R occurrences and U distinct arena facts; comparisons
read the argument columns. It retains O(U + E) locators and additional family evidence E. Compact intervals therefore
remove expanded analysis statements, not all per-row metadata or the existing admitted vocabulary.

**Why the summary's analysis is the full program's.** zetesis reads the dependency graph's predicates, edges and
components, and the program classes (horn, normality, stratification) that order certificate attempts. A fact adds its
predicate's node and no edge, and is Horn, normal and stratification-neutral; the summary keeps one fact per signature,
so it keeps every fact predicate's node. That invariance is themelios's behaviour at the reviewed pin, not something
zetesis can assert from outside, so a differential pins it on every maintained program and generated shape, and a pin
bump re-runs it.

**2. The public accessors: exact, infallible, rebuilt on demand.** `analyzed_program()` and `source_analysis()` on
`AdmittedFormula`, `AdmittedFormulaBundle`, `PreparedFormula`, `PreparedFormulaBundle`, the hybrid owners and
`AcyclicSource` each return the full program and its analysis, built at the first call and cached in a once-initialized
cell inside the owner's shared state.

- *Built from* the prevalidated source recipe and the post-replacement real carriers. A merge walk compares virtual
  facts with real carriers in themelios canonical content order. Equal carriers union root provenance; the later
  original carrier supplies nested evidence, matching themelios ingestion. Each fact clones validated `Symbol`
  leaves or constructs an already-bounded integer, with constructed child provenance. `Program::of_nodes` and
  `Analysis::of` then produce the exact complete view. No fallible bridge or normalization replay is deferred.
- *Built in preparation only for partition-eligible grounding programs*, after constraint replacement. The terminal
  partition's eligibility check is the explicit trigger. It reads the complete program, retains the complete original
  analysis and moves that view into the terminal owner; the selected base already has a complete analyzed program.
  Both public accessors share the same once-initialized view and reuse a preparation-built view.
- *Cost.* The first construction copies the full normalized statements and fact values, ingests them canonically and
  computes their full analysis. Its work includes upstream ingestion and analysis; it is not merely a linear row walk.
  The complete program and analysis then remain for the owner's lifetime. Summary preparation keeps the graph and class
  analysis without these expanded statements.
- *Failure.* The accessors stay infallible and retain the upstream ordinary allocation boundary. Preparation checks
  `AnalysisNodes` and `AnalysisEdges` in the exact full canonical statement sequence, after content deduplication and
  provenance union. The original row-occurrence allowances are checked separately during compilation. Typed source
  conversion and admission have already succeeded before a recipe is retained.
- *Budget.* Public inspection charges no budget. A later preparation trigger still performs the same logical atom,
  resolver and full-carrier reservation charges, independent of whether public inspection populated the cache. The
  trigger marker is recorded only after success; subsequent preparation use reuses that admitted view. Fresh
  preparation reconstruction reserves authored vectors fallibly and polls between entries, while upstream AST
  cloning/ingestion/analysis retains its ordinary allocation boundary.
- *Documentation.* The accessors describe first-call construction and retention. `source_classes()` reads the summary's
  equal class verdicts without rebuilding. `Debug` prints summary and canonical/original fact counts, builds nothing,
  and does not depend on whether an accessor ran. `source_analysis()` still describes exactly `analyzed_program()`.

**3. Internal readers move to the summary.**

- *Certificate order.* The solve crate computes its certificate order from the source analysis's classes and the
  analysis basis (`certificate_order`, `crates/zetesis-solve/src/countermodel.rs`). Read through the public accessors,
  every formula solve would rebuild the full program, so each formula owner gains a public `source_classes()`, the
  classes read from the summary analysis, and the solve crate reads that.
- *Domain analysis* reads facts from the fact blocks through the numeric analysis's engine; Stage 2 builds no fact view
  and changes nothing in `zetesis_domain::analyze(&Program)`, which keeps its contract for external callers. That
  analysis's Stage 1 lands no later than this Stage 2, since the summary would otherwise starve the old set analysis of
  facts.
- *Positive certificate.* Its scan reads the summary and checks each product's distinct constants with the per-term
  test; the verdict is equal because the test is a conjunction over an atom's terms.
- *Keys and fixed constraints.* Their statement loops (`zetesis_domain::keys`' two passes, `ask_under`'s `TermWork`
  unit, specialization's steps under its 100,000-step policy) run over the summary, O(statements kept) where today they
  spend O(facts). Their facts come from the families: `FactIndex` gains one constructor, `FactIndex::read_parts(producers,
  facts, work)`, the same construction as `read` over a program given as its non-fact statements and its fact atoms, charging
  one key-work step per statement of the program it describes. `read(&Program)` keeps its contract.
- *Which programs read facts in preparation.* Key analysis, as soon as it finds a keyed relation (a single-element
  `1 { … } 1` choice that is its predicate's only producer, such as `1 { color(X,C) : col(C) } 1 :- node(X).`); and
  fixed-constraint specialization, for any integrity constraint with two or more flat positive literals and an anchor
  atom with two distinct named variables, such as `:- edge(X,Y), color(X,C), color(Y,C).`, unless dependency projection
  is on. The ordinary assignment problem over data has both. These programs pay transient fact atoms; only
  partition-eligible grounding programs pay the full program, retained.
- *Owners* collect the summary's carriers, O(statements kept).
- *Terminal partition.* Where it is eligible, `terminal::analyze` reads every producer of each selected signature, and
  the partition takes ownership of the analyzed program, so materialization takes the full program from preparation's
  reader and the partition is unchanged. Stage 2 saves nothing retained there; a terminal analysis that reads facts from
  the arena is a later step with its own differential.

**One fact input in preparation: `PreparationFacts`.** The post-compilation `Summary` owns its source recipe,
canonical locators and eventual full view. The replacement-planning scope owns one `PreparationFacts` borrowing that
summary and lazily owning canonical fact atoms. `FixedFacts` borrows this outer owner and caches `FactIndex` over its
atoms, avoiding self-referential storage. Both are dropped after key analysis and specialization, before replacement.

- *Triggers.* The first qualifying key or specialization request builds the canonical atom sequence and then calls
  `FactIndex::read_parts`; subsequent requests reuse atoms and a completed index. This sequence includes generated
  facts from real analysis carriers as well as arena facts, deduplicated and interleaved in full program order. Every
  remaining producer is supplied separately once; representatives never stand in for the complete population.
- *Charges.* Atom construction uses the per-fact structural rates (`TermWork`, name `ScalarBytes`) plus named resolver
  and vector reservations; `read_parts` retains one `KeyWork` step per canonical statement. The exact analysis
  preflight still charges its canonical structural scan. A terminal trigger additionally reserves the complete
  carrier vector by one deterministic logical charge, even if inspection has already allocated it. Requested
  authored-vector growth is admitted before allocation; AST/provenance and upstream allocator overhead remain outside
  that named-payload allowance.
- *Refusals.* A key-work stop retains `KeyAnalysis::Stopped` or specialization's `Failure::Optional`. An expansion
  refusal constructing complete fact input or the terminal view refuses preparation. It is a distinct result variant,
  never absorbed as an optional stop. A stopped index publishes no partial index; already-built atoms may be reused.

### Stage 3a: the lazy route's core program keeps facts as tables

`zetesis_core::Program` holds facts as first-class columnar tables, `templates()` no longer lists them, and
`Program::new(templates: Vec<Template>, facts: FactTables, limits)` takes them. `FactTables` is the lazy-route form of
the arena (closed `Value`s and `i32` intervals, with runs); each block carries an anchor, the number of templates
before it; `FactTables::default()` is no facts.

**Construction.**

- *Every door emits tables through one classification* in `compile`: the extended route (whose `emit` also serves
  `prepare_program_relational`), the door `admit`, and the already-admitted door `admit_program`. A rule with an
  empty body and a positive atom head whose terms are closed values becomes a fact block at its program position; every
  other statement becomes a template. A program has the same representation whichever door built it.
- *Every construction-time pass takes the classified sequence*, so a pass that has not decided what it does with fact
  blocks fails to compile:
  - strong-negation coherence (`coherence::append_with`) collects a block's predicate and first evidence as it collects
    a template's patterns, and returns early only when neither carries a negative sign. Without this, `p(1). -p(1).`,
    `-p(1). p(X) :- q(X). q(1).` or `p(1). -p(X) :- q(X). q(1).` would lose its constraint, and the lazy route, which
    runs no other coherence check, would admit `{p(1), -p(1)}`. It charges one `TermWork` unit per block;
  - the count checks (`check_core_limits`, and `admit_program`'s `max_templates` pre-check) count templates and fact
    rows and refuse at today's statement. Coherence and native admission can report a classified template index or
    fact-row locator. The typed door's count pre-check refuses before inspecting the next statement and therefore
    reports no native index; it retains that original borrowed statement and the existing resource precedence;
  - statement identities derive from the item's position in the sequence, so every `ProgramSite` after the first fact
    keeps today's identity;
  - the evidence builders produce `template_origins()`, `fact_origins()`, `fact_sites()`, `template_statements()` and
    `fact_statements()` from the same sequence.
- *Derived views range over templates and fact rows.* `domain()` gains every fact value and `predicates()` every fact
  predicate, with `max_domain_values` checked on the union; `gate_predicates()` is unchanged, since no fact
  contributes to it. These views are load-bearing: built from template rows alone, the domain of `p(5). { q(X) } :- p(X).` would
  lack 5, `q(5)` would leave the carrier those views define, and an answer set would be missed.
- Admission interns each distinct value once into the vocabulary, interns each distinct fact atom once into the
  program's fact store, inserts it into its predicate's relation catalog, records the occurrence, and records the
  program order (O(runs)). `Program::new` never reclassifies: body-free ground templates a caller passes stay
  templates.

**Representation (A).** A `relation::Catalog` per fact predicate over a program-owned closed atom store, with an
occurrence list in program order. Every part exists in `zetesis-core` and the formula route already runs the pattern: a
writer over the program's vocabulary (as `AtomInterner::for_program` builds one), the consuming close
(`AtomInterner::into_closed_with`), and descendants that start empty over the closed base without copying
(`AtomInterner::for_closed_catalog`). Each component has a reader: row membership gives admission's deduplication and
the restrictions reader's uniqueness; the column reader seeds argument bounds once before variable propagation, with
repeated values coalesced by the existing finite-domain set; equality-identifier columns remain available to
subsequent consumers. Ground-head coordinates are retained per original occurrence. Ordered runs expose canonical
rows without a new sort. The restriction reader preserves original occurrence accounting through the mixed-rule
scan and the existing coordinate deduplication path. The store's bytes fall under
`AdmissionLimits::max_bytes`; the writer is given the remainder plus the vocabulary's bytes, so the vocabulary is
counted once. A program without facts has no store: `Program::fact_catalog()` returns `None`, and its workspaces bind
exactly as today.

**Whole-program traversal.** `Program::rules()` yields templates and fact rows in program order, and
`Program::is_empty()` means no rules of either kind. With `templates()` removed during the change, every call site
fails to compile and is assigned to `rules()` or `templates()` with its reason. Readers of facts: closure emptiness
(`is_empty()`), bootstrap (iterates `rules()`), argument bounds (each canonical fact column is visited once before
the fixed point, with every cell charged and repeated positions coalesced), head coordinates (a fact occurrence's
dense coordinate from its canonical arguments), `unconditional_facts` in
`crates/zetesis-cpu/src/oracle/restrictions.rs` (original fact occurrences follow the same checked coordinate and
deduplication path as explicit template facts, preserving their resource counts and refusal order), the host source scan and eager grounding
(fact rows at their positions). Rule indexes, join dimensions, world workspace depth, constraint restrictions and block
steps read templates only.

**The least closure.** The oracle computes the least positive closure selected by a frozen seed, the least consequence
of the reduct relative to the candidate, then compares it with the seed on the carrier `gate_predicates()` defines and checks constraints.

1. *Emptiness.* `check_view` and the prepared queries' closure test `program.is_empty()`, so a program of facts alone
   has its facts as its closure.
2. *Bootstrap.* Every non-incremental round iterates `rules()`: a template is visited exactly as today, and a fact row
   is seeded at its position with one work tick and `record_head`. Today a fact template has no condition and no positive
   literal, so its visit produces exactly one empty assignment whose emission is `record_head` on the head.
3. *Indexes* stay over templates; incremental rounds select rules named by new predicates, and a fact names none.

The reduct contains every fact unchanged, and its least consequence is the closure of the seeded rows under the
remaining templates' reducts. Bootstrap records the same heads in the same sequence, later rounds select the same
templates, and the closure, its round history, the agreement and the constraint verdict are identical.

**The workspace descends from the fact store.** `bind_program` builds the workspace's authority as a descendant of the
program's fact store through an explicit, additive `Program::fact_catalog()` and `AtomInterner::for_closed_catalog`.

- Presence in the base supplies no truth, and the descendant has a fresh atom scope and empty discovery.
- What the oracle reads is a discovery position, and descent assigns positions in today's order: for a fact predicate
  in a tree relation, `record_head` finds the atom in the base instead of importing it, in today's sequence; for a dense
  one, `record_head` marks a pending bit and `take_model` resolves it in the base at model publication, in
  relation-directory order and ascending dense position, as today. Only coordinates internal to the store differ, and
  the oracle reads none.
- Only oracle workspaces descend. The lazy-rounds catalog and eager grounding keep importing every fact atom, and each
  can descend later as its own change.
- *The ledgers.* `ClosureWorkspace::retained_bytes()` stays per workspace and inclusive, now including the shared fact
  store as it already includes the shared vocabulary; `max_closure_bytes` is checked against it, so a workspace whose
  allowance is below the store's bytes refuses at bind. The batch cache's `retained` and `admit`, and
  `QueryStatistics::retained_bytes`, are collective and subtract the store's rows and index
  (`shared_closed_catalog_bytes() − shared_vocabulary_bytes()`) from each workspace's inclusive receipt and count
  those preparation-owned allocations once in the shared cache, including while workspaces are still unbound.
  Vocabulary accounting stays as today.

**Public contracts and migration** (CHANGELOG): `Program::new(templates, FactTables::default(), limits)`; `templates()`
lists the non-fact templates on every door; `rules()` and `is_empty()`; `fact_origins()`, `fact_sites()` and
`AdmittedProgram::fact_statements()`, parallel to the fact rows; `Program::fact_catalog()`; `max_templates` documented
as bounding "source-normalized rules: templates and fact rows" (the count is today's on every program the extended
route builds); a fact-row locator on `AdmissionError` beside `template_index()`, which each door maps through its own
fact evidence; canonical failures use one inline cause plus
`AdmissionOrigin::{Program, Template, Fact}` rather than separate payload variants, and
`Canonical::origin` replaces its former `template` field without allocating to report a refusal;
`Program::storage_bytes()` counting the template catalog, metadata and the store's rows and index; and
the two retained-bytes receipts as above. The template-order accessors stay parallel to `templates()`. Stage 3 changes
what the device routes receive and is qualified by `scripts/check.sh hardware` on each backend.

### Stage 3b (own design): facts bound in the closure

After 3a each check still records every fact row at bootstrap. 3b binds the program's facts as true without the
per-check visit and lookup, in one of two forms: seeding truth by identity from each fact atom's recorded discovery
position through the publication `Catalogs::insert` performs (still O(facts) per check; dense fact predicates are
seeded as bits at their prepared coordinates), or a base relation shared across workspaces and read beside each
check's own relation (O(1) per check, needing a scope rule the store lacks). Either changes the closure's
representation under the refinement the [correspondence](../book/lean/correspondence.md) states, equality with a fresh
full-round closure, so 3b is designed separately. 3a leaves its seam: workspaces descend from the fact store.

### Stage 4 (own design): the ground program's facts as a fixed-true set

The Ferraris theory already treats an atomic root as a fact. Stage 1 keeps one atomic root and one producer per fact.
The end state replaces them with a fixed-true atom bit set over theory atoms, with per-family evidence, removing the
per-fact root, producer and evidence entries from the theory, search, certificates and metadata. It changes the
per-root `formula_origins()` contract, touches `zetesis-ferraris` and `zetesis-sat`, and states its own membership
argument: facts hold in every answer set because the reduct's least model contains them.

### Staging

| Stage | Lands when | Measured as | Checks |
|---|---|---|---|
| 1, conforming change | before Stage 2 | byte-identical output; formula-route time and memory per fact | portable, oracle, coverage |
| 2 | after the conforming change, with or after the numeric analysis's Stage 1; the analysis and `FactIndex` differentials hold | analysis, owner and key work per fact; live heap per fact | portable, oracle, coverage |
| 3a | the closure differential holds | lazy-route time and memory per fact; admission time per fact; each workspace's own closure bytes and first-check work; restrictions preparation work | portable, oracle, coverage, hardware |
| 3b, 4 | their own designs | per-check closure work; per-fact roots and producers gone | their own |

Each stage leaves the previous one's seams in place, and no stage depends on a later one.

## Correctness

**Stage 1.** The ordering rule places each fact row where its rule stood, so every downstream order is unchanged and
output is byte-identical wherever today's is deterministic.

**Stage 2.** The summary's graph and classes equal the full program's by the structural argument, pinned by the
differential. `FactIndex::read_parts` over the summary's non-fact statements and the families' fact atoms is the index
`read` builds over the full program: the same producers mark the same signatures, the same heads set the same opacity,
and each signature's tuples are its facts' arguments in full canonical program order, including generated facts. Key analysis reads producer counts only to ask
whether a choice head has exactly one producer; a signature with facts has at least one producer in both programs, so
the answer is equal. Wherever both programs' passes complete, the same keys are found and the same constraints asked
and rewritten. Where a ceiling binds, the summary's loops spend less and can reach further, and every rewrite is sound
by its module's argument, so answer families are kept. The rebuild produces the same post-replacement statements in
order, with the same root unions and later-carrier nested provenance. Canonical unique analysis counts never replace
original fact-occurrence accounting.

**Stage 3a.** A fact column's argument-bound contribution is constant across fixed-point passes, so adding it first
gives the same least fixed point; a value's domain position by identity equals the content search's result, since
equal canonical terms share one identifier. Each whole-program reader yields, at each fact row's position, what it
yielded for that row's template. The derived views gain exactly what each fact template contributed. Every door
classifies with one function, and every construction-time pass reads the classified sequence, so coherence
constraints, their evidence and every count verdict are today's. The closure and workspace arguments are above.

**Provenance and order.** Evidence is recorded once per statement and attached per row where it was attached per rule
or template, so every diagnostic naming a fact's statement, span or transform tag reads the same evidence. Stages 1
and 3a are accepted on byte-identical output; Stage 2 on equal answer families, since a binding key-work ceiling can
now rewrite more constraints, and on byte-identical output where its passes complete on both programs.

**Failure boundaries.**

- *Kept.* Every ceiling on the work a fact needs is checked at the same point with its typed failure and site: atom
  interning, support bytes for atoms, roots, origins, `Templates`, `Values` and `ScalarBytes` at expansion, the analysis
  ceilings on full counts, `max_templates`, `max_domain_values` on templates and fact rows together, the coherence
  constraints and the per-statement counts.
- *Removed.* A refusal that came only from skipped machinery's own charges can occur later or not at all: rule objects
  and their `Origins` charge, pattern rows and joins (Stage 1); expanded analysis carriers and their owners (Stage 2);
  per-template preparation and core admission bytes (Stage 3a).
- *Added.* Stage 3a's fact store, charged under `max_bytes` at core admission, where today atom storage is charged to
  the closure allowance at the first check.
- *Moved in time.* Each workspace's `max_closure_bytes` is charged the shared store's atoms at bind instead of each atom
  as the first check imports it. At w workers the per-workspace allowance is the collective one divided by w, so a
  fact-heavy program can be refused at bind; the refusal is the storage stop, reported with the worker count.
- Refusals never become unsatisfiability, exhaustive enumeration or proved optimality.

## Accounting

Work receipts measure the work performed; every public receipt that moves, by direction:

- **Stage 1 (implemented).** `Origins` falls by each fact's origins count (a fact was charged twice). Formula work
  counters fall by the skipped visits, variant lists, joins, plans, binding storage and attempts. `SupportBytes` falls
  by the pattern rows and rises by the arena's lease (statement evidence, product segments and columns, block
  records, run-column headers and cells, and run ends), each growth admitted before allocation. Before the conforming
  representation, deterministic receipts showed the largest support peak rising on 82 of 133 programs (those with few
  facts per block, at most 5.5%, median 1.9%), falling on 35 (every fact shape, 3.2% on `wide` to 20.3% on `frange`) and
  equal on 16. Those figures do not qualify the new layout. Its compact scratch changes expansion `ScalarBytes`,
  while all scalar admissions and their work remain. Grounding observers see one rule-instantiation phase per block.
- **Stage 2.** `AnalysisNodes` and `AnalysisEdges` use the complete deduplicated canonical statement sequence, and its
  structural `TermWork` and signature `ScalarBytes` preflight remains. Owner work applies to summary carriers.
  Source recipes, canonical locators, evidence-family lists and temporary resolvers add named storage reservations and
  one canonical lookup per original fact occurrence. Fact atoms are constructed only for a qualifying key or
  specialization read, and full carriers only for terminal analysis or public inspection. A preparation full-view
  trigger reserves its logical carrier capacity regardless of prior inspection. `ask_under` and specialization skip
  elided carriers, so their work stops can occur later; every resulting rewrite retains its existing soundness argument.
  These allowances describe the implemented operations, not a promise that every program's total receipt decreases.
- **Stage 3a.** `max_templates`' count is unchanged. `Program::storage_bytes()` loses the template structures (about
  390 B per fact) and gains the store's rows and index (about 105–175 B per distinct fact), so the `max_bytes` boundary
  moves later. Core admission work rises by each distinct fact atom's interning, which moves there from every
  workspace's first check, and by the relation catalogs' build, which is new. Coherence's `TermWork` falls to one unit
  per block. The closure allowance is charged at bind. Closure work falls by the per-template bootstrap overhead and
  each workspace's import of fact atoms; oracle preparation and host-scan work fall; the restrictions reader stops
  deduplicating fact rows. The lazy-rounds catalog's and eager grounding's receipts are unchanged.

Tests that pin exact receipts for programs with facts are re-derived, each with its reason, and the least-ceiling
searches keep their claims.

## Costs

Estimated time per fact at 16,000 facts, each owed a measurement:

| Stage | Removes | Formula route | Lazy route |
|---|---|---|---|
| 1 | rule objects, per-occurrence pattern admission, producer slots, support visits, activity joins | about 2.9 of 12 µs | — |
| 2 | `Analysis::of` input, owners, analyzed statements and domain producers per fact | about 1.8 µs where it applies | — |
| 3a | per-fact templates (core admission 0.33 µs), per-template preparation and bootstrap visits; each fact atom's import moves to admission, once | — | about 0.3–0.5 of 4.6 µs at one workspace, which the catalogs' build can consume on `bare`; more with several workspaces |

With the fact path's parts, the formula route goes from about 12 µs to about 5 µs per fact (195 ms to about 80 ms on
`fcmp`). The remainder is necessary work: parse and raise (about 1 µs, upstream), atom interning and publication (about
1.3 µs), model order and output (about 0.7 µs). Final emission's per-fact cost on the grounding sub-route is
unmeasured.

Memory under Stage 3a, per distinct fact on `bare`'s facts, as live-heap differences: the template structures every
representation removes (core template catalog 266 B, core admission 88 B, template origins 37 B) less the statement
evidence that remains (about 25 B) is about 365 B net. Representation (A) adds about 105–175 B to the program and saves
each active workspace its payload and exact index, about 50–60 B per fact atom (its discovery entries, about 100 B,
remain): about −0.23 to −0.31 KiB per fact at one workspace and −0.87 to −1.07 KiB with fourteen active workspaces.
How much of that resident size shows is what the measurement reports.

## Tests

Tests live in the existing per-crate test binaries; fact shapes are generated in Rust at small K, and new clingo
fixtures are small-K files in the oracle campaign's fixture module.

- **Stage 1 differential** (existing): support contents and row order, per-round deltas, activity cells, source
  eligibility, the builder's atoms, nodes, roots, origins and producers, and terminal selection agree with the rule
  path; `--all` output is byte-identical at one worker.
- **Stage 1 representation.** Checked spans reject numeric and occurrence-coordinate gaps; mixed-radix rows resolve
  to the same borrowed values, including signed integer endpoints; wide intervals keep constant segment capacity;
  run columns reproduce logical row order. An explicit scalar/predicate admission sequence matches the compact
  compiler's coordinates and work. Every outer and nested reservation failure publishes no partial row; retained
  capacities equal their lease. Split copies preserve scalar coordinates and evidence, reserve their summed outer
  storage and individual column buffers before moving, and allocate nothing while copying.
- **Stage 2 differential.** The summary's and the full program's dependency graphs, construct scans and `classes()`
  are equal; `source_classes()` equals `source_analysis().classes()`; `FactIndex::read_parts` equals `FactIndex::read`,
  tuples, opacity and key-work steps alike; the rebuilt `analyzed_program()` equals today's at every nesting level,
  built by an accessor and inside preparation, including the keyed program
  `#const target=a. key(a). value(0;1). 1{p(K,V):value(V)}1 :- key(K). :- p(target,V), V != 1.` and a fixed-witness
  specialization; `Debug` builds nothing; key analysis and specialization ask and rewrite the same constraints where
  both complete; under a binding ceiling every rewrite is sound and receipts move as stated; a program with an anchored
  integrity constraint and one with a keyed relation read facts through `PreparationFacts` and build no full program,
  while a partition-eligible program builds it once and reuses it; refusals inside `PreparationFacts` keep their
  mappings and types.
- **Stage 3a differential.** Templates and the fact store give equal argument bounds, head coordinates, unconditional
  restriction rows, host-scan instances, eager ground programs and derived views (including `p(5). { q(X) } :- p(X).`);
  closure checks over every candidate of the maintained populations give equal closures, round histories and verdicts,
  discovery positions compared exactly, from a workspace over the vocabulary and from one descending from the store; a
  program of facts alone has its facts as its closure; device answers equal the CPU's.
- **The doors.** The same source through `admit`, the extended route, `admit_program` and `prepare_program_relational`
  yields the same templates, fact rows, views and evidence; the three strong-negation programs keep today's verdict and
  coherence constraint with today's evidence on every entry point; a program at the template limit refuses at today's
  statement; a refusal at a fact row reports a fact-row locator with its statement's location; statement identities
  after the first fact are today's.
- **The fact store, the allowance and the ledgers.** The store's atoms are exactly the distinct fact atoms, and the
  occurrence list reproduces program order with duplicates; the shared least-ceiling search finds the least `max_bytes`
  admitting a fact program, and one byte less refuses with no partial program; an allowance below the store's bytes
  refuses at bind with the storage stop; per-workspace receipts include the store and collective ones count it once.
- **Duplicates, provenance and answers.** Repeated facts keep today's origins and diagnostics; answer families are
  identical on the portable, oracle and scalability suites.

## Measurement and acceptance

**Workloads**, at K = 1,000, 4,000, 16,000 and 64,000: the fact shapes above and their variants (several predicates,
copies, joins, provenance), plus formula shapes that reach the grounding sub-route, each bounded by a comparison the
extended compile refuses as not ground: `fchoice` (K facts and `{ q(X) } :- p(X), X > K - 3.`, reaching final emission
and the terminal partition), `fcycle` (with `q(X) :- r(X). r(X) :- q(X).`), `fmin` (with `#minimize { X : q(X) }.`),
`fproj` (with `#project q/1.`), a keyed shape for key analysis and `fanchor` (`edge(i,i+1)`, alternating
`color(i,c)`, `:- edge(X,Y), color(X,C), color(Y,C).`). Lazy shapes with many candidates (`fchoice-lazy`, 2^K answer
sets, measured with `--answers N`) serve Stage 3b. The route is recorded on every run from the banner and the recorded
execution statistic.

**Per stage:** interleaved A/B of process time on a quiet host; live heap per fact at the stage's named moment, with
the resident slope under `MallocMaxMagazines=1 MallocLargeCache=0` as the process check; the corpus at one and fourteen
workers and the scalability population. Stage 3a also reports each workspace's own closure bytes (its retained bytes
minus the store's rows and index) beside the store counted once, first-check closure work at one and fourteen workers,
and restrictions preparation work. Reach is reported per path: facts admitted as blocks against generated-fact rules,
the programs on each sub-route, those the terminal partition takes, those that pay `PreparationFacts`' fact atoms, and
for Stage 3a the programs built with tables by each door.

**Accept a stage** when outputs are byte-identical (Stages 1 and 3a; Stage 2 where its passes complete on both
programs) or answer families identical (Stage 2 under a binding key-work ceiling); no cell is slower by more than 3%
with separated ranges; and the targeted shapes improve in time and in live heap per fact, with the pinned-allocator
resident slope not worse. Final emission is accepted on `fchoice` and `fcycle`, on runs whose recorded route is the
grounding sub-route. **Stage 1's acceptance run** measures the reviewed commits against their parent, built the same
way: time per fact at 4,000, 16,000 and 64,000; reach per path; the grounding-sub-route shapes' memory and final
emission; and answer and byte identity on the 122 programs.

Measured so far for Stage 1, on sibling builds: answers and output bytes identical on 122 programs under the default and
forced formula routes, and live heap −0.29 KiB per fact. The owner record's growth fix took total allocation per fact
from 296 KB to 40 KB at 64,000 facts.

## Alternatives

- **A separate fact table beside the support relations.** It duplicates the dictionary and columns the support
  relation holds, and joins would read two structures for one predicate.
- **Keep facts as rules and skip their machinery per consumer.** Keeps a rule object, origins vector, owner entry,
  analyzed statement and template per fact, and every consumer must keep recognizing facts. Withdrawn.
- **The unit join**, the join of no relations producing one empty binding without a plan or storage. Complementary
  (it would serve generated facts and body-free choices and disjunctions), and counted before it is designed.
- **A record per fact family.** Rejected for memory on the instance-file shape, where a family is one fact.
- **Feed themelios no fact statements.** Fact-only predicates would vanish from the dependency graph. One
  representative per signature keeps the node set, for O(predicates).
- **A themelios analysis entry point that takes facts as relations.** Would make Stage 2's invariance a property of the
  API; outside zetesis, and recorded for upstream.
- **A shared evidence slice per family.** The public `formula_origins()` owns a vector per root; the slice belongs to
  Stage 4.
- **Stage 3 representations (B) and (C).** (B), an occurrence table per fact predicate with dictionary-encoded columns
  and no atoms, is the smallest first step with the least admission memory, but keeps each workspace's full import
  (about 150–160 B per fact atom) and adds a third representation until a migration. (C), the closed store and
  occurrence list without relation catalogs, has the least memory but gives up the columns the compounding designs
  read. Every reader survives any of the three, since each reads the store through its occurrence list.
- **A fact view in `zetesis-domain`** feeding the source-level set analysis. It would be built only to be removed,
  since the numeric analysis reads the fact blocks.
- **The full program built before constraint replacement.** Every formula-route program would pay, and replacements
  would be compiled twice.
- **Key analysis and specialization on a rebuilt full program.** It charges the ordinary assignment problem over data,
  the class this design exists for.
- **Every tuple owner descending from the store.** The widest sharing, but each owner's footprint would include the
  store and `Program::shared_vocabulary_bytes()` would need a rename to count it. The natural later step once the other
  owners are measured on fact-heavy input.

## Decisions

1. Stage 2's analyzed program is exact and rebuilt on demand: the public accessors keep describing the full program;
   preparation analyzes the summary; where preparation itself needs the full program, it builds it there, after
   replacement, and the accessors reuse it.
2. Stage 3 changes the core `Program` API: facts are first-class tables, `templates()` no longer lists them, and
   `Program::new` takes them, as a breaking 0.x change with a migration.
3. Stage 4, a fixed-true fact set in the Ferraris theory, is the end state, with its own design.
4. Formula owners gain a public `source_classes()`; the exact accessors stay infallible, and their on-demand rebuild
   charges no budget.
5. `max_templates` also counts fact rows; `fact_origins()`, `fact_sites()` and `AdmittedProgram::fact_statements()`
   are added; `AdmissionError` gains a fact-row locator; body-free ground templates passed to `Program::new` stay
   templates.
6. Stage 3's representation is (A): a relation catalog per fact predicate over a program-owned closed atom store.
7. Stage 3a reaches the store through an explicit, additive `Program::fact_catalog()`; only oracle workspaces descend.
8. Every door emits tables through one classification, and the summary and the full program are distinct types.
9. `zetesis_domain::analyze(&Program)` keeps its contract for external callers; preparation's domain analysis reads
   facts from the fact blocks through the numeric analysis's engine, so Stage 2 builds no fact view.
10. Stage 1's arena conforms to interval segments and column-major runs, with the cell encoding behind a row accessor,
    before Stage 2.
11. The owner record's geometric growth is accepted.
12. Stage 2 reads key analysis's and specialization's facts from the families; the terminal partition's fact read
    stays a later step.
13. The per-fact memory that belongs to themelios is reported upstream.

## Remaining acceptance items

The Stage 2 canonical reader, source resolver, full-view trigger and ownership are specified above and pinned by the
focused differentials. Passing those checks establishes representation behavior, not performance acceptance.

1. Measure the conforming representation against the cost model: interval readers, column-major growth, summary and
   canonical-locator storage, transient fact atoms, and the direction of support and expansion charges. The structural
   memory bound alone is not performance acceptance; the earlier Stage 2 time estimate omitted its retained exact
   structural preflight and new locator work.
2. "Product" names two different things in this design; give each its own name.
3. The keyed measurement shape must actually reach the fact read; record its route and reader reach.
4. Name the commit Stage 1's acceptance run measures, and verify every cited site at the branch head.
