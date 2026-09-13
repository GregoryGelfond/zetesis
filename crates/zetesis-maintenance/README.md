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

## Coverage and source identity

`coverage` separates floor policy, version observations, exact physical test
selection and libtest output checks from execution. It retains the existing
`toolchain.json` schema and independent workspace/CPU-only CLI populations. No
floor or filename filter is changed. The shell driver keeps the exclusive lock,
fresh cleanup, ordered commands and incomplete status until both floors pass.
After both reports exist, both floor commands run independently; `floors.tsv`
retains their profile names and exit statuses, including failures. This does not
change either floor or make a below-floor portable population pass.
The sixteen physical groups contain 56 exact tests; the Rust checker rejects
selection drift, zero matches and incomplete individual outcomes.

`scripts/maintenance.sh` enters the repository and selects Cargo 1.97.1 before
launching the current source-tree command. Set
`ZETESIS_MAINTENANCE` to an explicit prebuilt executable for frozen engineering
checks; it is an executable path, not shell text. Version strings are observations
supplied by the caller, not evidence that this library executed those tools.
Coverage metadata requires the actual cargo-llvm-cov version observation and
validates its pin; a direct library caller cannot omit that observation.
`inventory::sources` hashes the declared Cargo/Rust/WGSL boundary; this is not a
complete dependency or source-to-binary seal.

Policy input and physical logs have a 16 MiB inclusive ceiling. LLVM executable
identity reads have a 256 MiB ceiling. Proof/source reads expose their own limits;
these describe bytes and traversal work, not allocator RSS. Semantic inventories
refuse symbolic links. Confined recorded-file paths do not establish a filesystem
snapshot or protection against concurrent replacement by another process.

```sh
cargo test --locked -p zetesis-maintenance --all-features
cargo clippy --locked -p zetesis-maintenance --all-targets --all-features -- -D warnings
```

The `test-fixtures` feature enables a deterministic Rust subprocess stand-in.
Tests execute the actual shell drivers with those stand-ins and inspect command
order, profiles, status publication, failures and cleanup; they do not compile
programs, execute Lean, collect coverage or exercise Metal. Real compiler,
coverage and device qualification remain separate gates.

On Linux and macOS, subprocess tests reuse `zetesis_validation::process` through
a test-only dependency. Each invocation has a 20-second polling deadline and a
four MiB combined output ceiling, followed by bounded cleanup. A stopped capture
fails the test; an unresolved direct child is explicitly reported. This does not
prove descendant termination or add support for other process backends.

## Prepare proof-library views

`proofs::inventory` reads bounded current sources and returns typed declarations,
module identities and source hashes. Its `theorem_index` and `audit_source`
methods render deterministic views without writing files or reporting success.
The scanner is the same restricted scanner used by record verification.

```sh
scripts/maintenance.sh proof-inventory --proofs-dir proofs
scripts/maintenance.sh proof-inventory --proofs-dir proofs --view index
scripts/maintenance.sh proof-inventory --proofs-dir proofs --view audit
```

These commands print observations or proposed index/audit text. Publication is a
separate caller operation. After changing those files, collect fresh identities
and run the actual pinned Lean checks before producing a verification record.
The inventory neither generates `verification.json` nor copies assurance flags
from an old record. A complete command-executing record refresher is not provided.

## Current manual libraries

`book::select` reads one bounded Cargo JSON stream, requires successful completion
and named roots, and selects only current rlibs and procedural-macro libraries.
`Libraries::publish` checks regular confined files and unique basenames, then
hard-links the set into a fresh destination. Existing views are never overwritten;
a linking failure leaves an incomplete view that must not be used. This validates
the artifact list, not compiler execution or concurrent filesystem stability.

The book gate uses this library through `book-libraries`, keeping its incremental
build directory while excluding stale libraries from rustdoc's search path.
The caller must leave artifacts immutable during the check. The serialized input
and path-count limits bound representation size, not artifact payload size or RSS;
hard-link publication does not read or duplicate those payloads.
