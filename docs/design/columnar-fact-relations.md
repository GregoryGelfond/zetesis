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
| Stage 1's conforming change: interval segments, column-major runs, the cell encoding behind a row accessor | Decided; open; lands before Stage 2 |
| Stage 1's acceptance run on the reviewed commits | Owed |
| Stage 2: analysis inputs without a statement per fact | Designed; open |
| Stage 3a: the lazy route's core program keeps facts as tables, representation (A) | Designed; open; its prerequisite, the fact path's position table, is implemented |
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

Facts that keep a function, tuple or arithmetic subterm after normalization are generated facts and stay rules on
every stage.

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
expansion order (alternatives in order, last argument fastest). A fact block has one of two layouts:

- a **product**: the facts of one argument alternative of one source statement. A statement whose alternatives differ
  in predicate or arity, such as `p(1;a,b).`, contributes one block per product, so every block has one predicate, as
  the acyclic plan's key of component rank and position requires;
- a **run**: consecutive one-row fact statements of one predicate, the common instance-file shape. Its rows keep their
  order, but item-level counts change: grounding observers see one rule-instantiation phase per block, not per fact.

Wherever the code discovers, stages, interns or emits a fact rule's head, the block's row is discovered, staged,
interned or emitted at the same point. This one rule makes support publication, atom numbering, root order, model
enumeration and output identical, so stages are accepted on byte-identical output, not on equal answer families alone.
The type is `FactBlock` (`formula_facts.rs`), reached through `Item::Facts`.

### Representation

- **The fact arena.** Facts travel from expansion into both routes in one columnar form, in shared vectors reserved
  fallibly and leased like the compiler's other storage, with no record per fact. A product is a predicate admitted
  once, a range of columns and a row count; each column is a range of segments, each segment a scalar cell or an `i32`
  interval. On the formula route a scalar cell is the value's admitted scalar key, admitted once per distinct value; on
  the lazy route it is the closed `Value` the core program interns.
- **Runs** store one column of cells per argument plus each row's evidence range, so a row of a unary run costs one
  cell and one range.
- **Families.** A family is the facts of one statement: a range of products and a range of evidence. Its origins are
  stored once, where each fact rule used to clone them. A family records its statement, and the retained source
  program holds that statement's carrier; a fact statement's normalized provenance is the stamp of that carrier, so it
  can be re-derived exactly without a copy per family.
- **Access.** `rows()` borrows a block and yields each row into fixed scratch. Admitted keys are `Copy` and interval
  cells yield numbers, so nothing is cloned or allocated per row, and a block can be iterated any number of times.
- **Items on the formula route.** The prepared rules are private storage, reached through `Prepared::items()`, the
  typed traversal (`Item::Rule` or `Item::Facts`), and `rules_without_facts()`, a separately named view for readers
  that provably never see facts, each use carrying its reason. Making the field private turned the compile errors into
  the complete reader inventory.

**Stage 1 as implemented departs from this form in two ways**, and the arena is conformed before Stage 2:

- interval values are admitted as scalar cells, one 8-byte cell per value, so that the vocabulary grows exactly as it
  grew one fact rule at a time;
- runs are stored row-major, because a run grows by appending one row per statement.

So `p(0..K-1)` holds K cells where the design holds one segment, and a numeric read of a column costs O(cells). The
conforming change keeps the vocabulary order, since admission order and storage are separate: an interval is stored as
one segment while each value is admitted when a row first reaches it, and a run appends one cell to each column
vector. The cell encoding is then confined behind a row accessor, so readers stop depending on it. Output stays
byte-identical.

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

**1. The summary.** It keeps every non-fact analyzed statement, in order, plus one representative fact statement per
fact signature (its first fact in source order). A layout records, in compile order, the runs of non-fact statements
and the elided fact families; `replace_constraints` filters and appends the layout as it does the statement list
today. The two programs are two types: `Summary`, the summarized program with its layout, which preparation analyzes
and from which `Owners` are collected; and `FullProgram`, the rebuilt program the public accessors return.
`ask_all` and `specialize` take `(&Summary, &Owners)`, since their statement loops index owners by position, so the
program each pass iterates is checked where it compiles.

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

- *Built from* the retained source program, the families and the post-replacement layout: each non-fact statement from
  the summary, each family row through `formula_analysis::fact` with its provenance re-derived by the stamp, ingested by
  `SourceProgram::of_nodes` so content-equal statements merge as now, then `Analysis::of`. The output is today's
  analyzed program at every level.
- *Built in preparation only for partition-eligible grounding programs*, the one preparation reader that needs it (the
  terminal partition). There it is built once, after constraint replacement, charged to preparation's budget as today's
  construction is, and moved into the owner's cell. Replacement runs once, on the summary: it drops the asked
  statements from the layout by identity and appends its one compile's replacements, so the layout describes today's
  post-replacement program and nothing is compiled twice.
- *Cost.* O(facts × arity) at the first call, then the full program's space for the owner's lifetime; today that space
  is paid on every run.
- *Failure.* The accessors stay infallible. Preparation performs every check the per-fact construction would make:
  `AnalysisNodes` and `AnalysisEdges` against full counts (rows × the row's node count per product; facts add no
  edges), and the structured-value bridge once per distinct value. The rebuild can then fail only at the
  infallible-allocation boundary today's construction already has.
- *Budget.* The on-demand rebuild charges no budget: preparation's budget has been reported, and the work is bounded by
  ceilings admitted at preparation (`Templates`, `Values`, `ScalarBytes`, and the analysis ceilings on full counts).
- *Documentation.* The accessors' comments state the first call's cost and that preparation analyzed the summary,
  whose graph and classes equal the full program's. `Debug` on the formula owners prints the summary's statement count and the
  families' row count, builds nothing, and does not depend on whether an accessor has run. The manual's statement that
  `source_analysis()` describes exactly the value `analyzed_program()` exposes still holds.

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

**One reader of facts in preparation: `PreparationFacts`.** Preparation owns one value every pass that reads facts reads
through, so the class that pays is the set of its callers, which the compiler lists. It holds the summary, its layout,
the families and the retained source program, and, once built, the fact atoms, the `FactIndex` over them and the full
program. It replaces `FixedFacts` (`formula_rewrite.rs`).

- *Triggers.* `fact_index(work)` builds the fact atoms from the families' rows on its first call, then
  `FactIndex::read_parts`; later calls borrow the index. Its callers are key analysis once it finds a keyed relation and
  specialization at its first qualifying anchor. `full_program(budget)` builds the full program from the
  post-replacement layout; its caller is the terminal partition.
- *Charges.* The fact atoms at the per-fact preflight's rates (`TermWork`, `ScalarBytes`) to preparation's expansion
  budget; the full program as today's construction is charged; `read_parts`' steps under `KeyWork`.
- *Refusals.* A key-work stop keeps today's mapping at each call site (`KeyAnalysis::Stopped`; `Failure::Optional` in
  specialization). An expansion-budget refusal while building the fact atoms or the full program refuses preparation
  at either call site and is never absorbed as a stop. The two outcomes are distinct types in the reader's result.

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
    rows and refuse at today's statement, reporting a template index or a fact-row locator;
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
the restrictions reader's uniqueness; the dictionary gives argument bounds and head coordinates once per distinct
value; equality-identifier columns give head coordinates per row and the compounding designs their columns; ordered
runs give the restrictions reader its canonical order without a sort. The store's bytes fall under
`AdmissionLimits::max_bytes`; the writer is given the remainder plus the vocabulary's bytes, so the vocabulary is
counted once. A program without facts has no store: `Program::fact_catalog()` returns `None`, and its workspaces bind
exactly as today.

**Whole-program traversal.** `Program::rules()` yields templates and fact rows in program order, and
`Program::is_empty()` means no rules of either kind. With `templates()` removed during the change, every call site
fails to compile and is assigned to `rules()` or `templates()` with its reason. Readers of facts: closure emptiness
(`is_empty()`), bootstrap (iterates `rules()`), argument bounds (a fact column's distinct positions added once before
the fixed point), head coordinates (a fact row's dense coordinate from its columns' positions),
`unconditional_facts` in `crates/zetesis-cpu/src/oracle/restrictions.rs` (fact rows read once from the catalogs in
canonical order and merged with the template facts it still deduplicates), the host source scan and eager grounding
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
  (`shared_closed_catalog_bytes() − shared_vocabulary_bytes()`) from every workspace after the first, so the store is
  counted once and the vocabulary as today.

**Public contracts and migration** (CHANGELOG): `Program::new(templates, FactTables::default(), limits)`; `templates()`
lists the non-fact templates on every door; `rules()` and `is_empty()`; `fact_origins()`, `fact_sites()` and
`AdmittedProgram::fact_statements()`, parallel to the fact rows; `Program::fact_catalog()`; `max_templates` documented
as bounding "source-normalized rules: templates and fact rows" (the count is today's on every program the extended
route builds); a fact-row locator on `AdmissionError` beside `template_index()`, which each door maps through its own
fact evidence; `Program::storage_bytes()` counting the template catalog, metadata and the store's rows and index; and
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
and each signature's tuples are its facts' arguments in program order. Key analysis reads producer counts only to ask
whether a choice head has exactly one producer; a signature with facts has at least one producer in both programs, so
the answer is equal. Wherever both programs' passes complete, the same keys are found and the same constraints asked
and rewritten. Where a ceiling binds, the summary's loops spend less and can reach further, and every rewrite is sound
by its module's argument, so answer families are kept. The rebuild produces today's post-replacement statements in
order, with the same provenance by the stamp formula and the same ingestion.

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
  and their `Origins` charge, pattern rows and joins (Stage 1); the per-fact analysis preflight and owners (Stage 2);
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
  by the pattern rows and rises by the arena's lease (statement evidence at 24 B per site, cells and run ends, a 64-byte
  record and a 24-byte column per argument per block), each growth admitted per block before allocation. By
  deterministic receipts the largest support peak rises on 82 of 133 programs (those with few facts per block, at most
  5.5%, median 1.9%), falls on 35 (every fact shape, 3.2% on `wide` to 20.3% on `frange`) and is equal on 16. Grounding
  observers see one rule-instantiation phase per block.
- **Stage 2.** The analysis preflight's per-fact `TermWork` and `ScalarBytes` are charged only where per-fact
  statements are built inside preparation, so they fall elsewhere; `AnalysisNodes` and `AnalysisEdges` are unchanged;
  owner charges fall to the summary's statements; `PreparationFacts`' fact atoms are charged where key analysis or
  specialization reads facts, never more than today's per-fact statements; `TermWork` falls by `ask_under`'s unit and
  specialization's steps per elided statement; the key-work stop points are reached later or not at all, and where a
  ceiling binds today, `key_analysis()`, `keyed_constraints()` and the rewritten constraints can grow. Elided statements
  are not charged to keep receipts equal: that would charge work not performed.
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

## Open review items

1. `FactIndex::read_parts` over the families reads a different program from `read` over the full program, which is
   deduplicated and in canonical content order. Have `PreparationFacts` produce the canonical fact sequence once, for
   every consumer, and state its cost.
2. The on-demand rebuild has no named resolver from the arena's cells to the values it rebuilds; name it, with its
   lifetime.
3. Give the full program one decidable trigger, name its owner across the preparation and materialization boundary,
   and state what the terminal owner's analysis reads.
4. One canonical fact sequence should serve the three consumers (`FactIndex`, the rebuild and the terminal partition),
   with the reader's lifecycle stated once.
5. State the conforming change's cost model: readers of interval values, the growth of column-major runs, and the
   direction of the support-storage charge.
6. "Product" names two different things in this design; give each its own name.
7. The keyed measurement shape, as written, never reaches the fact read; replace it with one that does, and check its
   route.
8. Name the commit Stage 1's acceptance run measures, and verify every cited site at the branch head.
