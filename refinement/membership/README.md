# Packed-membership refinement

This optional package proves a property of the Lean body extracted from the
actual `zetesis_ferraris::Interpretation::contains` method. It uses
[Charon](https://github.com/AeneasVerif/charon) and
[Aeneas](https://github.com/AeneasVerif/aeneas). It is separate from the solver's
Lean 4.33.1 semantic library. The Rust harness participates in the ordinary
development gates; the Aeneas/Lean checks below remain optional.

## Claim

For every machine-sized atom index, `Membership.contains_refines` proves that
the extracted method returns the corresponding stored bit when the atom is in
the theory's universe, and returns false otherwise. The assumption
`Membership.Represented` requires enough words to store every declared atom.
It does not assume the membership result. Unused padding bits need not be zero.

The proof follows the source operations: read the atom count, check the universe,
divide the index by 64, read the word, reduce the bit position modulo 64 and test
its mask. `Membership.mask_bit` in `Mask.lean` connects the mask operation to
`BitVec.getLsbD`. `Membership.outside` proves the short circuit
without any storage assumption.

## Boundary

Lean checks the generated body against Aeneas's models of vectors, scalars and
results. The concrete `Arc` model in `Harness/TypesExternal.lean` stores a value;
its dereference in `Harness/FunsExternal.lean` returns that value. It supplies no
membership conclusion. Machine-sized integers use the Aeneas/Lean platform
width; the recorded extraction was produced on AArch64 macOS. Reference
counting, allocation, destruction and
concurrent access are not modeled here.

The connection from Rust to the checked body trusts the pinned Rust compiler,
Charon and Aeneas translations, and the correspondence of these library models
to Rust. This package does not prove the Rust constructor establishes the
storage invariant, machine-code correctness, the complete reduct checker,
grounding, candidate enumeration or WGSL execution. The three theorem audits
use only `propext`, `Classical.choice` and `Quot.sound`; there are no added axioms
or admitted proofs.

## Contents

- `rust/src/lib.rs` calls the production library method without replacing it.
- `harness.llbc` is the Charon intermediate representation.
- `Harness/Types.lean` and `Harness/Funs.lean` are unedited Aeneas output.
- `Harness/*External.lean` supplies the concrete immutable `Arc` model.
- `Membership.lean`, `Mask.lean` and `Audit.lean` contain the proof and audit.
- `provenance.json` identifies tools, translation options and source spans.
- `source-inputs.sha256` identifies the selected repository source inventory;
  `artifacts.sha256` identifies the extraction and checked package sources.

The generated files retain Aeneas's formatting/linter settings. Authored Lean
files are checked with implicit variables disabled and warnings treated as
errors. No compiler, backend library, downloaded dependency or cache is vendored.
The package and generated translations of zetesis code use the repository's MIT
license. Aeneas, Charon, Lean and their dependencies retain their upstream
licenses; the links above identify their sources.

## Check the retained extraction

Prerequisites are Lean 4.31.0 through elan, Git, `curl`, `tar`, and a SHA-256
checker. These commands use the pinned macOS ARM64 release. Other platforms need
the matching upstream release or a build of the same commits; this package does
not qualify those tool binaries.

Run from the repository root of a separate checkout at the
[retained package revision `983e5ba9`](https://github.com/GregoryGelfond/zetesis/commit/2619c31f370ab23e97d4a8718be6245f50d3eb23).
The [revision map](../../docs/book/reference/source-revisions.md) relates its
recorded identifier to the published checkout. Its selected Rust source bytes
match `sourceRevision`
`0798e1a6e770d88610104dab4d7ed44cae1c86a8` in `provenance.json`; both inventories
identify that historical extraction. Later source renames, lockfile changes and
README clarifications can make checks against a current checkout fail. Preserve
the recorded hashes; qualifying changed source requires a new extraction and
proof check.

```sh
shasum -a 256 -c refinement/membership/source-inputs.sha256
cd refinement/membership
shasum -a 256 -c artifacts.sha256
mkdir -p .lake/aeneas
curl -fL https://github.com/AeneasVerif/aeneas/releases/download/nightly-2026.09.09-505b6ca/aeneas-macos-aarch64.tar.gz -o .lake/aeneas.tar.gz
printf '%s\n' '45b049359c76dc7b818836391ca9e53c0a0355d3aac35ffeea4170f158ce471b  .lake/aeneas.tar.gz' | shasum -a 256 -c -
tar -xzf .lake/aeneas.tar.gz -C .lake/aeneas
MATHLIB_NO_CACHE_ON_UPDATE=1 lake resolve-deps
```

`sha256sum` can replace `shasum -a 256`. The checked-in Lake manifest pins all
Lean dependencies. The optional cache fetch below retrieves the direct Mathlib
imports of the pinned Aeneas backend and their dependencies. It must run from
this package directory; `MATHLIB_CACHE_DIR` keeps the downloaded cache local.

```sh
MATHLIB_CACHE_DIR="$PWD/.lake/mathlib-cache" \
  xargs lake exe cache get < mathlib-modules.txt
lake build
for source in Harness/TypesExternal.lean Harness/FunsExternal.lean Mask.lean Membership.lean Audit.lean; do
  lake env lean -DautoImplicit=false -DwarningAsError=true "$source" || exit "$?"
done
```

## Repeat the Rust extraction

The retained body was translated using Rust `nightly-2026-08-18`, Charon
`b104e24fea7d721b71e6c39fd70f26ff20bc0980` and Aeneas
`505b6ca35217e7be5c96c3e2f8045edfbdf47291`. Install that Rust toolchain with the
`rustc-dev` and `rust-src` components. This does not change the repository's Rust
pin. Check source hashes before extraction. A source change requires a fresh
extraction and proof check; an old artifact hash is not a correctness argument
for changed code.

From this directory in the retained package checkout, after preparing the tools
above:

```sh
repository_dir="$(git rev-parse --show-toplevel)"
package_dir="$PWD"
mkdir -p target/replay
(
  cd rust
  RUSTUP_TOOLCHAIN=nightly-2026-08-18 \
  CARGO_TARGET_DIR="$package_dir/target/rust" \
  RUSTFLAGS="--remap-path-prefix=$repository_dir=zetesis" \
  ../.lake/aeneas/charon cargo --preset=aeneas --sysroot default \
    --start-from zetesis_refinement_harness::membership \
    --include zetesis_ferraris --dest-file ../target/replay/harness.llbc \
    -- --lib --locked
)
.lake/aeneas/aeneas -backend lean -dest target/replay -split-files \
  -namespace ZetesisExtract -all-computable -no-progress-bar -sequential \
  -warnings-as-errors -abort-on-error target/replay/harness.llbc
cmp Harness/Types.lean target/replay/Types.lean
cmp Harness/Funs.lean target/replay/Funs.lean
```

Rust's path remapping produces the public `zetesis/crates/...` source spans;
the generated files are not rewritten afterward. LLBC metadata may vary across
compiler targets, so the comparisons check the generated definitions as well
as the retained artifact's own hash. Do not copy generated `*_Template.lean`
files into the checked package: those external placeholders are replaced by
the explicit concrete model. After intentionally accepting a changed body,
repeat the strict proof and axiom checks above.
