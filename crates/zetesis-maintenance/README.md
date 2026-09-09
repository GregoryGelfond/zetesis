# zetesis-maintenance

Repository assurance as a library, separate from answer-set validation. This
unpublished workspace package checks retained proof records without invoking
Lean or the solver. Applications call `proofs::verify`; the maintenance command
is a thin argument/output view.

```sh
cargo run --locked -p zetesis-maintenance -- proof-record --proofs-dir proofs
```

The checker admits plain line-leading theorem declarations, explicit namespace
and section scopes, and ASCII qualified names. It masks nested comments and
strings while retaining locations. It rejects unsupported declaration forms,
duplicate JSON keys, escaped paths, stale inventories, incorrect theorem
locations, incomplete axiom output and inconsistent command logs. File and
traversal limits are explicit in `proofs::Limits`.

Record consistency does not establish that commands ran. Run the pinned `lake
build` and strict `Audit.lean` command independently; neither hashes nor a passing
maintenance check replace Lean kernel checking or prove a Rust/WGSL refinement.
The synthetic regression fixtures make no claim of kernel acceptance.
