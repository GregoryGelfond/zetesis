# Name matched positive body atoms through the join's matched-row record

Final formula emission re-derives every positive body atom the join has just matched. This note designs the route
that names those atoms from the matched rows instead. It changes how emission names atoms, never which atoms or
formulas it emits. The [grounding chapter](../book/architecture/grounding.md) describes emission and the producer
route this design extends.

## Status

- **Stage A** (every emission join keeps the matched-row record; one function names a matched row's atom; one owner
  keeps the per-depth node cache): designed, not implemented.
- **Stage B** (a memo from atom identity to node): staged. It is considered only after Stage A is measured.
- Code is cited by path and function; paths are under `crates/zetesis-themelios/src/` unless another root is named.

## Problem

Emission instantiates each rule once per completed join row (`Builder::instantiate_rule`, `Builder::rule`,
`Builder::body` in `formula_ground.rs`). For every positive body literal, `Builder::atom` re-derives an atom the join
has just matched, with three lookups:

1. **Tuple interning.** `Computation::atom` (`formula_support/computation/atoms.rs`) projects the pattern's arguments
   from the binding, checks and hashes the tuple, and searches the discovery inverse for its position.
2. **Catalog position.** `Builder::atom_identity` asks the formula atom catalog for the atom's position.
3. **Node interning.** `Builder::node` interns `Node::Atom(index)`: a structural hash, a bucket probe and an equality
   check.

The join already holds the answer. `Join::next_base` matches a relation row against each depth's pattern
(`match_row` in `formula_support.rs`), and a join that keeps the witness record pushes the matched row. The row's
`atom()` is a canonical `AtomRef` of the support authority. The join plans exactly the body's ordinary positive atoms,
and each occurrence keeps its literal's index (`PatternOccurrence::source`, `formula_support/order.rs`). Today only
support production and the producer route use the record (`witnesses::eligible`, `formula_support/witnesses.rs`):
total flat positive bodies with a normal head and at least two atoms. Constraints, rules with negative literals,
comparisons, aggregates or conditionals, and choice and disjunctive heads take the plain path and re-derive every atom.

**Measured reach** (sampling profiles of the release CPU front at one worker; shares of all samples). The in-reach
column counts the rule's own body only, the part Stage A reaches:

| Class | In reach | Choice-element conditions | `Builder::atom` from every caller |
|---|---|---|---|
| sudoku | 15.3% | 1.6% | 19.4% |
| traveling-salesman, variant 05 case 05 | 3.0% | 0.2% | 4.4% |
| n-queens (scalability) | 1.9% | 0.3% | 3.0% |
| shortest-path, variant 04 case 06 | 1.0% | 0.2% | 3.2% |
| shortest-path, variant 01 case 01 | 0.3% | ≤ 0.05% | 1.3% |
| nested Mastermind, Mastermind | ≤ 0.05% | ≤ 0.1% | 0.2–3.1% |
| prime-sieve(300), large fact files | none | none | none |

Across these classes an atom's cost divides roughly into 50% the tuple projection, checks and hash (what the identity
import removes), 27% the import's retained work (entry admission, discovery-inverse search, insertion check, `refresh`),
21% catalog position and node interning, and 1% static pattern. The in-reach column is an upper bound: it counts
negative literals too, and owned completions name their deepest literal by the general route (Design 1). The profiles
predate the integer comparison forms and the support-completion skip, which shrank grounding elsewhere, so body atoms
are now a larger share of what remains; acceptance restates every estimate at the implementation's parent.

## Contract

**Preserved.** The emitted formula: atom catalog order and identities, node identities, roots, origins and activated
objectives; the theory, answers, output and model order. Every failure kind: Stage A adds no refusal kind. Its new
refusal sites are the record's reservation in `retain_witnesses` (support bytes, on joins that did not keep it), the
node cache's and inverse's reservations (support bytes), and a work-ceiling site at each new charge. Cancellation is
observed as before: every reused or imported literal charges a unit before it is named.

**Receipts** (work receipts measure the work performed):

| Receipt | Direction | Why |
|---|---|---|
| Grounding work | Either | The record charges one unit per push and per removed row on joins that did not keep it; an imported literal charges less than `Builder::atom`'s path; a reused literal charges one unit where the general path charges three lookups |
| `atom_lookups`, `node_lookups` | Fall by `row_atom_reuses` | A reused literal presents nothing to the catalog or to interning |
| `atoms_inserted`, `nodes_inserted` | Unchanged | Every atom and node is first named at the same point |
| `row_atoms`, `row_atom_reuses` | New | Planned positive literals named through their matched rows, and those reused from the unchanged prefix |
| Support bytes, `support_peak_bytes` | Either | The record, cache and inverse add O(body width) leased bytes per join and rule; a reused literal skips the import's `refresh`, which samples the owner peak |

**Bounded profiles.** A work ceiling can be reached earlier (the record's charges) or later (the replaced lookups). A
support-bytes ceiling can be reached earlier by at most the record's, cache's and inverse's bytes. A support-bytes
refusal observed only at a skipped `refresh` moves to the next check of the live total (allocation proposals are
admitted where they allocate; actual capacity is observed without a ceiling in `StorageLease::observe`), and does not
occur if no check follows. Acceptance lists every flip with its direction.

## Design

### 1. Every emission join keeps the witness record

- **The record.** `Witnesses { rows, unchanged }` (`formula_support/witnesses.rs`): one `MatchedRow` per matched depth,
  pushed when a row matches and the prefix filters pass, truncated on undo (`undo_at`), with `unchanged` lowered to
  the undone depth.
- **Who keeps it.** The plain emission loop and capture call `retain_witnesses` before traversal, as the producer
  route does. Its producer-eligibility assertions move to the producer route and to `witnesses::derive`, where
  eligibility holds; the record needs none of them.
- **Owned completion undoes the deepest depth.** `next_staged` completes a generated join, or one with head slots,
  with owned frames, and `resume` undoes the deepest depth at once. So a lent completion holds every planned depth
  when it is lent, while an owned completion holds every depth but the deepest: its deepest planned literal has no
  recorded row and takes the general route. A body with one planned literal and a generator gets no row route.
- **Lending.** Only a passing row carries a view of the record (rows by depth, the unchanged count, and the patterns,
  so depth maps to literal through `patterns[depth].source`). The record resets its count only when it lends a
  passing row, so a view's `unchanged` is the number of leading depths not undone since the previous passing lend.
  The view answers only for the depths it holds.
- **The lend invariant**, checked as a debug assertion keyed to the completion's ownership: a lent completion holds
  every planned depth, an owned completion all but the deepest.
- **Owned rows carry no view.** `next_owned_row` returns rows that outlive the traversal step; its consumers keep the
  general route.

### 2. One function from a matched row to its node

`Builder::matched_atom(row)` names the row's atom through the identity path: `Builder::atom_ref(row.atom())`, then
`Computation::atom_ref`, then `SupportAppend::discover_ref`. It adds no unit of its own: its unit is `atom_ref`'s, so
the producer route's receipts are unchanged when its two `atom_ref` calls become calls to this function. The import
skips the tuple's projection, checks and hash (for a same-store atom `identity_with` returns the canonical identity
without hashing) and keeps four steps: the entry admission, the O(log n) search of the discovery inverse, the occupied
insertion check, and `refresh`. The catalog position and node interning follow as today.

### 3. One owner of the node cache and its prefix rule

`MatchedNodes`, one per rule and consumer, owns the per-depth node cache (leased; it replaces the producer route's
`atoms` buffer) and, where literal order differs from depth order, the inverse from literal to depth (charged one unit
per literal). Its rule, for a passing row's view and a depth `d` the view holds: if `d < view.unchanged` and the cache
holds `d`'s node, charge one unit and return it (`row_atom_reuses`); otherwise call `matched_atom(view.rows[d])` and
record the node. Both count in `row_atoms`. The producer route's compiled groups keep their exemption from recording.

Consumers: the plain loop (`Builder::body` gains the optional view; every literal without a recorded row is named as
today, and its other callers pass none), capture (`capture_constraint` names its planned positive literals by the rule
and its negative literals as today), and the producer route.

### 4. Stage B: the memo

A memo maps an atom's identity to its node. A hit skips everything an imported literal still costs: the import's
retained search, insertion check and `refresh`, the catalog position and node interning.

- **Key.** The matched row's canonical atom identity, scoped to the support authority's owner. A relation holds each
  atom once in one snapshot, so the memo holds at most one entry per distinct atom named, at most `max_atoms`.
- **Owner binding.** The memo records its owner at its first entry and checks each key's owner against it; an atom of
  another owner takes the identity path unrecorded.
- **Representation and accounting.** A word-hashed index in the form of `nodes::Index`, count-bounded and unleased like
  its sibling `node_indices`: outside the byte allowance, bounded by `max_atoms` entries, growth moves charged as work
  before reserving, allocation failure typed `MetadataAllocation`. It adds no support-bytes flip.

### Composition

- **Columnar facts.** A fact block's rows enter support by the same atom operation as a fact rule's head, so a body
  occurrence over a fact predicate matches a support row like any other and Stage A names it unchanged, whatever the
  relation's storage. No special case for facts is needed.
- **Support completion's replay** (a later stage of [grounding without repeated work](grounding-without-repeated-work.md))
  would replay support traversals into emission. A replayed binding must carry its matched rows to keep this route; one
  that carries none takes the general route. Either way the formula is the same.

## Correctness

**Claim.** With Stage A, emission produces the same Builder state as without it at every point: catalog, node arena,
roots, origins, objectives and metadata. Only counters and peak receipts differ.

1. **A matched row's atom is the instantiated literal.** `match_row` binds the pattern's unbound variables to the row's
   columns and checks the bound ones by typed equality; a structural pattern emits the whole captured atom. Undo
   happens only on backtracking, or at once on an owned completion, which removes the deepest row from the record.
   Nothing after the match rebinds a variable a positive occurrence made safe: generators bind only unsafe variables,
   and head-slot cursors bind head variables beyond the body's.
2. **The identity path names the same position as the tuple path.** The support relations are views over the atom
   owner the appender writes, so `identity_with` takes its local branch, and both paths end at the same occupied
   discovery position (Lean `AtomCatalogs.identity_determines_position`).
3. **A reused node is the node interning would return.** A passing row's `unchanged` is the minimum, since the previous
   passing lend, of the depths the record was truncated to. If `d < unchanged`, no depth `≤ d` has been undone since,
   so depth `d` holds the same row and atom. The catalog and node table have only grown since (aggregate families
   append in a transaction and detach their suffix), and interning an existing node returns its earlier index (Lean
   `DagSharing.intern_existing_reuses`). The cache must be current: a cached node is used only while the rule's
   emission owns it.
4. **Order.** A literal is named where the general path names it, in literal order, so first occurrences and numbering
   are identical.
5. **The view answers only for the depths it holds.** The deepest literal of an owned completion is named by the
   general route, today's naming.
6. **Failure boundaries.** A reuse allocates nothing; it skips the import's `refresh`, which moves a slack-carried
   support-bytes refusal to the next check (Contract). Stage A refuses only at its reservations and charges.

Lean covers identity uniqueness and interning stability over its list model; matching exactness with the generator
premise, the passing-lend reset, the lend invariant and any memo index are Rust obligations pinned by the tests below.

## Costs

| Item | Work | Retained space |
|---|---|---|
| Witness record on joins that did not keep it | One charged unit per push and per removed row | Body width × `MatchedRow` per join, leased |
| Literal-to-depth inverse (where orders differ) | One charged unit per body literal, once per rule | One depth per literal, leased |
| Node cache | One store per imported literal | One optional node per depth, leased |
| A reused literal | One charged unit and one load | — |
| An imported literal | `atom_ref`'s unit, the import's admission, O(log n) inverse search, insertion check and `refresh`, then catalog and node | — |

**Expected gain** (shares of the run at one worker, estimates from attribution): Stage A without prefix reuse removes
the projection, checks and hash of reached atoms — sudoku about 8.2% of the run (10.3% of grounding), traveling-salesman
about 1.5%, n-queens 1.1%. With reuse fraction `r = row_atom_reuses / row_atoms` the gain is about
`reach × ((1 − r) × a + r)` with `a` ≈ 0.52–0.55. The memo's increment is at most `(1 − r)` times the import's retained
and catalog-and-node shares (sudoku about 7.1% of the run at `r = 0`).

## Tests

Each test states one proposition and asserts the route; a crate-internal, test-only switch disables the row route in
the same test binary, as the producer route's switch does.

1. `matched_rows_name_the_atoms_the_general_route_names` — a constraint whose two positive atoms recur; the emitted
   atoms, nodes, roots and origins equal the general route's, and `row_atoms > 0`.
2. `each_reuse_replaces_one_catalog_and_one_node_lookup` — the lookup counts differ by exactly `row_atom_reuses`;
   insertions equal.
3. `a_negative_literal_is_named_as_before` — `row_atoms = 0`, the formula equal.
4. `a_structural_pattern_names_its_captured_atom`.
5. `literals_joined_out_of_order_keep_their_own_rows` — join order unlike literal order; kills an inverse built in
   join order.
6. `failing_rows_keep_the_unchanged_count_until_a_passing_row` — kills a reset at a failing lend.
7. `a_generated_row_names_its_deepest_literal_by_the_general_route` — exact `row_atoms`.
8. `a_head_slot_row_names_its_deepest_literal_by_the_general_route` — exact `row_atoms`.
9. `a_generator_over_one_planned_literal_names_no_atom_by_its_row` — `row_atoms = 0`.
10. `a_streamed_constraint_retains_the_same_identities` — the hybrid schedule's capture.
11. `the_producer_route_keeps_its_compiled_groups_unrecorded`.
12. A property test over two-predicate bodies whose join order differs from literal order, with negative, structural
    and generated literals.
13. `scripts/check.sh portable` and `oracle`, answer families identical.

If the memo is adopted: `an_atom_of_another_owner_is_not_recorded` and
`a_memo_growth_charges_its_moves_before_it_reserves`. Each named mutation is applied once and must fail its test.

## Documentation and Lean

- The grounding chapter's matched-rows passage: the plain loop and capture name planned positive atoms from their rows,
  one function and one node cache serve every route, unchanged prefixes reuse nodes, owned completions name their
  deepest literal by the general route; restate its proof-boundary sentence.
- `WitnessRow`'s contract, `retain_witnesses` without the producer's assertions, and the lookup receipts' docs.
- A vocabulary entry for the route and its two receipts; a CHANGELOG entry with the receipts' directions.
- Lean `DagSharing`: add `intern_stable_under_extension` (interning in an extended table returns the earlier index),
  with a correspondence entry relating `nodes::intern` to `DagSharing.intern` and stating the Rust obligations.

## Acceptance

On a quiet host, one worker, binaries A (the parent) and B (with Stage A) identified by commit and SHA-256:

- Restate the estimates at A first (sudoku, n-queens, traveling-salesman variant 05, shortest-path variant 04).
- Grounding phase: interleaved A/B, 21 rounds, checking each run's exit status. End to end: hyperfine, 25 runs, in
  every cell whose restated estimate is at least 3% of the run.
- Cells: sudoku; n-queens (scalability and variants 01–04); traveling-salesman variant 05 case 05; shortest-path
  variant 04 case 06 and variant 01 case 01; nested Mastermind; controls Mastermind, prime-sieve(300) and a large fact
  file with a comparison rule.
- Pass if answers are identical on every program (a failed comparison, or any differing or erroring row, fails), if
  sudoku's end-to-end median falls by at least half its restated estimate with separated ranges, if no cell's median
  rises more than 2% with separated ranges, and if each cell reports its reach (`row_atoms / atom_lookups(A)`) and
  reuse fraction beside its timing, with the bounded profiles' flips listed.

## Alternatives considered

- A second matched-row record beside the witness record: two records of one join state. Declined.
- The identity path without prefix reuse: smaller, but it repeats lookups the record proves unchanged.
- Keeping the deepest row on an owned completion: changes owned completion's undo, which owned snapshots rely on.
- The view on element joins (choice-element and conditional-disjunction conditions): a later step, judged on Stage A's
  measurement (sudoku 1.6%, n-queens 0.3%).
- A memo keyed by relation and row position; `AtomIdentityMap` (its growth is neither proposed before the move nor
  charged); `CoordinateMap` (quadratic worst-case movement); dense per-relation vectors (memory grows with all support
  rows). Declined.

## Decisions

- The memo is staged after Stage A's measurement and adopted only if the measured remainder warrants it.
- If adopted, the memo is count-bounded and unleased, as `node_indices` is.
- `GroundingWork` gains the public receipts `row_atoms` and `row_atom_reuses`, printed in the grounding attribution.

## Open review items

- The shared rule's charges must agree with "the producer route's work is unchanged": either restate the producer
  route's receipts or make the shared rule charge exactly the producer's existing units.
- `row_atoms` and `row_atom_reuses` as specified also count the producer route, so the reach, reuse and lookup-fall
  statements misstate Stage A alone; scope the receipts or the statements.
- Correctness step 3 needs the cache-currency premise stated: a cached node is valid only while its rule's emission
  owns the cache.
- Smaller edits: restate the reach denominators, the test list's route assertions and several citations.
