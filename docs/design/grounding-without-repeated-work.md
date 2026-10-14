# Grounding without repeated work: unaffected rich producers and shared local join plans

Support completion computes the possible atoms in rounds over the producer rules; final emission then joins every
rule once over the completed support and builds the formula theory. This note removes two repetitions inside that
process and records a roadmap for two more. Each stage is an execution choice that preserves the support fixed point
and the emitted theory exactly; the [grounding chapter](../book/architecture/grounding.md) describes the schedules.

## Status

| Part | Status |
|---|---|
| Stage 1: one registration per producer, prepared once; an unaffected rich producer is not traversed | Implemented, with the receipt `support_unchanged_visits` |
| Stage 1's receipt `support_rich_traversals` | Decided; not yet implemented (Open review items) |
| Stage 2: one plan per local-join site per snapshot, owned by the snapshot, consulted only through `Join::local` | Designed; open |
| Stage 2's receipts `local_plan_preparations` and `local_plan_reuses` | Decided with stage 2 |
| Stage 3: share a rich producer's traversal between support and emission | Roadmap |
| Stage 4: one ordered pass on the direct route | Roadmap |

Stages 1 and 2 leave the support catalog, the emitted theory and the answer output unchanged; the statistics output
changes by design (new receipts, lower work where traversals or plans are saved). Paths are under
`crates/zetesis-themelios/src/` unless another root is named.

## Problem

Measured sample shares, support / emission, at one worker before stage 1:

| Program | Rounds | Support | Emission | Assignment values, support / emission | Choice, support / emission |
|---|---|---|---|---|---|
| traveling-salesman variant 05 case 05 | 12 | 37.0% | 17.4% | 3.4% in all | 13.1% / 2.6% |
| shortest-path variant 04 case 06 | 8 | 31.5% | 16.0% | 11.3% / 1.8% | 2.4% / 0.5% |
| nested Mastermind | 5 | 54.0% | 27.4% | 35.2% / 11.9% | 0.5% / 0.5% |
| sudoku | 3 | 12.6% | 66.4% | — | support's rule work |
| shortest-path variant 01 case 01 (tiny) | 4 | 12.3% | 9.3% | 2.0% in all | — |

1. **Rich producers re-traversed with unchanged inputs (stage 1).** A producer plan, which wakes only rules whose
   inputs changed, exists only for positive-flat programs without objectives (`formula_support/producers.rs`,
   `formula_domains/positive.rs`). Every maintained eager encoding with a choice rule runs `Schedule::all`, which
   visits every producer in every round. A *rich* producer (a choice, disjunctive or conditional head, or an
   aggregate, conditional or projection in its body) took a full traversal on every visit, even when nothing it reads
   had changed: its body join, every per-binding element join and aggregate proposal, and every head, each already
   supported. Every visit also re-classified the rule (`delta::variants` scanned the body, about 0.9% of the TSP
   case's samples). Facts are not producers: a fact block is interned in round 1 and has no producer slot.
2. **Per-binding local join construction (stage 2).** Each outer binding of a choice rule, conditional head or
   assignment aggregate builds a fresh `Join` and plan per element, although within one snapshot the plan is the same
   for every binding of one site with one bound-slot pattern. `Plan::new` + `with_plan` take 5.2% + 4.3% (TSP case),
   5.9% + 7.8% (nested Mastermind) and 1.9% + 1.4% (tiny shortest-path), whole-process shares that include outer
   plans.
3. **One populated traversal per rich producer in each pass (stage 3).** Support derives a rich producer's possible
   heads and emission builds its formulas over the same rows.
4. **The direct route joins every rule twice (stage 4):** support completion and exact activity, each about a third
   of prime-sieve(300)'s samples.

## Stage 1 (implemented)

**Registration.** On the full-schedule path each producer gets one registration, prepared before the first round and
retained for the completion. It records once what the per-visit classification derived:

- a **certified** producer (normal head, no projected, conditional or aggregate body literal) registers its positive
  body occurrences, as the producer plan's prepared inputs do;
- a **rich** producer registers its **read set** `R(r)`: the distinct predicates of every atom occurrence, of either
  sign, in its body and in its head elements' conditions, at any depth (aggregate elements and keys, conditional
  literals' conditions and consequents, projections' predicates and witness bindings);
- a **constraint** is recorded as such; `Schedule::all` never yields it.

The walk is one enumeration of the atom occurrences a frame holds (`formula_support/occurrences.rs`), exhaustive over
the IR's head, literal, consequent, projection and aggregate-key forms with no wildcard arm, and shared with
`Demand::of`, so the two consumers cannot disagree. A read-set entry is a compiled coordinate, deduplicated by sign,
name and arity; the registration's bytes count as the catalog's prepared bytes for the completion.

**The test.** Round 1 traverses every producer. In later rounds a certified producer takes its registered inputs'
delta variants without the per-visit scan, and a rich producer is traversed exactly when some predicate of `R(r)` has
new rows in the round's snapshot (`old_rows < row_count`). The test stops at the first changed predicate and charges
one unit per entry tested plus the resolution's unit. `support_unchanged_visits` counts the rich-producer visits that
ended at the test. A test-only, thread-local switch restores full traversal for in-process reference runs.

**Correctness.** A traversal of `r` in round `i` reads (a) the relations of `R(r)`'s predicates through its outer
join, element joins, assignment proposals and group validation, and (b) the support membership of the atoms it
derives, only to prune a supported head or inside `retain`. Its plan depends only on `R(r)`'s row counts, and nothing
else it reads varies between rounds (domain guards exist only for producer-plan programs; an assignment's required
tuples are fixed in the IR; integer comparison forms decide rows from their values; fact rows are published once). If
`r` was traversed in round `j` and is unaffected through round `k`, then `S_k` and `S_j` agree on `R(r)` (publication
sets old rows before appending), membership only grows, and a round-`k` traversal would visit a subset of round `j`'s
bindings, discover nothing and retain nothing. Skipping it leaves every pending delta, identity, publication and the
fixed point unchanged; completion is still detected by an empty delta. Round 1 traverses every producer, which a
producer with only negated body literals (`{a} :- not b.`) needs.

The premise the code must keep is that `R(r)` covers every relation (a) lists; tests protect each kind of member.

**Lean.** `proofs/Zetesis/ProducerScheduling.lean` is generalized over stable producers: a producer family has an
abstract proposal function; `selective_step_exact_of_stable` and `empty_selection_closed_of_stable` add a stability
premise (an unaffected producer proposes nothing over the current carrier it did not propose over the old one);
`stable_of_reads_only` proves stability for a producer whose proposals depend only on the carrier's restriction to its
registered predicates, when the carrier grows. The earlier theorems keep their statements as corollaries. Rust supplies
the instance: the read set is the registration, snapshots grow because publication only appends, and the reads-only
property is the correspondence obligation above.

**Accounting** (work receipts measure the work performed):

| Receipt | Stage 1 | Stage 2 |
|---|---|---|
| `support_production_work` | Falls with skipped traversals and the removed scan; rises by at most two units per read-set entry per rich visit after round 1 | — |
| `support_join_work`, `support_head_work`, `join_rows`, `binding_snapshots` | Fall with skipped traversals | — |
| `support_producer_visits` | Unchanged (every visit is recorded before the variants) | — |
| `support_unchanged_visits` | Rich-producer visits that ended at the test | — |
| `local_plan_preparations`, `local_plan_reuses` | — | Plans built and inserted by `Join::local`; joins over an entry |
| Expansion `TermWork` | Falls with skipped traversals | Falls by `Plan::new`'s charges per reuse; rises by lookups and insertions |
| `support_peak_bytes` | Rises by at most the registration; falls where a skipped traversal set the peak | Rises by at most the plan table |
| Work and byte ceilings | Either direction under explicit limits; no refused↔completed flip at the default limits | The same |

Summed over a completion, the candidate's formula work exceeds the baseline's by at most the preparation plus
`Σ max(0, 2|R(r)| − |body(r)|)` over rich visits after round 1 (a skipped traversal can cost less than its test, for
example `{q(X) : a(X), b(X), c(X)} :- t.` reads four predicates with one body literal).

**Measured effect** (deterministic receipts, no timing): `support_unchanged_visits` 30 (TSP case), 18 (shortest-path
variant 04), 8 (nested Mastermind), 3 (sudoku), 2 (n-queens 8), 4 (tiny shortest-path); support production work
−59%, −63%, −57%, −34%, −33%, −26%; join work −56%, −74%, −63%, −50%, −51%, −37%; rounds, producer visits and peak
bytes unchanged. End to end at one worker on a quiet host: nested Mastermind −27.5%, the TSP and shortest-path cases
about −12%, sudoku −7%. Answers and text output are identical on the corpus, scalability and example programs, also
with the formula route forced.

## Stage 2 (open): one plan per local-join site per snapshot

**The plan's inputs.** `order::Plan::new` reads the literal slice, the prefix's length and bound slots, the variable
count, the snapshot's component view, and each positive pattern's relation row counts (local joins take the full
variant, no pivot). Within one snapshot the row counts do not change: a support round derives into its pending delta
and publishes only at the round's end, and the completed support is immutable. So within one snapshot a plan is a
function of the slice's contents, the variable count and the bound-slot pattern.

**Owner and lifetime.** The snapshot owns the plans: `Support` (`formula_support/queries.rs`) gains a second lazily
created workspace beside its shared table indices, dropped with the snapshot at the end of each round and after
emission. A plan therefore cannot outlive the snapshot it was planned against.

**Served sites.** Local joins get their own constructor, `Join::local`, which consults the table; `Join::element` and
`Join::local_head` build through it, and nine sites that build local joins with `Join::new` today switch to it (in
support: assignment values; in emission: projection witnesses, aggregate elements, conditional literals; arithmetic
admission's local joins; objective presence's local join). `Join::new` never consults the table, so outer rule joins,
outer variants, prepared rules, objective joins and the factoring component join (which re-decides its plan through
`owned_plan`) keep building their own plans.

**Key and entries.** The key is the borrowed slice itself (`&'source [LiteralIr]`, by address and length) with the
variable count; an entry records the bound-slot pattern its plan was built for. Empty frames bypass the table. A site
has at most one entry per snapshot; a binding with another pattern builds an owned plan and leaves the entry. A join
holds a counted reference (`Rc`) to a shared plan, which makes `Support` neither `Send` nor `Sync`, so the table cannot
cross a thread. The table's cell is borrowed only for a lookup or insertion, so a nested local join can fill it while
earlier plans are lent. A failing plan is never inserted; its failure is raised at its first binding.

**Account.** The table charges the expansion budget's `TermWork`, as `Plan::new` does: one unit per search step, per
bound slot compared and per entry moved on insertion. Retained plans and the entry array join the snapshot's live bytes
and are admitted against `max_support_bytes` when inserted.

**Correctness.** A shared plan yields the same traversal, rows and order as a fresh one, and an entry is used only
when the slice, the variable count and the pattern all match, within the snapshot that owns the table.

**The alternative considered.** Preparing each element's plan once per visit (in support) or once per rule (in
emission) and passing it to its sites: no key, lookup or retained table, but a plan handoff along about a dozen call
paths, each resting on its own caller. The snapshot table was chosen: one entry point and one test surface, with its
invariants held by types.

**Tests.** For every served site of the maintained programs, a shared plan equals a fresh one and a join over it yields
the same rows in the same order; a differing pattern builds its own plan; a failing plan is not inserted; a nested local
join fills the table while an earlier plan is lent; an entry made in round `i` is absent in round `i+1`; an empty frame
makes no entry; the component, objective and outer joins never consult the table; a test-only counter of found entries
asserts the route independently of the public receipts; the table's bytes appear in `support_peak_bytes`, and a ceiling
just below them refuses at insertion with `SupportBytes`.

## Stages 3 and 4 (roadmap)

**Stage 3, replay.** Support's base join prunes a subtree once the head is bound and supported, for every rule with a
normal head, so a support traversal's rows are complete only for producers that do not prune by head projection
(choice, disjunctive and conditional heads, and normal heads whose variable only an assignment generator binds).
Support also hands its consumer only passing rows, while emission validates failing rows too. A replay therefore
records below row selection, passing and failing, for producers inside both boundaries, and emission builds and
validates from the record instead of re-joining (about `rows × slots × 4` bytes per producer until emission). Atom
numbering, and so model order, may change; stage 3 is accepted on identical answer families. Emitting during support
is not proposed: support cannot know a traversal is its last before the fixed point.

**Stage 4, one ordered pass on the direct route.** For complete signed acyclic programs, rules in dependency order can
share each rule's join between support derivation, original-family arithmetic admission and exact activity, removing
one of the two joins per rule. It follows columnar fact blocks, which change how facts enter that route.

Each owes its own design and preservation argument.

## Acceptance (each stage)

On a quiet host, against the implementation's parent built as a release front, with the receipts re-measured there
first:

1. **Answer identity** on the corpus, scalability and example programs at one and fourteen workers (answer sets, costs
   and optimality verdicts); in-process, the support catalog and emitted theory identical.
2. **No refusal flip** at the default limits; a flip under another profile reported with both work totals.
3. **Estimates held to account** by the reach report (rounds, unchanged visits and rich traversals for stage 1; plans
   prepared and reused for stage 2).
4. **Whole-run gain:** end-to-end time falls with separated ranges (hyperfine, 25 runs, alternating) in every cell whose
   estimate is at least 3% of the run.
5. **No regression** above 3% with separated ranges in any grounding A/B, end-to-end or corpus cell.

## Alternatives considered

- Extending the producer plan to every program (its wake sets come from the positive-flat certificate).
- Semi-naive variants for rich producers (aggregate carriers do not decompose by occurrence).
- Recording all support bindings for emission (pruned traversals record incomplete rows).
- A single gringo-style pass (the end state; it moves the builder into the fixed point).
- Consulting the table inside `Join::new` (a new outer site would consult it silently).
- A dense site number in the IR (a compiler change at every site; the borrowed-slice key gives the same identity).
- Deriving `demand::read_predicates` from the shared walk (it changes the hybrid route's retained relations; needs
  its own measurement).

## Decisions

- Stage 1 reports `support_unchanged_visits`; rich traversals are counted by a public receipt,
  `support_rich_traversals`, not by a census outside the solver.
- A later stage may change atom numbering, and so model order, when answer families are identical.
- Stage 2's plan table is owned by the snapshot and consulted only through `Join::local`.
- Stage 4 follows columnar fact blocks.

## Open review items

- Implement `support_rich_traversals` (stage 1's decided receipt), and bind stage 1's evidence to the implementing
  commits.
- Stage 2: separate call site from frame (state the policy when one frame is planned from several call sites), key
  entries on exactly what `Plan::new` reads, and give stage 2's reach report a denominator.
- Hold, or guard with a check, the premise that the key covers every planner input.
- Keying by address makes the binary search's charges depend on heap layout: make the charges independent of layout,
  or state and accept the dependence; state the per-snapshot complexity and argue the structure.
- State the whole lifetime path of the snapshot-owned table, including the support path and the binding cursor, or
  drop assignment values from the served sites; restate the edits and type-held invariants against that path.
- Restate stage 1's correspondence obligation as its two parts (the read set covers the traversal's reads; pruning
  removes only heads already in the carrier).
- Give stage 2 an in-process reference and identity tests, and name the instrument for its per-site proposition.
