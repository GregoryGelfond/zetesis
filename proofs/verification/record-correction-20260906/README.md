# Proof verification record correction

Readiness review on 2026-09-06 found inconsistent metadata in the published
batched-formula checkpoint. The top-level count of 484 theorems was correct;
the nested consistency counts still said 467, three generated-artifact hashes
referred to the earlier checkpoint, and the log hash map omitted the current
batch-accounting commands' logs.

The original record is preserved byte-for-byte in
[`verification.before.json`](verification.before.json). The reproduced mismatch
is recorded in [`detected-inconsistency.json`](detected-inconsistency.json).
All 35 recorded proof source/configuration hashes matched. The current theorem
index, source locations, Audit commands and printed audit all contained 484
theorems, and the current axiom audit exactly matched the retained successful
batch-accounting audit. This was an assurance-record defect; no proof statement,
Rust source, shader or installed solver binary was changed by the correction.

The corrected `proofs/verification.json` refreshes all three artifact hashes,
the four nested counts and the current log identities. Historical log hashes
remain retained and checked. Its `metadata_correction` field links back to this
record. The proof README now documents the record consistency gate.

The fresh [`commands.json`](commands.json) records a successful `lake build` and
strict `Audit.lean` invocation. Its [`audit.log`](audit.log) is byte-identical to
the current `axiom-audit.txt`, which allows only standard Lean logical axioms.

The new `scripts/proof_record.py` is a read-only consistency check, run after Lean
by the repository's `proofs` and `full` check modes. It checks the complete
declared source inventory, artifact/log hashes, theorem/source index, audit
names, allowed axioms and derived counts. Unit regressions exercise corruption,
including the stale hash/count failure class. It is deliberately not a Lean
kernel, generalized Lean parser or implementation-refinement proof.

The jointly frozen 14-group checkpoint remains historical evidence for the
unchanged runtime and proof sources. Its original fingerprint record is not
rewritten to pretend that the inconsistent metadata had been checked correctly.
The subsequent readiness supplement records the changed documentation, metadata
and check tooling, their validation, and the unchanged installed binary hashes.
