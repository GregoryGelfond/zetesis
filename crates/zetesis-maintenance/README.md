# zetesis-maintenance

Repository assurance as a library, separate from answer-set validation. This
unpublished workspace package separates retained-record inspection from explicit
pinned proof execution. Applications call `proofs::verify` to inspect records or
`proofs::capture::capture` to execute and publish a current record. The maintenance
command maps arguments and output; neither operation invokes the solver.

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
The maintained proof gate now retains live Audit stdout/stderr and passes the
successful stdout to `proof-record --live-audit`. `proofs::verify_with_audit`
checks its exact correspondence with the retained record; nonzero execution,
Audit stderr or differing live bytes cannot pass. Failed audit output remains
visible and is retained under a fresh `target/proof-checks/run.*` directory.

## Coverage and source identity

`coverage` separates floor policy, version observations, exact physical test
selection and libtest output checks from execution. It retains the existing
`toolchain.json` schema and independent workspace/CPU-only CLI populations. No
floor or filename filter is changed. The shell driver keeps the exclusive lock,
fresh cleanup, ordered commands and incomplete status until both floors pass.
After both reports exist, both floor commands run independently; `floors.tsv`
retains their profile names and exit statuses, including failures. This does not
change either floor or make a below-floor portable population pass.
The fourteen physical groups contain 58 exact tests; the Rust checker rejects
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
cargo test --locked -p zetesis-maintenance
cargo clippy --locked -p zetesis-maintenance --all-targets -- -D warnings
```

The `zetesis-maintenance-fixture` binary supplies deterministic Rust subprocess stand-ins.
Tests execute the actual shell drivers with those stand-ins and inspect command
order, profiles, status publication, failures and cleanup; they do not compile
programs, execute Lean, collect coverage or exercise Metal. Real compiler,
coverage and device qualification remain separate gates.

On Linux and macOS, capture and subprocess tests reuse the existing
`zetesis_validation::process` owner. Gate-driver test invocations have a 20-second polling deadline and a
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
from an old record.

## Capture current proof evidence

After reviewing the generated index and Audit, use actual toolchain directories
and a new empty evidence directory outside the repository:

```sh
proof_evidence=$(mktemp -d "${TMPDIR:-/tmp}/zetesis-proofs.XXXXXXXX")
lean_bin=$(dirname "$(cd proofs && elan which lean)")
rust_bin=$(dirname "$(rustup which --toolchain 1.97.1 rustc)")
scripts/maintenance.sh proof-capture --repository . --evidence "$proof_evidence" \
  --lean-bin "$lean_bin" --rust-bin "$rust_bin"
```

The producer observes Lean 4.33.1's actual platform and commit, runs a clean Lake
build, strict module checks and the strict axiom audit, and runs the current
maintenance record regressions with Rust/Cargo 1.97.1. Before execution, it copies
the admitted maintenance executable into the external `tools` directory,
preserves its permissions and records original/frozen paths and equal hashes.
The frozen copy remains the checker owner if Cargo replaces the original command
path during those regressions. Its final hash must still match. Counts come from the current
restricted inventory. It verifies the staged record against unchanged source,
document and tool hashes and the actual audit bytes before publication. Tool
hashes identify executable files, not their entire loader closure or authenticity.
The selected executables must be trusted current builds; no retained hash alone
establishes that trust. This workflow covers theorem declarations recognized by
the inventory; proof-valued definitions remain outside its declaration census.

Each CLI command has a 300-second polling deadline, a 16 MiB combined stream
ceiling, two seconds of initial cleanup and a two-second cleanup retry. Library
callers supply the first three limits explicitly. Stopped or failed commands
cannot publish a record. An unreaped direct child is reported explicitly; capture
completion does not prove that arbitrary descendants terminated. Allocation and
OS calls are outside the polling-time guarantee.

Commands run through `env -i` with pinned tool directories first in PATH,
`LC_ALL=C`, offline Cargo, one build worker and the declared Rust/Lean toolchains.
Only HOME, temporary-directory variables, CARGO_HOME, CARGO_TARGET_DIR, RUSTUP_HOME,
ELAN_HOME and the explicit build-storage settings CARGO_INCREMENTAL,
CARGO_PROFILE_DEV_DEBUG and CARGO_PROFILE_TEST_DEBUG are forwarded in addition to
that PATH. Actual argv, environment
assignments, separate raw streams, exits and cleanup disposition stay under the
external `commands` directory; its `before` directory preserves the prior record,
audit and logs. A source copy and portable record views stay under `stage`.
Compiler flag variables are not silently inherited. Cargo configuration files
and transitive loader inputs remain the caller's separate build-provenance duty.

Publication owns `proofs/verification/.capture-lock` and replaces the log set,
audit and record using local renames. An ordinary failure restores moved
originals; a recovery failure reports both causes and the external archive.
`Phase::Cleanup` specifically means a new validated record was committed but
scratch/lock cleanup failed. Readers and other writers must stay out during this
exclusive transaction: it is neither a crash-atomic snapshot nor a defence
against concurrent edits. A stale lock after a crash requires inspection and
manual recovery using preserved evidence, not an automatic retry.

The log-copy passes admit at most 16,384 entries, 64 MiB per file and 512 MiB total
read bytes; executable reads admit 256 MiB each. These bounds do not describe RSS.
Synthetic producer controls exercise real bounded child ownership and publication
failure paths with declared stand-ins, and make no claim of Lean acceptance.

## Current manual libraries

`book::select` reads one bounded Cargo JSON stream, requires successful completion
and named roots, and selects only current rlibs and procedural-macro libraries.
`Libraries::publish` checks regular confined files and unique basenames, then
hard-links the set into a fresh destination. Existing views are never overwritten;
a linking failure leaves an incomplete view that must not be used. This validates
the artifact list, not compiler execution or concurrent filesystem stability.

The book gate uses this library through `book-libraries`, reusing the checkout's
ordinary Cargo target while excluding stale libraries from rustdoc's search path.
Other source checkouts and instrumented coverage populations keep separate targets.
The caller must leave artifacts immutable during the check. The serialized input
and path-count limits bound representation size, not artifact payload size or RSS;
hard-link publication does not read or duplicate those payloads.

## Installed tools and documented executables

`install::check` holds the installed tool set to one source, the lists in
`scripts/install.sh`: INSTALL.md's tool table must describe exactly those tools,
its Cargo commands must name exactly the installer's packages, and those
packages' binary targets, as `cargo metadata` reports them, must be exactly the
tools. `invocations::check` holds live documentation to what the workspace
builds: a command in a shell code block, or a tool table's first cell, that
begins with a zetesis executable must name a binary or package of the workspace,
and `zetesis bench` is not a command. A page beginning with `invocations::RECORD`
records a dated measurement and keeps the spellings of the binaries it records;
prose is not checked. Both take text their caller has read and build nothing.
The portable gate runs them over the repository in the `executable_agreement`
tests.
