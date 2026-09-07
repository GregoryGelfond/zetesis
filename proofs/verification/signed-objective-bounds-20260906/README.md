# Signed objective candidate bounds — proof record

This addition registers 17 laws in `Zetesis/SignedObjectiveBounds.lean`, bringing
the package from 496 to 513 audited theorems across 32 semantic modules. It
reuses the existing complete-key objective normalization and incumbent-bound
contracts; no previous semantic module is changed.

After direction normalization and global complete-key OR coalescing, each
negative weight `w` contributes a fixed offset `w` plus a nonnegative term `-w`
on the complemented eligibility. The entire grouped list remains intact, with
no new deduplication after this transformation. The exact integer sum identity
preserves shifted `<`, `=`, `≤` and per-priority lexicographic comparisons,
candidate regions, original-theory optimum predicates and ordered tie lists.
All optima survive a verified incumbent's non-strict bound. Completed scalar
bounded coverage is reused through an explicit normalized-region premise.
A checked duplicate-key example distinguishes this from charging raw source
occurrences before coalescing.

The original Ferraris theory is unchanged. These classical candidate-query
identities do not justify rewriting signed aggregates in its frozen reduct.
The proof uses unbounded integers and supplied total Boolean eligibility;
finite-width negation, checked offsets and shifted ceilings, fallback selection,
source grounding, priority/presence discovery, graph/restriction compilation,
search completion, resource accounting and Rust refinement are outside scope.
The native implementation's `i32::MIN` and shifted-`i64` fallback boundaries
remain implementation obligations, not consequences of these arithmetic laws.

`commands.json` retains the successful strict module check, clean build,
complete strict axiom audit and pinned Lean version check. `audit.log` is
copied byte-for-byte to `proofs/axiom-audit.txt`. The current manifest hashes
all current proof/configuration files and generated artifacts, as well as
the retained command logs and prior record files. The read-only repository
proof-record consistency gate is recorded separately in `record-check.log`.

The prior 496-theorem manifest, index, Audit, umbrella, README and axiom output
are retained in the `*.before.*` files. They describe the prior source inventory;
they are historical records and are not asserted to describe this new module.
No Rust, sibling estate project, external corpus or backend source was edited
by this proof task.
