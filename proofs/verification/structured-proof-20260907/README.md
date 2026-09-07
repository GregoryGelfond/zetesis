# Structured proof pilot — 7 September 2026

Refactored only the body and documentation of
`Zetesis.CertifiedExecution.completed_membership_exact`. Its declaration and
assumptions are byte-for-byte unchanged, as recorded in
`statement-preservation.json`; the prior module is retained beside it. The pilot
uses named scoped case obligations and explicit final composition. It adds no
public theorem, definition, axiom or external dependency.

A fresh Lean 4.33.1 clean build and strict complete audit passed for all **613
theorems in 41 semantic modules**. The complete old inventory, audit, umbrella,
README and verification manifest are retained. The refreshed index preserves
qualified theorem names and updates source lines. The source-hash checker verifies
the exact current source/configuration files and retained evidence; it is not a
replacement for the Lean build or a verified statement-equivalence tool.

The new proof convention and mathematical reading guide are documented under
`proofs/STYLE.md` and `proofs/guide/`. Independent review checked the worked
argument against the formal statement and caught one omitted supporting premise
in the prose description of a neighboring theorem; that premise was made
explicit before this record. No theorem assumptions were changed.

This pilot changes no Rust or shader source. Concurrent telemetry/JSON work has
separate implementation and regression evidence. The proof retains its original
semantic scope: completed membership composition under supplied soundness and
exact-oracle premises, not termination, source coverage, publication, Rust or GPU
refinement. No library-wide style conversion or novelty is claimed.
