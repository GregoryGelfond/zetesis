# Candidate restrictions, probing and projection indexing

Every optimization in this note acts on a classical candidate query. The stored
Ferraris theory, candidate truth mask and proper-subset reduct acceptance remain
the semantic authority. No domain identifier or corpus metadata is inspected.

## Transactional candidate constraints

`StableModels::restrict_candidates` accepts a constraint-only `Theory` whose
semantic atom indices have the same meaning as the original theory. Full gate
equivalences append it to the candidate CNF. Earlier restrictions and exact
semantic blocks remain, while the outer watch/trail cursor restarts against the
new fixed prefix. An encoding failure restores the previous variables, clauses,
submitted shape counts and live cursor. Work charged before failure remains.

This API can intentionally exclude stable models. Its exhausted flag therefore
means complete coverage of the constrained candidate region. A separate optimizer
may use a verified incumbent to exclude strictly worse costs. It must preserve
equal costs if it promises all optimum ties, retain independently verified models,
and never treat a classical assignment as an incumbent. Closed iterators,
carrier-count mismatches and pending blocking failures are explicit errors.

## Failed-literal probing

The first candidate region skips this optional precheck. After a successful
candidate restriction strengthens the query, the fresh cursor probes after
initial unit propagation and before choosing the branching permutation. It
visits original semantic variables in index order. For each
unassigned variable it trials false and true. Each trial uses the existing unit
propagator and then undoes its entire assignment suffix. Watch movements remain
valid under the same principle as chronological backtracking. A unit conflict
under the trial literal proves that every satisfying candidate has its opposite;
only that opposite may become a permanent root assignment. It is propagated
before continuing. An interrupted probe discards the incomplete cursor state.

Probes do not open decision frames. Trial propagation, conflicts, scanning and
undo contribute to the existing counters and poll cancellation/deadlines. No
new default limit is introduced. This scheduling policy uses the candidate-query lifecycle, not domain names
or source shapes. It avoids initial probing whose setup may cost more than it
saves. The direct CNF solver and fresh inner reduct queries retain their previous
search procedure. A one-pass probe need not find
every forced literal and is not guaranteed to improve every theory.

## Exact complete projection index

Each block still exists as a canonical CNF clause containing exactly one signed
literal for each original semantic atom. The indexed cursor validates that
shape, then inserts the clause's falsifying Boolean key into a binary trie. A
complete leaf is rejected exactly when its semantic prefix is already indexed.
Keys never include auxiliary variables, so multiple auxiliary extensions are
filtered consistently. Zero atoms use the empty key and empty blocking clause.

Every appended block is indexed once; lookup examines at most the semantic width.
The trie holds at most one node per admitted blocking literal plus its root.
Its arena grows fallibly under the existing CNF shape bound, and all insert/lookup
steps charge work. On successful candidate restriction the old blocks are already
part of the fresh base CNF; a new empty suffix index is created. They need not be
copied into the new trie because the base solver already enforces them.

## Bounded evidence

The unchanged pinned `scenarios/task-allocation/variant-04/05-larger-mix.lp`
case had 190 atoms, 4,303 formula nodes and 547 roots. With the exact verified
incumbent bounds, per-query gate sharing and the same 100,000,000 work ceiling:

| Candidate search | Optimum ties retained | Work | Coverage |
| --- | ---: | ---: | --- |
| Retained cursor, linear block scans | 627 | 100,000,000 | Incomplete |
| Root probing, linear block scans | 1,105 | 100,000,000 | Incomplete |
| Root probing, exact projection trie | 1,176 | 72,695,161 | Exhausted |

The final run made 5,003 decisions, checked 1,177 classical candidates and the
same number of frozen reducts, found zero countermodels, and installed two
candidate restrictions. All 1,176 retained ties have cost 5. These measurements record the earlier always-probe policy and
explain the general transform; no workload constants enter its implementation.
Full corpus validation is a separate integration record. No wall-clock speedup
or universal complexity improvement is claimed.

The later lifecycle policy retains probing only after successful restrictions.
A matched all 94 cases comparison preserved complete native full-model/optimum-tie
sets and reduced total SAT work from 150,970,627 to 145,537,716. Work decreased on 74
cases, stayed equal on 13, and increased on 7; the largest increase was 23,836.
The last task04 case remained complete at 72,651,587 work. These are scheduling
tradeoffs, not a universal improvement theorem.

In 21 paired warm scratch-harness runs matching the default CLI control policy,
SEND median process time decreased from 108.17 to 102.68 ms, while source admission stayed
about 82.2 ms. Search time decreased from 21.51 to 16.43 ms. The dominant source-admission
cost remains separate. These scratch timings do not replace installed executable
benchmarks; phase instrumentation was identical in both compared binaries.

## Checks and Lean boundary

Portable tests compare complete initial and refined/probed traversal with all canonical CNFs through
two variables, and indexed versus linear filters with free auxiliary bits and
empty keys. They cover both failed polarities, trial trail restoration, two-sided
conflict, nonexact block refusal, every work cutoff of a small probed traversal,
interruption during insertion, fused errors, and successful/failed restriction
restarts. Existing generated Ferraris DAG, renaming and independent minimal-model
differentials retain the original acceptance contract.

`IndexedCandidates.lean` proves executable recursive trie insertion/lookup equals
all exact blocking clauses, preserves unseen keys and treats equal projections
identically. It also proves that an actually refuted classical trial branch
permits forcing the opposite literal without changing the query. The latter is a
certificate law, not a proof that Rust unit propagation produces that certificate.
`ObjectiveBounds.lean` proves scalar incumbent dominance and complete optimum-tie
coverage using a verified original stable model. It does not verify priority
vectors, source tuple semantics or the bound-formula compiler. Neither module
refines the Rust arena, watch/trail state, undo, SAT encoding or resource accounting.
