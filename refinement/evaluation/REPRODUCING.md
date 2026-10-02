# Reproduce the evaluator refinement

Run these commands from `refinement/evaluation` in a checkout whose selected
Rust files match `source-inputs.sha256`. The package uses Lean 4.31.0, Charon
`b104e24fea7d721b71e6c39fd70f26ff20bc0980` and Aeneas
`505b6ca35217e7be5c96c3e2f8045edfbdf47291`.

## Prepare and check

Install the Lean toolchain through elan. The commands below download the pinned
macOS ARM64 Aeneas release; another platform requires its corresponding upstream
release or the same source commits. This record qualifies no other tool binary.
Use `sha256sum` in place of `shasum -a 256` where appropriate.

```sh
(cd ../.. && shasum -a 256 -c refinement/evaluation/source-inputs.sha256)
(cd ../.. && shasum -a 256 -c refinement/evaluation/semantic-inputs.sha256)
shasum -a 256 -c artifacts.sha256
mkdir -p .lake/aeneas
curl -fL https://github.com/AeneasVerif/aeneas/releases/download/nightly-2026.09.09-505b6ca/aeneas-macos-aarch64.tar.gz -o .lake/aeneas.tar.gz
printf '%s\n' '45b049359c76dc7b818836391ca9e53c0a0355d3aac35ffeea4170f158ce471b  .lake/aeneas.tar.gz' | shasum -a 256 -c -
tar -xzf .lake/aeneas.tar.gz -C .lake/aeneas
MATHLIB_NO_CACHE_ON_UPDATE=1 lake resolve-deps
MATHLIB_CACHE_DIR="$PWD/.lake/mathlib-cache" \
  xargs lake exe cache get < mathlib-modules.txt
lake build
lake env lean -DautoImplicit=false -DwarningAsError=true Audit.lean
lake env lean -DautoImplicit=false -DwarningAsError=true SharedAudit.lean
```

The Lake manifest pins dependency revisions. The `Zetesis` library target reads
only the required semantic modules directly from `../../proofs`; it does not
change the main library's toolchain or import its compiled artifacts. All authored
files compile with implicit variables disabled and warnings treated as errors. Only `propext`,
`Classical.choice` and `Quot.sound` may appear in the audit. The retained
`verification.json`, `axiom-audit.txt` and `shared-axiom-audit.txt` record the
checked artifact hashes and commands, separately from the main semantic library's gate.
The default build includes the generated evaluator and root scan, their
composition into theory satisfaction, and the checked boundary examples. These use the documented
fixed-token external model; the build does not establish its correspondence with
concurrent Rust execution.

## Repeat extraction

Install Rust `nightly-2026-08-18`, including `rustc-dev` and `rust-src`. This is
an extraction toolchain, not a change to the solver's ordinary Rust pin. The
extractor selects the real private production evaluator and root scan directly;
no new Rust wrapper is needed. The extraction runs offline; if dependencies are
not cached, first run `cargo fetch --locked --manifest-path ../../Cargo.toml`.

```sh
repository_dir="$(git rev-parse --show-toplevel)"
package_dir="$PWD"
mkdir -p target/replay
RUSTUP_TOOLCHAIN=nightly-2026-08-18 \
CARGO_TARGET_DIR="$package_dir/target/rust" \
CARGO_BUILD_JOBS=2 \
RUSTFLAGS="--remap-path-prefix=$repository_dir=zetesis" \
.lake/aeneas/charon cargo --preset=aeneas --sysroot default \
  --start-from zetesis_ferraris::oracle::evaluate \
  --start-from zetesis_ferraris::oracle::failed_root \
  --include zetesis_ferraris --include zetesis_cpu::cancellation \
  --dest-file target/replay/evaluator.raw.llbc \
  -- --manifest-path ../../crates/zetesis-ferraris/Cargo.toml --lib --locked --offline
```

Use an owned build directory and retire it after retaining source/tool hashes,
extraction and proof evidence. Source changes require a new extraction and proof
check; old hashes cannot qualify changed code.

## Normalize names and compare

The destination path in LLBC is extraction metadata. Set it to a portable name.
Two local debug names are renamed to avoid generated namespace collisions. All
three changed fields are checked explicitly; executable operations remain
unchanged. These commands require `jq`:

```sh
jq -cae '.translated.options.dest_file = "evaluator.llbc"' \
  target/replay/evaluator.raw.llbc > target/replay/evaluator.source.llbc
jq -e --slurpfile raw target/replay/evaluator.raw.llbc '
  .translated.options.dest_file == "evaluator.llbc"
  and ((.translated.options.dest_file = $raw[0].translated.options.dest_file)
       == $raw[0])
' target/replay/evaluator.source.llbc
jq -cae '
def checked_argument($id; $name; $count; $type):
  .translated.fun_decls[$id] as $function |
  $function.def_id == $id
  and $function.item_meta.name == [
    {"Ident":["zetesis_ferraris",0]},
    {"Ident":["oracle",0]}, {"Ident":[$name,0]}]
  and $function.item_meta.is_local == true
  and $function.item_meta.opacity == "Transparent"
  and $function.body.Structured.locals.arg_count == $count
  and $function.body.Structured.locals.locals[1].index == 1
  and $function.body.Structured.locals.locals[1].ty == $type
  and $function.body.Structured.locals.locals[1].name == "theory";
if checked_argument(11; "evaluate"; 5;
     {"Value":[64,{"Ref":[{"Body":1},{"Deduplicated":4},"Shared"]}]})
   and checked_argument(12; "failed_root"; 3; {"Deduplicated":64})
then .translated.fun_decls[11].body.Structured.locals.locals[1].name = "program"
   | .translated.fun_decls[12].body.Structured.locals.locals[1].name = "program"
else error("unexpected evaluator or root-scan input") end
' target/replay/evaluator.source.llbc > target/replay/evaluator.llbc

jq -e --slurpfile source target/replay/evaluator.source.llbc '
  .translated.fun_decls[11].body.Structured.locals.locals[1].name == "program"
  and .translated.fun_decls[12].body.Structured.locals.locals[1].name == "program"
  and ((.translated.fun_decls[11].body.Structured.locals.locals[1].name = "theory"
        | .translated.fun_decls[12].body.Structured.locals.locals[1].name = "theory")
       == $source[0])
' target/replay/evaluator.llbc

jq -e --slurpfile raw target/replay/evaluator.raw.llbc '
  (.translated.options.dest_file = $raw[0].translated.options.dest_file
   | .translated.fun_decls[11].body.Structured.locals.locals[1].name = "theory"
   | .translated.fun_decls[12].body.Structured.locals.locals[1].name = "theory")
  == $raw[0]
' target/replay/evaluator.llbc

mkdir -p target/replay/Evaluator
.lake/aeneas/aeneas -backend lean -dest target/replay/Evaluator \
  -split-files -namespace ZetesisExtract -all-computable -no-progress-bar \
  -sequential -warnings-as-errors -abort-on-error target/replay/evaluator.llbc
cmp Evaluator/Types.lean target/replay/Evaluator/Types.lean
cmp Evaluator/Funs.lean target/replay/Evaluator/Funs.lean
```

Recheck the selected source hashes and both precise function/local identities in
`provenance.json` before accepting a repeated extraction. These paths are specific
to this recorded extraction. Never reuse them silently after a structural change.
Compiler-platform metadata may differ; generated-definition comparison, source
identity and explicit review remain separate checks.

Do not install generated external templates. `PureExternals`, `AtomicTypes` and
`AtomicLoad` supply the concrete, scoped models reviewed by these proofs.
Regeneration does not itself establish their correspondence to Rust or prove
whole-loop behavior. Repeat the strict build and complete authored-theorem audit
after accepting any changed extraction.
