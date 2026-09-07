# Brave and cautious consequences

Status: accepted version 1.1 target, not implemented. Consequence computation
will be a library capability with CLI and machine-readable views. It continues
to use answer sets established through the original program and its reduct.
Set operations do not replace stable membership checking.

## Semantic object and evidence

For a program P and a finite requested atom set Q, define brave consequences as
the atoms of Q belonging to at least one answer set of P, and cautious
consequences as the atoms of Q belonging to every answer set of P. These are the
union and intersection views of the answer-set family. Potassco uses the same
distinction in its [enumeration modes](https://potassco.org/guide/).

The native result should distinguish complete consequences, partial knowledge
and an inconsistent program. If P has no answer sets, the mathematical union is
empty and the intersection relative to Q is Q by vacuity. Preserve that fact in
the specification; present the inconsistent-program outcome explicitly so a
consumer cannot mistake vacuous caution for conclusions supported by a model.
An interrupted search with no answer set yet is a different outcome.

Record whether the selected family contains all answer sets or only globally
optimal answer sets. The all-answer-set mode must not inherit an optimization
mode that discards non-optimal models. The optimal-family mode needs established
optimality and the same objective priority ordering as ordinary solving; a
current incumbent cannot establish a consequence of all optimal answer sets.
Aggregate semantics, source parameters and program identity are also part of
the query identity.

Full atom identity is the semantic boundary. `#show` remains an observation view,
and shown terms need not be atoms. A future consequence operation over displayed
terms would need its own explicit semantics. Selecting Q from output signatures
does not justify discarding hidden rules, constraints or possible witnesses.
Strong negation retains signed atom identity. Default negation is not silently
turned into a positive atom in the result.

## Streaming reference algorithm

The first implementation can consume verified answer sets without retaining the
entire enumeration. For a nonempty observed family E, maintain

    B_E = union of (M intersect Q), for M in E
    C_E = intersection of (M intersect Q), for M in E

Each new answer set can only enlarge B_E and shrink C_E. Until complete coverage
is established, B_E is a lower bound on bravery and C_E an upper bound on
caution. Membership in C_E alone is not a proved cautious consequence. A model
containing an atom supplies a brave witness; a model omitting it supplies a
counterexample to caution. Do not initialize an empty observation as an ordinary
completed intersection result: carry whether any model has been observed.

Per-batch OR and AND reductions are associative, commutative and idempotent over
a shared atom catalog, permitting Rayon and wgpu schedules. Additional reduction
storage can be O(|Q|), independent of the number of models; this says nothing
about candidate-search or lazy-grounding memory. Device kernels may update
consequence state only from accepted models, including exact host completion of
any unresolved GPU membership checks. Failed or unfinished batches cannot
certify coverage.

With lazy grounding, Q must be explicit or established from a complete finite
query domain. Dynamically discovered atoms cannot be assumed to have appeared
in earlier models. A growing catalog must initialize each new atom against the
already observed family consistently. Candidate, source and query-domain
coverage are distinct obligations; a finished bitset reduction establishes none
of them by itself.

## Searches that establish consequences directly

Full enumeration is a useful reference implementation, not an efficiency
requirement. Search the original answer-set family under additional candidate
restrictions, retaining the original frozen-reduct acceptance condition:

- To establish bravery of a, seek an answer set containing a. Exhaustion of
  that restricted family proves absence of a from the brave consequences.
- To refute caution of a, seek an answer set omitting a. Exhaustion proves
  caution, with the base-program consistency outcome still explicit.

One accepted model can settle many atoms. A larger query can require at least
one previously unseen atom to be true, or at least one remaining cautious
candidate to be false. A successful model then makes measurable progress; a
complete failed search can settle the remaining set. The precise search
restriction, source-coverage obligation and termination measure belong in the
Lean specification before optimization. A stopped restricted search leaves
those atoms unresolved, even when other queries completed.

This is established territory to study: Alviano and colleagues describe
[targeted cautious searches and algorithms using minimal models and unsatisfiable cores](https://arxiv.org/abs/1804.08480).
Earlier work develops [anytime cautious reasoning](https://arxiv.org/abs/1405.3546).
These are sources of algorithms to assess, not evidence that zetesis has an
unsatisfiable-core API or that their implementations transfer unchanged. Here,
every accepted witness must still satisfy the original reduct-based contract.

Parallel restricted searches may share immutable source and formula structure
and batch membership queries. Each keeps its own restrictions, budgets and
coverage; redundant searches, data movement and synchronization can erase the
benefit. Class certificates may justify cheaper consequence calculations for
particular programs, but need their own applicability and preservation laws.
Efficient bitset reduction alone does not establish efficient consequence search.

## Version 1.1 delivery and qualification

Version 1.1 targets exact brave and cautious reasoning through the Rust API and
CLI, with truthful partial-result semantics and qualified CPU, Rayon and Metal
execution. The complete-enumeration fold is the reference; targeted-search
optimizations are evaluated against it. This target does not change version
1.0's required lazy-Metal execution or assign the separately proposed alternative
aggregate semantics to a particular release. No release date is implied.

Design typed query, scope, progress and outcome values first; the CLI composes
them. Do not label a consequence set as an answer set. Expose proved facts,
refutations and unknowns separately, with optional witness evidence and explicit
completion. Avoid a public API that requires callers to parse solver output.

First prove the finite-family folds, bounds, batch composition and restricted
search laws in Lean. Keep Rust/shader refinement and lazy-source completeness
as separate obligations. Then compare streaming results with independent complete
enumeration and clingo's brave/cautious modes on bounded fixtures, including
inconsistency, unique models, choices, signed atoms, hidden displays, optimal
ties, interruptions and dynamic catalogs. Benchmark enumeration and targeted
algorithms separately on CPU, Rayon and physical GPUs. Retain queries, actual
membership work, transfer bytes, peak memory, completion and source identities;
do not infer an enumeration count from the number of consequence-search witnesses.
