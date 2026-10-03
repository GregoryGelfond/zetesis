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
admission step, validators, `Theory::new`, and the frozen-reduct producer and
public query, the generic interpretation constructor and insertion phase, and
the public `oracle::check` wrapper and owned `check_interpretation` decision API.
Allocation and reservation use the explicit supplied operations
described below. These use the documented fixed-token external model; the build does not establish its correspondence with
concurrent Rust execution.

## Repeat extraction

Install Rust `nightly-2026-08-18`, including `rustc-dev` and `rust-src`. This is
an extraction toolchain, not a change to the solver's ordinary Rust pin. The
extractor selects the real private production evaluator, root scan, subset
query and countermodel-search phases directly.
It also selects the actual owner check and theory clone, and the private
admission step, four validators and `Theory::new` itself. It selects
`FrozenReduct::{freeze,new,is_satisfied_by,satisfied_by}` through the reduct
module and retains the actual `oracle::reserve` helper. The generic
`Interpretation::new` delegates its checked packed writes to `insert_atoms`,
which takes the atom bound, an iterator and a fixed-length mutable word slice. Both are selected
directly. The public `oracle::check` wrapper is selected with the same operations
and library bindings. The owned `checked::check_interpretation` wrapper and its
candidate, verdict, acceptance, statistics and move/accessor methods are selected
individually; derived `Clone` and `Debug` are outside this selected API. Extraction runs offline; if dependencies are not cached, first run `cargo fetch --locked --manifest-path ../../Cargo.toml`.

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
  --start-from zetesis_ferraris::oracle::reserve \
  --start-from zetesis_ferraris::reduct::_::freeze \
  --start-from zetesis_ferraris::reduct::_::new \
  --start-from zetesis_ferraris::reduct::_::is_satisfied_by \
  --start-from zetesis_ferraris::theory::insert_atoms \
  --start-from zetesis_ferraris::oracle::check \
  --start-from zetesis_ferraris::checked::check_interpretation \
  --start-from zetesis_ferraris::checked::_::candidate \
  --start-from zetesis_ferraris::checked::_::verdict \
  --start-from zetesis_ferraris::checked::_::accepted \
  --start-from zetesis_ferraris::checked::_::statistics \
  --start-from zetesis_ferraris::checked::_::into_stable_interpretation \
  --start-from zetesis_ferraris::checked::_::theory \
  --start-from zetesis_ferraris::checked::_::interpretation \
  --start-from zetesis_ferraris::checked::_::into_interpretation \
  --include zetesis_ferraris \
  --include zetesis_cpu::cancellation \
  --include zetesis_ferraris::reduct::FrozenReduct \
  --include zetesis_ferraris::reduct::_::satisfied_by \
  --include zetesis_ferraris::reduct::_::theory \
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
also renames eight local debug names to avoid Lean namespace collisions, removes
the unused derived `Debug` registration, and clears two unused `Step` method slots
in the trait and its `usize` implementation. Pinned Charon registers
`forward_overflowing` and `backward_overflowing`, but the pinned Lean `Step`
record lacks those fields. Its range iterator calls `forward_checked`.
The official `-filter-trait-methods` option alone does not resolve this mismatch.
Both `Theory::new` and generic `Interpretation::new` remain exported, together
with the fixed-length `insert_atoms` phase. No constructor export is omitted.

The guarded selection below checks identities, the `usize` implementation,
absence of a vtable and absence of direct or indirect references in surviving
semantic declarations. Method indices remain fixed; all function declarations
and executable bodies remain intact except the stated local debug names.
Restoration must reproduce the entire parsed source and raw output. The later
Lean allocation and reservation binders are separate, explicitly documented
adaptations. This is
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
  and $debug.methods[0].skip_binder.id == 222
  and $t.fun_decls[222] == null
  and $t.ordered_decls[169] == {"TraitImpl":{"NonRec":25}}
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
  and step_method(2; "forward_overflowing"; 215)
  and step_method(6; "backward_overflowing"; 219)
  and ((.translated.trait_decls[9].methods[2] = null
    | .translated.trait_decls[9].methods[6] = null
    | .translated.trait_impls[23].methods[2] = null
    | .translated.trait_impls[23].methods[6] = null
    | .translated.fun_decls[215] |= del(.src)
    | .translated.fun_decls[219] |= del(.src)
    | [.translated.type_decls, .translated.fun_decls,
       .translated.global_decls, .translated.trait_decls,
       .translated.trait_impls]
    | walk(if type == "object" then del(.item_meta) else . end)
    | [.. | objects | select(
        .Fun? == 215 or .Fun? == 219
        or .Regular? == 215 or .Regular? == 219
        or .fun_id? == 215 or .fun_id? == 219
        or .function_id? == 215 or .function_id? == 219
        or .TraitMethod? == [9,2] or .TraitMethod? == [9,6]
        or (.trait_ref?.id? == 9 and (.item_id? == 2 or .item_id? == 6))
        or (.impl_ref?.id? == 23 and (.item_id? == 2 or .item_id? == 6)))])
      | length) == 0;
def checked_interpretation_constructor:
  .translated.fun_decls[25] as $function |
  $function.def_id == 25
  and $function.item_meta.name == [{"Ident":["zetesis_ferraris",0]},{"Ident":["theory",0]},{"Impl":{"Ty":{"params":{"regions":[],"types":[],"const_generics":[],"trait_clauses":[],"regions_outlive":[],"types_outlive":[],"trait_type_constraints":[]},"skip_binder":{"Deduplicated":4},"kind":"InherentImplBlock"}}},{"Ident":["new",0]}]
  and $function.src == "Normal"
  and $function.item_meta.is_local == true
  and $function.item_meta.opacity == "Transparent"
  and $function.body.Structured.locals.arg_count == 2
  and $function.body.Structured.locals.locals[1].index == 1
  and $function.body.Structured.locals.locals[1].name == "theory"
  and $function.body.Structured.locals.locals[1].ty == {"Deduplicated":71}
  and ([.translated.ordered_decls[] | select(. == {"Fun":{"NonRec":25}})] | length) == 1;
def checked_insertion:
  .translated.fun_decls[30] as $function |
  $function.def_id == 30
  and $function.item_meta.name == [{"Ident":["zetesis_ferraris",0]},{"Ident":["theory",0]},{"Ident":["insert_atoms",0]}]
  and $function.src == "Normal"
  and $function.item_meta.is_local == true
  and $function.item_meta.opacity == "Transparent"
  and $function.body.Structured.locals.arg_count == 3
  and $function.body.Structured.locals.locals[1].index == 1
  and $function.body.Structured.locals.locals[1].name == "atom_count"
  and $function.body.Structured.locals.locals[1].ty == {"Deduplicated":0}
  and ([.translated.ordered_decls[] | select(. == {"Fun":{"NonRec":30}})] | length) == 1;
def checked_owned_exports:
  .translated as $t |
  all([
  {
    "id": 32,
    "name": [
      {
        "Ident": [
          "zetesis_ferraris",
          0
        ]
      },
      {
        "Ident": [
          "checked",
          0
        ]
      },
      {
        "Ident": [
          "check_interpretation",
          0
        ]
      }
    ],
    "arguments": 3
  },
  {
    "id": 33,
    "name": [
      {
        "Ident": [
          "zetesis_ferraris",
          0
        ]
      },
      {
        "Ident": [
          "checked",
          0
        ]
      },
      {
        "Impl": {
          "Ty": {
            "params": {
              "regions": [],
              "types": [],
              "const_generics": [],
              "trait_clauses": [],
              "regions_outlive": [],
              "types_outlive": [],
              "trait_type_constraints": []
            },
            "skip_binder": {
              "Deduplicated": 6
            },
            "kind": "InherentImplBlock"
          }
        }
      },
      {
        "Ident": [
          "candidate",
          0
        ]
      }
    ],
    "arguments": 1
  },
  {
    "id": 34,
    "name": [
      {
        "Ident": [
          "zetesis_ferraris",
          0
        ]
      },
      {
        "Ident": [
          "checked",
          0
        ]
      },
      {
        "Impl": {
          "Ty": {
            "params": {
              "regions": [],
              "types": [],
              "const_generics": [],
              "trait_clauses": [],
              "regions_outlive": [],
              "types_outlive": [],
              "trait_type_constraints": []
            },
            "skip_binder": {
              "Deduplicated": 6
            },
            "kind": "InherentImplBlock"
          }
        }
      },
      {
        "Ident": [
          "verdict",
          0
        ]
      }
    ],
    "arguments": 1
  },
  {
    "id": 35,
    "name": [
      {
        "Ident": [
          "zetesis_ferraris",
          0
        ]
      },
      {
        "Ident": [
          "checked",
          0
        ]
      },
      {
        "Impl": {
          "Ty": {
            "params": {
              "regions": [],
              "types": [],
              "const_generics": [],
              "trait_clauses": [],
              "regions_outlive": [],
              "types_outlive": [],
              "trait_type_constraints": []
            },
            "skip_binder": {
              "Deduplicated": 6
            },
            "kind": "InherentImplBlock"
          }
        }
      },
      {
        "Ident": [
          "accepted",
          0
        ]
      }
    ],
    "arguments": 1
  },
  {
    "id": 36,
    "name": [
      {
        "Ident": [
          "zetesis_ferraris",
          0
        ]
      },
      {
        "Ident": [
          "checked",
          0
        ]
      },
      {
        "Impl": {
          "Ty": {
            "params": {
              "regions": [],
              "types": [],
              "const_generics": [],
              "trait_clauses": [],
              "regions_outlive": [],
              "types_outlive": [],
              "trait_type_constraints": []
            },
            "skip_binder": {
              "Deduplicated": 6
            },
            "kind": "InherentImplBlock"
          }
        }
      },
      {
        "Ident": [
          "statistics",
          0
        ]
      }
    ],
    "arguments": 1
  },
  {
    "id": 37,
    "name": [
      {
        "Ident": [
          "zetesis_ferraris",
          0
        ]
      },
      {
        "Ident": [
          "checked",
          0
        ]
      },
      {
        "Impl": {
          "Ty": {
            "params": {
              "regions": [],
              "types": [],
              "const_generics": [],
              "trait_clauses": [],
              "regions_outlive": [],
              "types_outlive": [],
              "trait_type_constraints": []
            },
            "skip_binder": {
              "Deduplicated": 6
            },
            "kind": "InherentImplBlock"
          }
        }
      },
      {
        "Ident": [
          "into_stable_interpretation",
          0
        ]
      }
    ],
    "arguments": 1
  },
  {
    "id": 38,
    "name": [
      {
        "Ident": [
          "zetesis_ferraris",
          0
        ]
      },
      {
        "Ident": [
          "checked",
          0
        ]
      },
      {
        "Impl": {
          "Ty": {
            "params": {
              "regions": [],
              "types": [],
              "const_generics": [],
              "trait_clauses": [],
              "regions_outlive": [],
              "types_outlive": [],
              "trait_type_constraints": []
            },
            "skip_binder": {
              "Deduplicated": 7
            },
            "kind": "InherentImplBlock"
          }
        }
      },
      {
        "Ident": [
          "theory",
          0
        ]
      }
    ],
    "arguments": 1
  },
  {
    "id": 39,
    "name": [
      {
        "Ident": [
          "zetesis_ferraris",
          0
        ]
      },
      {
        "Ident": [
          "checked",
          0
        ]
      },
      {
        "Impl": {
          "Ty": {
            "params": {
              "regions": [],
              "types": [],
              "const_generics": [],
              "trait_clauses": [],
              "regions_outlive": [],
              "types_outlive": [],
              "trait_type_constraints": []
            },
            "skip_binder": {
              "Deduplicated": 7
            },
            "kind": "InherentImplBlock"
          }
        }
      },
      {
        "Ident": [
          "interpretation",
          0
        ]
      }
    ],
    "arguments": 1
  },
  {
    "id": 40,
    "name": [
      {
        "Ident": [
          "zetesis_ferraris",
          0
        ]
      },
      {
        "Ident": [
          "checked",
          0
        ]
      },
      {
        "Impl": {
          "Ty": {
            "params": {
              "regions": [],
              "types": [],
              "const_generics": [],
              "trait_clauses": [],
              "regions_outlive": [],
              "types_outlive": [],
              "trait_type_constraints": []
            },
            "skip_binder": {
              "Deduplicated": 7
            },
            "kind": "InherentImplBlock"
          }
        }
      },
      {
        "Ident": [
          "into_interpretation",
          0
        ]
      }
    ],
    "arguments": 1
  }
][]; . as $expected |
    $t.fun_decls[$expected.id] as $function |
    $function.def_id == $expected.id
    and $function.item_meta.name == $expected.name
    and $function.item_meta.is_local == true
    and $function.item_meta.opacity == "Transparent"
    and $function.body.Structured.locals.arg_count == $expected.arguments
    and ([$t.ordered_decls[] | select(. == {"Fun":{"NonRec":$expected.id}})] | length) == 1);
if checked_argument(11; "identities"; 2;
     {"Value":[71,{"Ref":[{"Body":1},{"Deduplicated":3},"Shared"]}]})
   and checked_argument(13; "evaluate"; 5; {"Deduplicated":71})
   and checked_argument(14; "failed_root"; 3; {"Deduplicated":71})
   and checked_argument(15; "find_countermodel"; 6; {"Deduplicated":71})
   and checked_argument(16; "check_subset"; 5; {"Deduplicated":71})
   and checked_argument(17; "select_atoms"; 4; {"Deduplicated":71})
   and checked_argument(31; "check"; 4; {"Deduplicated":71})
   and checked_interpretation_constructor and checked_insertion and checked_owned_exports
   and debug_is_unused and step_is_unused
then .translated.fun_decls[11].body.Structured.locals.locals[1].name = "program"
   | .translated.fun_decls[13].body.Structured.locals.locals[1].name = "program"
   | .translated.fun_decls[14].body.Structured.locals.locals[1].name = "program"
   | .translated.fun_decls[15].body.Structured.locals.locals[1].name = "program"
   | .translated.fun_decls[16].body.Structured.locals.locals[1].name = "program"
   | .translated.fun_decls[17].body.Structured.locals.locals[1].name = "program"
   | .translated.fun_decls[25].body.Structured.locals.locals[1].name = "program"
   | .translated.fun_decls[31].body.Structured.locals.locals[1].name = "program"
   | .translated.trait_impls[25] = null
   | del(.translated.ordered_decls[169])
   | .translated.trait_decls[9].methods[2] = null
   | .translated.trait_decls[9].methods[6] = null
   | .translated.trait_impls[23].methods[2] = null
   | .translated.trait_impls[23].methods[6] = null
else error("unexpected selected function, unused Debug registration or Step method") end
' target/replay/evaluator.source.llbc > target/replay/evaluator.llbc

jq -e --slurpfile source target/replay/evaluator.source.llbc '
(.translated.fun_decls[11].body.Structured.locals.locals[1].name = "theory"
 | .translated.fun_decls[13].body.Structured.locals.locals[1].name = "theory"
 | .translated.fun_decls[14].body.Structured.locals.locals[1].name = "theory"
 | .translated.fun_decls[15].body.Structured.locals.locals[1].name = "theory"
 | .translated.fun_decls[16].body.Structured.locals.locals[1].name = "theory"
 | .translated.fun_decls[17].body.Structured.locals.locals[1].name = "theory"
 | .translated.fun_decls[25].body.Structured.locals.locals[1].name = "theory"
 | .translated.fun_decls[31].body.Structured.locals.locals[1].name = "theory"
 | .translated.trait_impls[25] = $source[0].translated.trait_impls[25]
 | .translated.ordered_decls = (.translated.ordered_decls[:169]
     + [$source[0].translated.ordered_decls[169]] + .translated.ordered_decls[169:])
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
 | .translated.fun_decls[25].body.Structured.locals.locals[1].name = "theory"
 | .translated.fun_decls[31].body.Structured.locals.locals[1].name = "theory"
 | .translated.trait_impls[25] = $source[0].translated.trait_impls[25]
 | .translated.ordered_decls = (.translated.ordered_decls[:169]
     + [$source[0].translated.ordered_decls[169]] + .translated.ordered_decls[169:])
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
# Compare Funs.lean after the explicit external bindings below.
```

Recheck the selected source hashes, all eight precise function/local identities and
the unused Debug and Step declarations in `provenance.json` before accepting a
repeated extraction. These paths are specific
to this recorded extraction. Never reuse them silently after a structural change.
Compiler-platform metadata may differ; generated-definition comparison, source
identity and explicit review remain separate checks.

## Bind allocation and reservation operations

Stock Aeneas emits calls to `alloc.sync.Arc.new` and
`alloc.vec.Vec.try_reserve_exact` without provider parameters. This package adds
eight scoped section binders: `Theory::new` takes `ArcAllocation`, and
`oracle::{reserve,check}`, `checked::check_interpretation`,
`FrozenReduct::{freeze,new,is_satisfied_by}` and `Interpretation::new` take
`VectorReservation`. No other declaration
receives either parameter. These are authored changes to the Lean signatures,
not stock translator output or debug-name normalization. Every executable body
remains unchanged.

Neither class has a global instance or assumes success. The supplied operations
retain backend failure and divergence; reservation also retains its source-level
success/refusal and returned vector. Needed value, sequence, owner or capacity
contracts belong in the consuming theorem. The active vector records logical
contents and a length bound, not physical capacity. `Arc::new` allocation failure
is not converted into a typed admission refusal.

Each fixed provider is a pure function. Repeated calls on equal represented
inputs do not model different allocator outcomes or fresh allocation histories.
The proofs concern supplied invocations, including a returning reservation
refusal; they do not install the separate `RuntimeEffects` event specification.
The public checker uses one supplied reservation provider for all four calls;
the owned wrapper passes the same provider to it.
Equal represented inputs therefore have equal modeled results. This preserves
successful logical outputs under the sequence-preservation contract, but does not
establish correspondence to mixed success/refusal histories or changing runtime
observations.

Keep the raw generated file, check its identity, insert exactly the eight binders,
and verify that removing them recovers the raw bytes:

```sh
printf '%s\n' 'a4dca2591195c0bf381f16f7c187ff773ba3cf353e0f4e6f290a4356541f7daf  target/replay/Evaluator/Funs.lean' | shasum -a 256 -c -
awk '
function begin_scope(name, provider) {
  if (active != "" || seen[name] != 0) exit 1
  print "section " name
  print "variable [" provider "]"
  print ""
  active = name
  seen[name]++
}
/^\/-- / {
  if (active != "") {
    print "end " active
    print ""
    active = ""
  }
  if ($0 ~ /^\/-- \[zetesis_ferraris::theory::\{zetesis_ferraris::theory::Theory\}::new\]:$/)
    begin_scope("TheoryAllocation", "ArcAllocation")
  if ($0 ~ /^\/-- \[zetesis_ferraris::oracle::reserve\]:$/)
    begin_scope("StorageReservation", "VectorReservation")
  if ($0 ~ /^\/-- \[zetesis_ferraris::oracle::check\]:$/)
    begin_scope("CheckReservation", "VectorReservation")
  if ($0 ~ /^\/-- \[zetesis_ferraris::checked::check_interpretation\]:$/)
    begin_scope("SubjectReservation", "VectorReservation")
  if ($0 ~ /^\/-- \[zetesis_ferraris::reduct::.*::freeze\]:$/)
    begin_scope("FreezeReservation", "VectorReservation")
  if ($0 ~ /^\/-- \[zetesis_ferraris::reduct::.*::new\]:$/)
    begin_scope("ReductReservation", "VectorReservation")
  if ($0 ~ /^\/-- \[zetesis_ferraris::reduct::.*::is_satisfied_by\]:$/)
    begin_scope("QueryReservation", "VectorReservation")
  if ($0 ~ /^\/-- \[zetesis_ferraris::theory::\{zetesis_ferraris::theory::Interpretation\}::new\]:$/)
    begin_scope("InterpretationReservation", "VectorReservation")
}
/^end ZetesisExtract$/ {
  if (active != "") {
    print "end " active
    print ""
    active = ""
  }
}
{ print }
END {
  if (active != "" || seen["TheoryAllocation"] != 1 || seen["StorageReservation"] != 1 ||
      seen["FreezeReservation"] != 1 || seen["ReductReservation"] != 1 || seen["QueryReservation"] != 1 ||
      seen["InterpretationReservation"] != 1 || seen["CheckReservation"] != 1 || seen["SubjectReservation"] != 1) exit 1
}
' target/replay/Evaluator/Funs.lean > target/replay/Funs.bound.lean
printf '%s\n' 'e0c4979d76f69b62e4eef335f1bca02c514bf2eb4f9c3587b5c3ee4f20ca6d37  target/replay/Funs.bound.lean' | shasum -a 256 -c -
awk '
BEGIN {
  provider["TheoryAllocation"] = "ArcAllocation"
  provider["StorageReservation"] = "VectorReservation"
  provider["CheckReservation"] = "VectorReservation"
  provider["SubjectReservation"] = "VectorReservation"
  provider["FreezeReservation"] = "VectorReservation"
  provider["ReductReservation"] = "VectorReservation"
  provider["QueryReservation"] = "VectorReservation"
  provider["InterpretationReservation"] = "VectorReservation"
}
$1 == "section" && ($2 in provider) {
  if (NF != 2 || active != "" || seen[$2] != 0) exit 1
  active = $2
  if (getline <= 0 || $0 != "variable [" provider[active] "]") exit 1
  if (getline <= 0 || $0 != "") exit 1
  seen[active]++
  next
}
$1 == "end" && ($2 in provider) {
  if (NF != 2 || active != $2) exit 1
  if (getline <= 0 || $0 != "") exit 1
  active = ""
  next
}
{ print }
END {
  if (active != "") exit 1
  for (name in provider) if (seen[name] != 1) exit 1
}
' target/replay/Funs.bound.lean > target/replay/Funs.restored.lean
cmp target/replay/Evaluator/Funs.lean target/replay/Funs.restored.lean
cmp Evaluator/Funs.lean target/replay/Funs.bound.lean
```

Do not install generated external templates. `PureExternals`, `OwnerExternals`, `AtomicTypes`,
`AtomicLoad`, `ArcAllocation`, `VectorReservation` and `UsizeCeiling` supply the concrete, scoped
models reviewed by these proofs.
Regeneration does not establish external-model correspondence to Rust.
The opaque `TryReserveError` token represents no allocator internals.
`VectorReservation` supplies the reservation operation; its interface alone
proves neither success nor preservation of logical contents or capacity. The
private-query theorem requires a represented mask; separate producer proofs
connect that invariant to actual freezing. Reservation and public frozen queries
use the explicit supplied operation above. Repeat the strict build and complete
authored-theorem audit after accepting any changed extraction.

## Current subset-search extraction limit

The allocation-free `find_countermodel` phase and its subset-query and carry
helpers now translate directly. Their exported bodies are the production
operations, with no generated-code rewriting or success assumptions for missing
externals. Translation alone does not prove the search invariant, termination,
proper-subset coverage or final membership verdict.

The public `oracle::check` wrapper is selected directly with its owner check,
initial poll, four reservations, original-model branch and proper-subset search.
The owned `check_interpretation` wrapper now calls that operation on its retained
candidate's theory and moves the candidate into the returned decision. Its
acceptance, verdict/statistics, checked-subject, stable conversion and stable
subject/theory accessors are selected too. The additions are these operations,
three directly used `Check` accessors, and the two checked/stable record types.
All preceding generated declarations and external templates are unchanged.
Completed-call proof scope is recorded separately in `verification.json`;
extraction alone establishes no verdict theorem.

`VectorReservation` permits returning typed reservation failures, but the active
sequence model has no capacity field or evolving allocator history. One provider
is shared by the wrapper's calls. Do not install generated axiom templates or
replace reservation with an always-success function.

The selected owner check and clone use explicit owner tokens. Immutable
heap consistency connects a successful owner comparison to equal stored theory
data. Allocator internals and reference counting remain trusted library
implementations; runtime correspondence is distinct from generated-call proofs.

`RuntimeEffects` specifies returning read and reservation events separately.
The current extracted `Result`, callback traits and loop interfaces still use
the fixed-observation model. Connecting the richer effect to generated calls,
then rechecking that dependency closure, remains necessary.

`Theory::new` runs `admit`, then returns its refusal unchanged or passes its
admitted data to `Arc::new`. The constructor, admission step and validators are
now selected directly. `AdmissionValidation` and `AdmittedData` prove the
validators and admission step; constructor proofs use the explicit allocation
binding above. Any stored-value or owner conclusion depends on the stated
contract for that invocation's returned allocation. `Interpretation::new` and
its insertion loop are now selected directly. The constructor retains reservation
and zero resizing before iterator conversion, checks each atom before its packed
write, and clones the theory only after successful insertion. Its trusted
`usize::div_ceil` binding uses checked scalar conversion and preserves division by
zero failure. Reservation success alone does not imply an empty returned vector;
zero initialization requires the explicit sequence-preservation contract.
No allocator, reference-count or constructor runtime correspondence follows from
extraction or supplied-operation bindings alone.
