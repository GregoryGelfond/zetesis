# Dependency and ordered-probe proof audit

The complete library has **702 kernel-checked theorems in 53 semantic modules**:
eight new `AggregateConsumers` laws, six `StructuredWitnesses` laws and two
`OrderedProbes` laws. The prior 686-theorem verification record, index, umbrella,
audit requests/output and README are preserved byte for byte as `*.before.*`
files. Earlier verification directories were not changed.

All fifty prior semantic module files retain their recorded SHA-256 hashes.
Every prior theorem retains its name, source location and transitive axiom set.
The new module contents were not edited during this integration.
[Source continuity](prior-source-continuity.json) and the
[theorem/axiom comparison](continuity-check.json) retain the checked identities.

The actual pinned toolchain identified itself as Lean 4.33.1, commit
`819816b2e0a3bf405af45ae5c7af2491d8f5bee6`, on
`arm64-apple-darwin24.6.0`. [commands.json](commands.json) records exact arguments,
working directories, UTC starts, elapsed times, exits and logs for:

- Version identification, `lake clean` and a fresh `lake build`.
- A separate strict compilation of `Zetesis/OrderedProbes.lean`.
- Strict individual compilation of every one of the 53 semantic modules.
- The complete strict `Audit.lean`, requesting every indexed theorem.

The individual checks and audit use `-DautoImplicit=false -DwarningAsError=true`.
All commands passed. The audit has 702 unique entries and only the existing
standard logical axioms `propext`, `Quot.sound` and `Classical.choice`; some
theorems need no axioms. There are no project axioms, proof holes or native
evaluation shortcuts. The current `axiom-audit.txt` is byte-identical to this
run's [audit.log](audit.log).

The existing proof-record checker, its fifteen mutation/shell-propagation
regressions and `sh scripts/check.sh proofs` all passed. Their actual commands and
logs are listed in [check-commands.json](check-commands.json). The checker validates
record consistency and source/artifact identities; the Lean commands perform the
kernel checks. Neither substitutes for the other.

The mathematical scope is explicit:

- Aggregate consumers assume an already ready instruction order and a completed
  covering proposal carrier. Accepted proposals keep their computed value
  association, while each clause retains the original aggregate formula and
  static filter. Proposal membership is not logical truth. The frozen identity
  law assumes equal computed values.
- Positive structured witnesses use supplied finite rows, constructor-shape
  checks and named-slot constraints. The complete source atom remains the
  logical witness, and the completed condition binding and antecedent remain in
  scope. Collection replacement requires identical complete row membership.
- Ordered probes assume an immutable predicate with an initial true segment.
  Binary search decreases finite interval width and proves an exact partition.
  Two certified boundaries retain all full matches under an explicit implication
  from full matching to the key predicates. This is storage-order reasoning,
  independent of ASP term order.

The laws do not prove Rust scheduling or source dependency/safety inference,
actual source/aggregate/support coverage, scalar machine evaluation, matcher
traversal and rollback, comparator correspondence, allocation/work accounting,
cancellation, or physical device execution. No theorem establishes completion
after a resource stop. Proof counts describe the mathematical inventory, not an
implementation correctness score.
