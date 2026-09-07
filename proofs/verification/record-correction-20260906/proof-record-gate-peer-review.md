# Proof-record gate peer review — 2026-09-06

No unresolved concrete defect was found within the gate's stated metadata-consistency scope. This was an independent read-only review of the final script, regression tests, shell integration and corrected proof record. I computed file hashes and compared retained bytes; I did not execute the gate, tests, Lean, Cargo or device code.

The exact reviewed hashes are in `proof-record-gate-peer-review.json`. All three script/test/hook hashes match the author's final freeze. Every current recorded hash matches its file: 35 proof source/configuration files, three generated artifacts and 15 retained logs. The 35 proof source hashes are identical to the original record captured when the defect was discovered. The current audit is byte-identical to both the retained batch-accounting audit and root's fresh correction audit. The original faulty manifest is preserved byte-for-byte in the correction directory.

## Reviewed acceptance path

- `scripts/proof_record.py:51–71` requires canonical contained regular files, exact source/artifact inventory and valid matching SHA-256 values. Duplicate JSON keys are refused.
- `scripts/proof_record.py:74–171` masks nested comments and strings, restricts declaration/scope syntax, checks unique qualified theorem names and exact source file/line identity against the index. This is deliberately a limited source convention, not a Lean parser.
- `scripts/proof_record.py:174–205` checks all unique Audit requests, the ordered complete printed-name list and only the allowed transitive axioms. Unexpected output, omitted/extra/duplicate entries and nonallowed axioms fail.
- `scripts/proof_record.py:208–235` requires successful current command records, individually hashed retained logs, an actual recorded `lake build` command and one strict Audit invocation; that invocation's retained bytes must equal `axiom-audit.txt`. Historical hashed logs are also checked.
- `scripts/proof_record.py:238–269` reconciles exact integer counts, assurance flags, the current module inventory and the optional 17-law batch-accounting count. The current result is 484 theorems, 30 semantic modules and 35 source/configuration files.
- `scripts/check.sh:31–34` invokes the record gate only after successful Lean build and strict Audit checks in both `proofs` and `full`. `set -eu` prevents a failed step from being silently followed by later qualification. The shell regression uses explicit command stubs; it does not pretend to run Lean.

## Findings resolved during this review

1. The original Python equality check admitted `schema_version: true` as version 1. The final implementation requires exact `int` type; a malformed-record regression is retained.
2. The scanner originally accepted a standalone attribute line before a theorem despite claiming attribute-prefixed declarations were outside the admitted convention. The final implementation rejects source attribute-prefix lines, with both same-line and separate-line regressions.

The author retains 15 passing targeted test methods, all 42 passing Python methods, an actual current-record PASS and the original stale-record rejection under `work/proof-record-gate-validation/`. Those are author-run results. The fixtures deliberately model record consistency, not kernel-checked theorems; several corruptions recompute hashes so the tests also exercise structural validation rather than stopping only at stale hashes.

## Boundaries

The gate establishes internal agreement among supported source declarations, index, Audit requests/output, counts and retained file identities. It does not establish that command records are authentic, that the recorded compiler executed those bytes, or that Lean proves the Rust/WGSL implementation. It does not replace actual pinned Lean execution. Additional descriptive fields and arbitrary import semantics are outside its narrow validation contract. Pathological malformed inputs can still terminate with an ordinary nonzero Python error; no such path reviewed manufactures a PASS.

The initial readiness report and metadata-detection artifacts remain unchanged. They correctly preserve the defect as unresolved at the time of detection; this separate review records the later correction and prevention gate.
