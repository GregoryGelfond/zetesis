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
`FrozenReduct::satisfied_by` query, their semantic composition, the generated
admission step, validators and `Theory::new` wrapper, and the checked boundary
examples. The wrapper receives the explicit allocation operation described below. These use the documented
fixed-token external model; the build does not establish its correspondence with
concurrent Rust execution.

## Repeat extraction

Install Rust `nightly-2026-08-18`, including `rustc-dev` and `rust-src`. This is
an extraction toolchain, not a change to the solver's ordinary Rust pin. The
extractor selects the real private production evaluator, root scan, subset
query and countermodel-search phases directly.
It also selects the actual owner check and theory clone, and the private
admission step, four validators and `Theory::new` itself. It selects
`FrozenReduct::satisfied_by` through the existing reduct module, then
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
  --start-from zetesis_ferraris::oracle::identities \
  --start-from '{impl core::clone::Clone for zetesis_ferraris::theory::Theory}::clone' \
  --start-from zetesis_ferraris::oracle::evaluate \
  --start-from zetesis_ferraris::oracle::failed_root \
  --start-from zetesis_ferraris::oracle::find_countermodel \
  --start-from zetesis_ferraris::oracle::check_subset \
  --start-from zetesis_ferraris::oracle::select_atoms \
  --start-from zetesis_ferraris::oracle::advance_subset \
  --start-from zetesis_ferraris::reduct \
  --start-from zetesis_ferraris::theory::validate_node \
  --start-from zetesis_ferraris::theory::validate_nodes \
  --start-from zetesis_ferraris::theory::validate_root \
  --start-from zetesis_ferraris::theory::validate_roots \
  --start-from zetesis_ferraris::theory::admit \
  --start-from zetesis_ferraris::theory::_::new \
  --include zetesis_ferraris \
  --include zetesis_cpu::cancellation \
  --include zetesis_ferraris::reduct::FrozenReduct \
  --include zetesis_ferraris::reduct::_::satisfied_by \
  --include zetesis_ferraris::reduct::_::theory \
  --exclude zetesis_ferraris::oracle::check \
  --exclude zetesis_ferraris::oracle::reserve \
  --exclude zetesis_ferraris::reduct::_::new \
  --exclude zetesis_ferraris::reduct::_::freeze \
  --exclude zetesis_ferraris::reduct::_::is_satisfied_by \
  --exclude zetesis_ferraris::reduct::_::candidate \
  --exclude zetesis_ferraris::reduct::_::fmt \
  --dest-file "$package_dir/target/replay/evaluator.raw.llbc" \
  -- --manifest-path ../../crates/zetesis-ferraris/Cargo.toml --lib --locked --offline
```

Use an owned build directory and retire it after retaining source/tool hashes,
extraction and proof evidence. Source changes require a new extraction and proof
check; old hashes cannot qualify changed code.

Pinned Charon serializes its `short_names` table in an order that can differ
between runs of the same extraction. Every declaration, the ordered declaration
list, item names, files and options are unaffected, and the generated Lean is
byte-identical. Compare a repeated extraction by those sections and by the
generated files below, not by the bytes of `evaluator.source.llbc`.

## Normalize the translation input and compare

The public source LLBC changes only destination metadata. The translation input
also renames six local debug names to avoid Lean namespace collisions, removes
the unused derived `Debug` registration, omits the unused generic
`Interpretation::new` ordered export, and clears two unused `Step` method slots
in the trait and its `usize` implementation. Pinned Charon registers
`forward_overflowing` and `backward_overflowing`, but the pinned Lean `Step`
record lacks those fields. Its range iterator calls `forward_checked`.
The official `-filter-trait-methods` option alone does not resolve this mismatch.
The independently selected generic `Interpretation::new` export fails in the
pinned translator; its complete function declaration and closure bodies remain
in the input. Only its unreferenced ordered export is omitted.

The guarded selection below checks identities, the `usize` implementation,
absence of a vtable and absence of direct or indirect references in surviving
semantic declarations. Method indices remain fixed; all function declarations
and executable bodies remain intact except the stated local debug names.
Restoration must reproduce the entire parsed source and raw output. The later
Lean allocation binder is a separate, explicitly documented adaptation. This is
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
  $t.trait_impls[25] as $debug |
  $debug.def_id == 25
  and $debug.item_meta.name == [
    {"Ident":["zetesis_ferraris",0]}, {"Ident":["reduct",0]},
    {"Impl":{"Trait":25}}]
  and $debug.item_meta.attr_info.attributes == [{"Builtin":"AutomaticallyDerived"}]
  and $debug.impl_trait.id == 30
  and $t.trait_decls[30].item_meta.name == [
    {"Ident":["core",0]}, {"Ident":["fmt",0]}, {"Ident":["Debug",0]}]
  and ($debug.methods | length) == 1
  and $debug.methods[0].skip_binder.id == 200
  and $t.fun_decls[200] == null
  and $t.ordered_decls[136] == {"TraitImpl":{"NonRec":25}}
  and ([$t.ordered_decls[] | select(. == {"TraitImpl":{"NonRec":25}})] | length) == 1
  and ([[$t.type_decls, $t.fun_decls, $t.global_decls, $t.trait_decls,
          ($t.trait_impls | to_entries | map(select(.key != 25) | .value))]
        | .. | objects | select(
            .TraitImpl? == 25 or .TraitImpl?.id? == 25
            or .impl_ref?.id? == 25 or .trait_impl?.id? == 25
            or .trait_impl_id? == 25)] | length) == 0;
def step_method($slot; $name; $function):
  .translated as $t |
  $t.trait_decls[9].methods[$slot] as $decl |
  $t.trait_impls[23].methods[$slot] as $impl |
  $decl.kind == {"TraitMethod":[9,$slot]}
  and $decl.skip_binder.name == $name
  and $impl.kind == {"TraitMethod":[9,$slot]}
  and $impl.skip_binder.id == $function
  and $t.fun_decls[$function].body == "Opaque"
  and $t.fun_decls[$function].item_meta.name == [
    {"Ident":["core",0]}, {"Ident":["iter",0]}, {"Ident":["range",0]},
    {"Impl":{"Trait":23}}, {"Ident":[$name,0]}];
def step_is_unused:
  .translated as $t |
  $t.trait_decls[9].item_meta.name == [
    {"Ident":["core",0]}, {"Ident":["iter",0]}, {"Ident":["range",0]},
    {"Ident":["Step",0]}]
  and $t.trait_impls[23].impl_trait.id == 9
  and $t.trait_impls[23].impl_trait.generics.types == [{"Deduplicated":0}]
  and $t.item_names[1].value[2].Impl.Ty.params.const_generics[0].ty ==
    {"Value":[0,{"Scalar":{"Integer":{"Unsigned":"Usize"}}}]}
  and $t.trait_impls[23].vtable == null
  and step_method(2; "forward_overflowing"; 193)
  and step_method(6; "backward_overflowing"; 197)
  and ((.translated.trait_decls[9].methods[2] = null
    | .translated.trait_decls[9].methods[6] = null
    | .translated.trait_impls[23].methods[2] = null
    | .translated.trait_impls[23].methods[6] = null
    | .translated.fun_decls[193] |= del(.src)
    | .translated.fun_decls[197] |= del(.src)
    | [.translated.type_decls, .translated.fun_decls,
       .translated.global_decls, .translated.trait_decls,
       .translated.trait_impls]
    | walk(if type == "object" then del(.item_meta) else . end)
    | [.. | objects | select(
        .Fun? == 193 or .Fun? == 197
        or .TraitMethod? == [9,2] or .TraitMethod? == [9,6]
        or (.trait_ref?.id? == 9 and (.item_id? == 2 or .item_id? == 6))
        or (.impl_ref?.id? == 23 and (.item_id? == 2 or .item_id? == 6)))])
      | length) == 0;
def interpretation_export_is_unused:
  .translated as $t |
  $t.fun_decls[25] as $function |
  $function.def_id == 25
  and $function.item_meta.name == [
    {"Ident":["zetesis_ferraris",0]}, {"Ident":["theory",0]},
    {"Impl":{"Ty":{"params":{"regions":[],"types":[],"const_generics":[],
      "trait_clauses":[],"regions_outlive":[],"types_outlive":[],
      "trait_type_constraints":[]},"skip_binder":{"Deduplicated":4},
      "kind":"InherentImplBlock"}}}, {"Ident":["new",0]}]
  and $function.src == "Normal"
  and $function.item_meta.is_local == true
  and $function.item_meta.opacity == "Transparent"
  and $function.body.Structured.locals.arg_count == 2
  and $function.body.Structured.locals.locals[1].name == "theory"
  and $t.ordered_decls[156] == {"Fun":{"NonRec":25}}
  and ([$t.ordered_decls[] | select(. == {"Fun":{"NonRec":25}})] | length) == 1
  and ([[$t.type_decls,
          ($t.fun_decls | to_entries | map(select(.key != 25) | .value)),
          $t.global_decls, $t.trait_decls, $t.trait_impls]
        | walk(if type == "object" then del(.item_meta) else . end)
        | .. | objects | select(.Fun? == 25 or .Regular? == 25
            or .fun_id? == 25 or .function_id? == 25)] | length) == 0
  and ([[$t.trait_decls[], $t.trait_impls[]] | .[] | select(. != null)
        | .methods[]? | select(. != null) | select(.skip_binder.id? == 25)]
       | length) == 0;
if checked_argument(11; "identities"; 2;
     {"Value":[67,{"Ref":[{"Body":1},{"Deduplicated":3},"Shared"]}]})
   and checked_argument(13; "evaluate"; 5; {"Deduplicated":67})
   and checked_argument(14; "failed_root"; 3; {"Deduplicated":67})
   and checked_argument(15; "find_countermodel"; 6; {"Deduplicated":67})
   and checked_argument(16; "check_subset"; 5; {"Deduplicated":67})
   and checked_argument(17; "select_atoms"; 4; {"Deduplicated":67})
   and debug_is_unused and step_is_unused and interpretation_export_is_unused
then .translated.fun_decls[11].body.Structured.locals.locals[1].name = "program"
   | .translated.fun_decls[13].body.Structured.locals.locals[1].name = "program"
   | .translated.fun_decls[14].body.Structured.locals.locals[1].name = "program"
   | .translated.fun_decls[15].body.Structured.locals.locals[1].name = "program"
   | .translated.fun_decls[16].body.Structured.locals.locals[1].name = "program"
   | .translated.fun_decls[17].body.Structured.locals.locals[1].name = "program"
   | .translated.trait_impls[25] = null
   | del(.translated.ordered_decls[156])
   | del(.translated.ordered_decls[136])
   | .translated.trait_decls[9].methods[2] = null
   | .translated.trait_decls[9].methods[6] = null
   | .translated.trait_impls[23].methods[2] = null
   | .translated.trait_impls[23].methods[6] = null
else error("unexpected selected function, unused Debug/Interpretation export or Step method") end
' target/replay/evaluator.source.llbc > target/replay/evaluator.llbc

jq -e --slurpfile source target/replay/evaluator.source.llbc '
(.translated.fun_decls[11].body.Structured.locals.locals[1].name = "theory"
 | .translated.fun_decls[13].body.Structured.locals.locals[1].name = "theory"
 | .translated.fun_decls[14].body.Structured.locals.locals[1].name = "theory"
 | .translated.fun_decls[15].body.Structured.locals.locals[1].name = "theory"
 | .translated.fun_decls[16].body.Structured.locals.locals[1].name = "theory"
 | .translated.fun_decls[17].body.Structured.locals.locals[1].name = "theory"
 | .translated.trait_impls[25] = $source[0].translated.trait_impls[25]
 | .translated.ordered_decls = (.translated.ordered_decls[:136]
     + [$source[0].translated.ordered_decls[136]] + .translated.ordered_decls[136:])
 | .translated.ordered_decls = (.translated.ordered_decls[:156]
     + [$source[0].translated.ordered_decls[156]] + .translated.ordered_decls[156:])
 | .translated.trait_decls[9].methods[2] = $source[0].translated.trait_decls[9].methods[2]
 | .translated.trait_decls[9].methods[6] = $source[0].translated.trait_decls[9].methods[6]
 | .translated.trait_impls[23].methods[2] = $source[0].translated.trait_impls[23].methods[2]
 | .translated.trait_impls[23].methods[6] = $source[0].translated.trait_impls[23].methods[6]) == $source[0]
' target/replay/evaluator.llbc

jq -e --slurpfile source target/replay/evaluator.source.llbc \
  --slurpfile raw target/replay/evaluator.raw.llbc '
(.translated.fun_decls[11].body.Structured.locals.locals[1].name = "theory"
 | .translated.fun_decls[13].body.Structured.locals.locals[1].name = "theory"
 | .translated.fun_decls[14].body.Structured.locals.locals[1].name = "theory"
 | .translated.fun_decls[15].body.Structured.locals.locals[1].name = "theory"
 | .translated.fun_decls[16].body.Structured.locals.locals[1].name = "theory"
 | .translated.fun_decls[17].body.Structured.locals.locals[1].name = "theory"
 | .translated.trait_impls[25] = $source[0].translated.trait_impls[25]
 | .translated.ordered_decls = (.translated.ordered_decls[:136]
     + [$source[0].translated.ordered_decls[136]] + .translated.ordered_decls[136:])
 | .translated.ordered_decls = (.translated.ordered_decls[:156]
     + [$source[0].translated.ordered_decls[156]] + .translated.ordered_decls[156:])
 | .translated.trait_decls[9].methods[2] = $source[0].translated.trait_decls[9].methods[2]
 | .translated.trait_decls[9].methods[6] = $source[0].translated.trait_decls[9].methods[6]
 | .translated.trait_impls[23].methods[2] = $source[0].translated.trait_impls[23].methods[2]
 | .translated.trait_impls[23].methods[6] = $source[0].translated.trait_impls[23].methods[6]) | (.translated.options.dest_file = $raw[0].translated.options.dest_file) == $raw[0]
' target/replay/evaluator.llbc

mkdir -p target/replay/Evaluator
.lake/aeneas/aeneas -backend lean -dest target/replay/Evaluator \
  -split-files -namespace ZetesisExtract -all-computable -no-progress-bar \
  -sequential -warnings-as-errors -abort-on-error target/replay/evaluator.llbc
cmp Evaluator/Types.lean target/replay/Evaluator/Types.lean
# Compare Funs.lean after the explicit allocation binding below.
```

Recheck the selected source hashes, all six precise function/local identities and
the unused Debug, Interpretation export and Step declarations in `provenance.json` before accepting a
repeated extraction. These paths are specific
to this recorded extraction. Never reuse them silently after a structural change.
Compiler-platform metadata may differ; generated-definition comparison, source
identity and explicit review remain separate checks.

## Bind the allocation operation

Stock Aeneas emits `Theory::new` with a call to `alloc.sync.Arc.new` and no
allocation parameter. This package adds a section binder only around that
constructor. It changes the Lean signature: the constructor now takes an
implicit `ArcAllocation` provider. The executable body is unchanged, but this
parameterization is an authored binding adaptation, not stock translator output
or merely a debug-name change.

`ArcAllocation` supplies one external operation; it has no global instance and
assumes neither success, value preservation nor freshness. Theorems state any
needed contract for a returned invocation explicitly. A pure provider reused on
equal inputs does not model independent fresh allocations. Heap freshness and
composition across invocations require their own runtime account. Rust's
`Arc::new` does not report a typed admission error on allocation failure.

Keep the raw generated file, check its recorded identity, insert the binder and
check that removing it recovers the exact raw bytes:

```sh
printf '%s\n' '3955f00e2fa32fe39001937ef00c1dbc6ab8a493d53f273988d5ea0d3c9624bf  target/replay/Evaluator/Funs.lean' | shasum -a 256 -c -
awk '
/^\/-- \[zetesis_ferraris::theory::\{zetesis_ferraris::theory::Theory\}::new\]:$/ {
  if (opened != 0 || closed != 0) exit 1
  print "section TheoryAllocation"
  print "variable [ArcAllocation]"
  print ""
  opened++
}
/^\/-- \[zetesis_ferraris::theory::\{zetesis_ferraris::theory::Interpretation\}::new::\{impl / {
  if (opened != 1 || closed != 0) exit 1
  print "end TheoryAllocation"
  print ""
  closed++
}
{ print }
END { if (opened != 1 || closed != 1) exit 1 }
' target/replay/Evaluator/Funs.lean > target/replay/Funs.bound.lean
printf '%s\n' '6dce2f17708d9d6463d94fe52703b38a9c607d621e9bed2507e1263f91edd4a5  target/replay/Funs.bound.lean' | shasum -a 256 -c -
awk '
$0 == "section TheoryAllocation" {
  if (opened != 0 || closed != 0) exit 1
  if (getline <= 0 || $0 != "variable [ArcAllocation]") exit 1
  if (getline <= 0 || $0 != "") exit 1
  opened++
  next
}
$0 == "end TheoryAllocation" {
  if (opened != 1 || closed != 0) exit 1
  if (getline <= 0 || $0 != "") exit 1
  closed++
  next
}
{ print }
END { if (opened != 1 || closed != 1) exit 1 }
' target/replay/Funs.bound.lean > target/replay/Funs.restored.lean
cmp target/replay/Evaluator/Funs.lean target/replay/Funs.restored.lean
cmp Evaluator/Funs.lean target/replay/Funs.bound.lean
```

Do not install generated external templates. `PureExternals`, `OwnerExternals`, `AtomicTypes`,
`AtomicLoad` and `ArcAllocation` supply the concrete, scoped models reviewed by these proofs.
Regeneration does not establish external-model correspondence to Rust.
The retained unused interpretation closure mentions `TryReserveError`; its
abstract token in `PureExternals` supplies no reservation operation or model. The
private query does not cover frozen-mask construction, fallible reservation,
composition of owner checks with the public wrapper, or public query wrappers. Repeat the strict build and complete authored-theorem audit
after accepting any changed extraction.

## Current subset-search extraction limit

The allocation-free `find_countermodel` phase and its subset-query and carry
helpers now translate directly. Their exported bodies are the production
operations, with no generated-code rewriting or success assumptions for missing
externals. Translation alone does not prove the search invariant, termination,
proper-subset coverage or final membership verdict.

The public `oracle::check` wrapper remains outside this package's selected
roots. A separate translation check confirmed that it and `FrozenReduct`
construction translate without Rust changes. Their dependency closure requires
`Vec.try_reserve_exact` and `TryReserveError`; the active sequence model does not
represent returning reservation failures or capacity. Do not install generated
axiom templates or replace reservation with an always-success function.

The selected owner check and clone now use explicit owner tokens. Immutable
heap consistency connects a successful owner comparison to equal stored theory
data. This does not prove allocation, reference counting or the public wrapper's
composition of these operations.

`RuntimeEffects` specifies returning read and reservation events separately.
The current extracted `Result`, callback traits and loop interfaces still use
the fixed-observation model. Connecting the richer effect to generated calls,
then rechecking that dependency closure, remains necessary.

`Theory::new` runs `admit`, then returns its refusal unchanged or passes its
admitted data to `Arc::new`. The constructor, admission step and validators are
now selected directly. `AdmissionValidation` and `AdmittedData` prove the
validators and admission step; constructor proofs use the explicit allocation
binding above. Any stored-value or owner conclusion depends on the stated
contract for that invocation's returned allocation. The generic
`Interpretation::new` export remains unsupported and omitted, although its full
LLBC declaration and generated closure bodies are retained. No reservation,
allocator, reference-count or constructor runtime correspondence follows from
this extraction or binding.
