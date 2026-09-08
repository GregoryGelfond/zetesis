# Boolean evaluation and ranked-support composition

Lean 4.33.1 completed a clean build, strict checks of all **67 semantic modules**,
and a complete **783-declaration axiom audit**. `TightEvaluation` adds 11 laws.
All 772 prior theorem names, file/line locations and transitive axiom sets are
preserved. All 66 prior semantic module source files retain their recorded
bytes. These counts describe the audit inventory, not executable verification.

[commands.json](commands.json) records the actual commands, working directory,
UTC starts, elapsed times and exits. The pinned toolchain reports commit
`819816b2e0a3bf405af45ae5c7af2491d8f5bee6` on `arm64-apple-darwin24.6.0`.
`lake clean` precedes the build. Every semantic module and `Audit.lean` was
checked with `-DautoImplicit=false` and `-DwarningAsError=true`.
The audit uses only the established standard axioms `propext`, `Quot.sound` and
`Classical.choice`; no project axiom, proof hole or native-evaluation shortcut
was introduced.

The prior manifest, theorem inventory, umbrella imports, audit source/output,
README and reading guide are preserved as `*.before.*`.
[prior-source-continuity.json](prior-source-continuity.json) records exact prior
semantic bytes, and [continuity-check.json](continuity-check.json) compares
declarations and axiom sets. The current `axiom-audit.txt` equals this record's
[audit.log](audit.log). The proof-record checker, its 15 regression tests and
repository proof gate have separate records in
[check-commands.json](check-commands.json). Their consistency checks complement
kernel checking; hashes alone do not prove trustworthy execution.

## What is established

The [new module](../../Zetesis/TightEvaluation.lean) reuses the existing DAG
representation and proves that its Boolean prefix fold produces the mapped
original formula truth. The evaluator correspondence is derived by induction,
not supplied by the caller. Root and indexed-body consumers use that same table.
An absent producer body explicitly denotes truth; it is not a node sentinel.
The producer append law proves that independently reduced heads combine by OR,
without requiring unique heads or disjoint producer lists.

Exact head/body-formula links to the admitted producer grammar and complete
present-atom carrier coverage establish supportedness. Original producer
membership and a strict positive rank then supply the existing ranked-support
stability argument. The completed-classification theorem composes those
computed checks with exact residual completion in the original theory.
The [reading guide](../../guide/tight-evaluation.md) describes the argument and
its assumption boundaries for an ASP practitioner.

## What is not established

The syntactic links, candidate carrier, original producer membership and rank
are certificate/compiler obligations. This development does not prove that
Rust constructs them or that a lazy source registry is complete. The inherited
mathematical falsum/false default makes decoding total; it does not permit
out-of-bounds access or replace runtime DAG admission and identity checks.

No correspondence for Rust arrays, fixed-width words, WGSL instructions,
workgroup initialization, atomic OR, barriers, deterministic witness selection,
ordered readback, work/memory bounds, cancellation or transport is proved.
Successful termination and physical GPU execution are not implied. The list
fold is a finite specification and does not establish GPU complexity or speed.
Residual exactness remains an explicit premise; stopped or incomplete work does
not acquire an acceptance certificate from this development.

No Rust source, solver run or performance campaign is part of this proof slice.
The original theory and its Ferraris reduct remain the permanent semantic basis.
