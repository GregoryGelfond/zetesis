# Aggregate bounds, evaluation storage and lazy-round proof audit

The complete library has **723 kernel-checked theorems in 57 semantic modules**.
This tranche adds four `AggregateBounds` laws, seven `CountEligibility` laws,
three `EvaluationPrefix` laws and seven `LazyRounds` laws. All 702 prior theorem
names, source locations and transitive axiom sets are unchanged. All 53 prior
semantic source files retain their recorded SHA-256 hashes; the four new modules
were not edited while integrating their imports and declarations.

The previous verification manifest, theorem index, umbrella, audit requests and
output, README and reading guide are preserved byte for byte as `*.before.*`
records. Earlier verification directories remain unchanged.
[Source continuity](prior-source-continuity.json) and the
[declaration/axiom comparison](continuity-check.json) record these checks.

## Pinned kernel checks

The toolchain identified itself as Lean 4.33.1, commit
`819816b2e0a3bf405af45ae5c7af2491d8f5bee6`, on
`arm64-apple-darwin24.6.0`. [commands.json](commands.json) retains exact arguments,
working directories, UTC starts, elapsed times, exit codes and output logs for:

- Toolchain version identification.
- `lake clean` and a fresh `lake build`, completing 60 build jobs.
- Strict individual compilation of all 57 semantic modules.
- The complete strict `Audit.lean`, covering all 723 indexed declarations.

The module checks and audit use `-DautoImplicit=false -DwarningAsError=true`.
Every command passed. The audit contains 723 unique names and only the existing
standard logical axioms `propext`, `Quot.sound` and `Classical.choice`; some laws
require no axioms. No project axiom, proof hole or native-evaluation proof shortcut
was introduced. The current `axiom-audit.txt` is byte-identical to [audit.log](audit.log).

The record checker validates source/artifact hashes, declaration locations and
audit correspondence; actual Lean execution performs kernel checking. The
subsequent [check commands](check-commands.json) retain the checker, its fifteen
mutation/shell-propagation regressions and the repository's complete `proofs`
gate separately. All three checks passed. A final read-only record check after
recording their log hashes also passed.

## Mathematical and implementation boundaries

`AggregateBounds` keeps both the aggregate equality and rule activation around
natural cardinality intervals. Active bounds reject outside candidates; inactive
instances impose no frozen obligation. A proposed value does not activate a group.
Actual bound evaluation, complete proposal coverage, distinct head identities and
source scope remain assumptions. No signed machine-bound or additional partial
search-pruning implementation is verified.

`CountEligibility` relates complete tuple and atom indices while retaining all
ordered eligibility witnesses, including duplicates. Selected tuple activity
equals the representative head conjoined with coalesced eligibility in original
truth and every frozen M/J pair; finite thresholds preserve this equivalence.
Complete distinct representatives, possible-support traversal and eligibility
preservation are explicit premises. Rust grouping/cursors, source safety, budgets,
weighted/extremal heads, general aliases and negative heads remain outside scope.

`EvaluationPrefix` relates finite partial-operation plans over a live prefix to
plans over a storage list with an explicit initialized length. Successful steps
extend the live prefix by one; complete evaluation preserves its result or first
error. Reset excludes all old cells. The laws do not verify operation validity,
machine arithmetic, operand indices, resource charging, allocation or Rust
drop/unwind behavior.

`LazyRounds` uses a shared union of immutable candidate-world snapshots only to
cover possible source instances. Each world still checks its own positive
antecedents and frozen gates. Fresh complete scans yield exact consequences and
constraint detection; fixed-snapshot chunks compose without cross-world truth
leakage. Coverage belongs to its exact snapshot. These laws do not verify Rust
traversal, atom IDs, budgets, WGSL, readback, publication or physical Metal use.

The original theory and its frozen reduct remain the solver's permanent
acceptance foundation. These are reusable fundamentals supporting eventual
solver verification. The theorem count is an inventory, not a claim that Rust,
GPU execution or the whole solver is formally verified.
