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
The default build includes the generated evaluator, root scan and private
`FrozenReduct::satisfied_by` query, their semantic composition, and the checked
boundary examples. These use the documented
fixed-token external model; the build does not establish its correspondence with
concurrent Rust execution.

## Repeat extraction

Install Rust `nightly-2026-08-18`, including `rustc-dev` and `rust-src`. This is
an extraction toolchain, not a change to the solver's ordinary Rust pin. The
extractor selects the real private production evaluator, root scan and subset
phase helpers directly.
It selects `FrozenReduct::satisfied_by` through the existing reduct module, then
excludes the other methods. No new Rust wrapper is needed. The extraction runs offline; if dependencies are
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
  --start-from zetesis_ferraris::oracle::select_atoms \
  --start-from zetesis_ferraris::oracle::advance_subset \
  --start-from zetesis_ferraris::reduct \
  --include zetesis_ferraris --include zetesis_cpu::cancellation \
  --include zetesis_ferraris::reduct::FrozenReduct \
  --include 'zetesis_ferraris::reduct::_::satisfied_by' \
  --include 'zetesis_ferraris::reduct::_::theory' \
  --exclude 'zetesis_ferraris::reduct::_::new' \
  --exclude 'zetesis_ferraris::reduct::_::freeze' \
  --exclude 'zetesis_ferraris::reduct::_::is_satisfied_by' \
  --exclude 'zetesis_ferraris::reduct::_::candidate' \
  --exclude 'zetesis_ferraris::reduct::_::fmt' \
  --dest-file target/replay/evaluator.raw.llbc \
  -- --manifest-path ../../crates/zetesis-ferraris/Cargo.toml --lib --locked --offline
```

Use an owned build directory and retire it after retaining source/tool hashes,
extraction and proof evidence. Source changes require a new extraction and proof
check; old hashes cannot qualify changed code.

## Normalize the translation input and compare

The public source LLBC changes only destination metadata. The translation input
also renames three local debug names to avoid Lean namespace collisions, removes
the unused derived `Debug` registration, and clears two unused `Step` method
slots in the trait and its `usize` implementation. Pinned Charon registers
`forward_overflowing` and `backward_overflowing`, but the pinned Lean `Step`
record lacks those fields. Its range iterator calls `forward_checked`.
The official `-filter-trait-methods` option alone does not resolve this mismatch.

The guarded selection below checks identities, the `usize` implementation,
absence of a vtable and absence of direct or indirect references in surviving
semantic declarations. Method indices remain fixed; all function declarations
and executable bodies remain intact except the stated local debug names.
Restoration must reproduce the entire parsed source and raw output. This is
explicit tool-model compatibility selection, not verification of Rust's standard
library. These commands require `jq`:

```sh
jq -cae '.translated.options.dest_file = "evaluator.llbc"' \
  target/replay/evaluator.raw.llbc > target/replay/evaluator.source.llbc
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
def debug_is_unused:
  .translated as $t |
  $t.trait_impls[20] as $debug |
  $debug.def_id == 20
  and $debug.item_meta.name == [
    {"Ident":["zetesis_ferraris",0]}, {"Ident":["reduct",0]},
    {"Impl":{"Trait":20}}]
  and $debug.item_meta.attr_info.attributes == [{"Builtin":"AutomaticallyDerived"}]
  and $debug.impl_trait.id == 27
  and $t.trait_decls[27].item_meta.name == [
    {"Ident":["core",0]}, {"Ident":["fmt",0]}, {"Ident":["Debug",0]}]
  and ($debug.methods | length) == 1
  and $debug.methods[0].skip_binder.id == 171
  and $t.fun_decls[171] == null
  and $t.ordered_decls[108] == {"TraitImpl":{"NonRec":20}}
  and ([$t.ordered_decls[] | select(. == {"TraitImpl":{"NonRec":20}})] | length) == 1
  and ([[$t.type_decls, $t.fun_decls, $t.global_decls, $t.trait_decls,
          ($t.trait_impls | to_entries | map(select(.key != 20) | .value))]
        | .. | objects | select(
            .TraitImpl? == 20 or .TraitImpl?.id? == 20
            or .impl_ref?.id? == 20 or .trait_impl?.id? == 20
            or .trait_impl_id? == 20)] | length) == 0;
def step_method($slot; $name; $function):
  .translated as $t |
  $t.trait_decls[8].methods[$slot] as $decl |
  $t.trait_impls[18].methods[$slot] as $impl |
  $decl.kind == {"TraitMethod":[8,$slot]}
  and $decl.skip_binder.name == $name
  and $impl.kind == {"TraitMethod":[8,$slot]}
  and $impl.skip_binder.id == $function
  and $t.fun_decls[$function].body == "Opaque"
  and $t.fun_decls[$function].item_meta.name == [
    {"Ident":["core",0]}, {"Ident":["iter",0]}, {"Ident":["range",0]},
    {"Impl":{"Trait":18}}, {"Ident":[$name,0]}];
def step_is_unused:
  .translated as $t |
  $t.trait_decls[8].item_meta.name == [
    {"Ident":["core",0]}, {"Ident":["iter",0]}, {"Ident":["range",0]},
    {"Ident":["Step",0]}]
  and $t.trait_impls[18].impl_trait.id == 8
  and $t.trait_impls[18].impl_trait.generics.types == [{"Deduplicated":0}]
  and $t.item_names[1].value[2].Impl.Ty.params.const_generics[0].ty ==
    {"Value":[0,{"Scalar":{"Integer":{"Unsigned":"Usize"}}}]}
  and $t.trait_impls[18].vtable == null
  and step_method(2; "forward_overflowing"; 164)
  and step_method(6; "backward_overflowing"; 168)
  and ((.translated.trait_decls[8].methods[2] = null
    | .translated.trait_decls[8].methods[6] = null
    | .translated.trait_impls[18].methods[2] = null
    | .translated.trait_impls[18].methods[6] = null
    | .translated.fun_decls[164] |= del(.src)
    | .translated.fun_decls[168] |= del(.src)
    | [.translated.type_decls, .translated.fun_decls,
       .translated.global_decls, .translated.trait_decls,
       .translated.trait_impls]
    | walk(if type == "object" then del(.item_meta) else . end)
    | [.. | objects | select(
        .Fun? == 164 or .Fun? == 168
        or .TraitMethod? == [8,2] or .TraitMethod? == [8,6]
        or (.trait_ref?.id? == 8 and (.item_id? == 2 or .item_id? == 6))
        or (.impl_ref?.id? == 18 and (.item_id? == 2 or .item_id? == 6)))])
      | length) == 0;
if checked_argument(11; "evaluate"; 5;
     {"Value":[66,{"Ref":[{"Body":1},{"Deduplicated":4},"Shared"]}]})
   and checked_argument(12; "failed_root"; 3; {"Deduplicated":66})
   and checked_argument(13; "select_atoms"; 4; {"Deduplicated":66})
   and debug_is_unused and step_is_unused
then .translated.fun_decls[11].body.Structured.locals.locals[1].name = "program"
   | .translated.fun_decls[12].body.Structured.locals.locals[1].name = "program"
   | .translated.fun_decls[13].body.Structured.locals.locals[1].name = "program"
   | .translated.trait_impls[20] = null
   | del(.translated.ordered_decls[108])
   | .translated.trait_decls[8].methods[2] = null
   | .translated.trait_decls[8].methods[6] = null
   | .translated.trait_impls[18].methods[2] = null
   | .translated.trait_impls[18].methods[6] = null
else error("unexpected selected function, unused Debug declaration or Step method") end

' target/replay/evaluator.source.llbc > target/replay/evaluator.llbc

jq -e --slurpfile source target/replay/evaluator.source.llbc '
(.translated.fun_decls[11].body.Structured.locals.locals[1].name = "theory"
 | .translated.fun_decls[12].body.Structured.locals.locals[1].name = "theory"
 | .translated.fun_decls[13].body.Structured.locals.locals[1].name = "theory"
 | .translated.trait_impls[20] = $source[0].translated.trait_impls[20]
 | .translated.ordered_decls = (.translated.ordered_decls[:108]
     + [$source[0].translated.ordered_decls[108]] + .translated.ordered_decls[108:])
 | .translated.trait_decls[8].methods[2] = $source[0].translated.trait_decls[8].methods[2]
 | .translated.trait_decls[8].methods[6] = $source[0].translated.trait_decls[8].methods[6]
 | .translated.trait_impls[18].methods[2] = $source[0].translated.trait_impls[18].methods[2]
 | .translated.trait_impls[18].methods[6] = $source[0].translated.trait_impls[18].methods[6]) == $source[0]
' target/replay/evaluator.llbc

jq -e --slurpfile source target/replay/evaluator.source.llbc \
  --slurpfile raw target/replay/evaluator.raw.llbc '
(.translated.fun_decls[11].body.Structured.locals.locals[1].name = "theory"
 | .translated.fun_decls[12].body.Structured.locals.locals[1].name = "theory"
 | .translated.fun_decls[13].body.Structured.locals.locals[1].name = "theory"
 | .translated.trait_impls[20] = $source[0].translated.trait_impls[20]
 | .translated.ordered_decls = (.translated.ordered_decls[:108]
     + [$source[0].translated.ordered_decls[108]] + .translated.ordered_decls[108:])
 | .translated.trait_decls[8].methods[2] = $source[0].translated.trait_decls[8].methods[2]
 | .translated.trait_decls[8].methods[6] = $source[0].translated.trait_decls[8].methods[6]
 | .translated.trait_impls[18].methods[2] = $source[0].translated.trait_impls[18].methods[2]
 | .translated.trait_impls[18].methods[6] = $source[0].translated.trait_impls[18].methods[6]) | (.translated.options.dest_file = $raw[0].translated.options.dest_file) == $raw[0]
' target/replay/evaluator.llbc

mkdir -p target/replay/Evaluator
.lake/aeneas/aeneas -backend lean -dest target/replay/Evaluator \
  -split-files -namespace ZetesisExtract -all-computable -no-progress-bar \
  -sequential -warnings-as-errors -abort-on-error target/replay/evaluator.llbc
cmp Evaluator/Types.lean target/replay/Evaluator/Types.lean
cmp Evaluator/Funs.lean target/replay/Evaluator/Funs.lean
```

Recheck the selected source hashes, all three precise function/local identities and
the unused Debug and Step declarations in `provenance.json` before accepting a
repeated extraction. These paths are specific
to this recorded extraction. Never reuse them silently after a structural change.
Compiler-platform metadata may differ; generated-definition comparison, source
identity and explicit review remain separate checks.

Do not install generated external templates. `PureExternals`, `AtomicTypes` and
`AtomicLoad` supply the concrete, scoped models reviewed by these proofs.
Regeneration does not establish external-model correspondence to Rust. The
private query does not cover frozen-mask construction, fallible reservation,
public owner checks or public query wrappers. Repeat the strict build and complete authored-theorem audit
after accepting any changed extraction.

## Current subset-search extraction limit

The selected-atom and packed-carry helpers now translate. The same pinned tools
extract the complete `oracle::check` to LLBC, but Aeneas still rejects its outer
control flow with `Early returns inside of loops are not supported yet`
(`PrePasses.ml:576`). This refusal occurs before Lean output. It is a translation
limitation, not a proved defect in the Rust algorithm.

To reproduce without replacing the successful package inputs, repeat the
extraction command above with the additional option
`--start-from zetesis_ferraris::oracle::check` and change its destination to
`target/subset-probe/evaluator.llbc` (create that directory first). Then run:

```sh
mkdir -p target/subset-probe/Evaluator
.lake/aeneas/aeneas -backend lean -dest target/subset-probe/Evaluator \
  -split-files -namespace ZetesisExtract -all-computable -no-progress-bar \
  -sequential -warnings-as-errors -abort-on-error target/subset-probe/evaluator.llbc
```

This command is expected to fail before Lean output and needs no LLBC
normalization. The helper extraction and its boundary laws do not establish the
complete checker's subset coverage or membership verdict. Allocation, owner
identity and changing concurrent observations remain separate boundaries.
