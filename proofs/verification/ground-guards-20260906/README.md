# Fully evaluated ground guards — proof record

This addition registers 12 laws in `Zetesis/GroundGuards.lean`, bringing the
package to 496 audited theorems across 31 semantic modules. The inputs are
atom-free expressions of already total Boolean values. The main replacement
law preserves every frozen M/J interpretation pair and stable models in an
unchanged theory context. Source comparison evaluation, undefined arithmetic,
finite-width operations, safety and binding, complete substitutions and Rust
implementation refinement are outside this contract.

`commands.json` retains successful strict module checking, clean build, complete
strict axiom audit and pinned Lean version output. The current audit was copied
byte-for-byte to `proofs/axiom-audit.txt`. Current source, generated artifact and
command-log hashes are in `proofs/verification.json`; the repository-root
`python3 scripts/proof_record.py` gate passes with 496/31/36.

The prior 484-theorem manifest is retained as `verification.before.json`, with
its former index, Audit, umbrella and README alongside it. They are historical
snapshots and are not asserted to match the new source inventory. No existing
semantic module was changed. The only semantic source addition is GroundGuards;
the umbrella, index and Audit register it.

The separate direct-clingo investigation is retained under the workspace's
`work/ground-guard-review/`: 92 exact sources, actual executable hashes before
and after, raw JSON and diagnostics. It establishes observed whole-chain sign
scope, independently bound filtering, source-binding boundaries and undefined
arithmetic behavior. This oracle evidence does not constitute a source compiler
or kernel proof.
