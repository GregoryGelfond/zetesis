# Integrated language tranche proof record

On 7 September 2026 the integration worktree contains five newly imported
semantic modules: `CertifiedExecution` (6 theorems), `TrueHeads` (14),
`CountHeads` (11), `ValueExtrema` (18), and `FinitePools` (8). The complete
inventory is 613 theorems across 41 semantic modules, an increase of 57 laws
from the previous 556-theorem / 36-module record.

`theorems.json` was rebuilt with the existing `scripts/proof_record.py`
`declarations` helper, retaining the established qualified-name sort. Every module is imported exactly once by `Zetesis.lean`;
only blank separators in that umbrella were normalized. `Audit.lean` requests
all 613 indexed names in their recorded order. An initial audit in source order
also passed; its command and output are retained separately. The final strict
audit was repeated after restoring the established index order. No semantic module, Rust source,
check script or proof checker was changed by this refresh.

## Actual checks

`commands.json` retains the exact argument vectors, working directories, UTC
start times, elapsed times, exit statuses and log paths for the executed commands.
The pinned toolchain reports Lean 4.33.1, commit
`819816b2e0a3bf405af45ae5c7af2491d8f5bee6`, arm64-apple-darwin24.6.0.

1. `lake env lean --version` confirms the actual pinned compiler.
2. `lake clean` followed by `lake build` recompiles the complete package.
3. `lake env lean -DautoImplicit=false -DwarningAsError=true Audit.lean`
   audits every indexed theorem; `audit.log` is copied exactly to the current
   `axiom-audit.txt`. Only `Classical.choice`, `Quot.sound` and `propext` appear;
   there are no project axioms, proof holes or native-evaluation shortcuts.
4. `python3 -m unittest discover -s scripts/tests -v`, from the repository root,
   passes all 51 script regressions, including the proof-record checker tests.
5. `python3 scripts/proof_record.py`, from the repository root, checks source and
   artifact hashes, all source/index/audit names and counts, source line numbers,
   allowed transitive axioms and current command logs. It is run again after
   adding its first result to the completed manifest.

The current `proofs/verification.json` hashes all 46 source/configuration files,
the generated README/index/audit, current evidence files, and retained historical
logs. Hashes establish record consistency; the actual fresh Lean commands supply
the kernel-checking evidence. These checks do not prove runtime refinement or
physical device execution.

## Preserved history and scope

The six `*.before.*` files are byte-for-byte copies of the incoming Audit,
umbrella, theorem index, axiom output, README and verification manifest. They
retain the pre-refresh state, including the umbrella's already integrated five
imports and the earlier 556-theorem index. Every existing historical record
outside this new directory remains unchanged.

The contracts and their boundaries are described in the current proof README.
Finite pool and head laws assume completed source/eligibility families; count
head correspondence is checked over supplied complete rows. Complete-value
extrema requires its explicit comparator/selection law. Certificate completion
requires sound support and exact residual classification over the original
Ferraris theory. Source safety, coverage construction, Rust maps/cursors/counters,
resource accounting, scheduling and device execution remain separate obligations.
