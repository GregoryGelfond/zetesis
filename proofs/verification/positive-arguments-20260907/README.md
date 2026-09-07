# Positive-argument proof registry and audit

The library now records **686 kernel-checked theorems in 50 semantic modules**.
The nine additions are the previously reviewed `PositiveArguments` laws. The
prior 677-theorem index, 49-module manifest, umbrella, audit requests/output and
documentation are preserved as `*.before.*` files in this directory. The old
execution-tranche verification directory remains unchanged.

All 49 prior semantic module files retain their exact recorded SHA-256 hashes;
all 677 prior theorem names, source locations and statements are unchanged.
Every prior theorem also retains its complete transitive axiom set. The new
module uses only the existing standard Lean logical axioms `propext`,
`Quot.sound` and `Classical.choice`; there are no project axioms, proof holes or
native-evaluation proof shortcuts. `prior-source-continuity.json` records the
unchanged source hashes, and `continuity-check.json` records the completed audit
comparison.

Actual Lean 4.33.1 commands ran in the integration worktree. `commands.json`
retains exact arguments, working directory, UTC start, elapsed time, exit and log
for version identification, `lake clean`, `lake build`, strict individual checks
of all 50 modules, and the complete `Audit.lean`. The individual checks and audit
use `-DautoImplicit=false -DwarningAsError=true`. Every command passed.
The current `axiom-audit.txt` is byte-identical to this run's `audit.log`.
The repository's existing `scripts/proof_record.py` passed after the manifest
refresh, checking all source/artifact hashes, declaration locations, unique
audit names, allowed axioms and current command-log links.

Positive-argument laws treat independently supplied environments and complete
finite support rows as premises. Exact equality filters retain original atoms
and preserve their original/frozen contexts. They do not prove Rust source
readiness, arithmetic inversion, join completeness, source/capture layout,
failure scheduling or resource accounting. No prior theorem or implementation
was refactored during this registry refresh; proof counts are inventory, not an
implementation correctness score.
