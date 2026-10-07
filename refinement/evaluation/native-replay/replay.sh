#!/bin/sh
set -eu
repository_dir="$(git rev-parse --show-toplevel)"
package_dir="$PWD"
replay_dir=${1:-$package_dir/target/native-replay}
if [ -e "$replay_dir" ]; then
    printf '%s\n' 'Native replay output must be a new directory.' >&2
    exit 2
fi
mkdir -p "$replay_dir"
(cd "$repository_dir" && shasum -a 256 -c refinement/evaluation/native-source-inputs.sha256)
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
  --exclude 'zetesis_ferraris::theory::{zetesis_ferraris::theory::FormulaTransaction}::new' \
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
  --start-from zetesis_ferraris::tight::compile::classify \
  --start-from zetesis_ferraris::atomic_choice::atom \
  --start-from zetesis_ferraris::atomic_choice::pair \
  --start-from zetesis_ferraris::tight::compile::producer \
  --include zetesis_ferraris \
  --include zetesis_cpu::cancellation \
  --include zetesis_ferraris::reduct::FrozenReduct \
  --include zetesis_ferraris::reduct::_::satisfied_by \
  --include zetesis_ferraris::reduct::_::theory \
  --exclude zetesis_ferraris::reduct::_::candidate \
  --exclude zetesis_ferraris::reduct::_::fmt \
  --dest-file "$replay_dir/native.raw.llbc" \
  -- --manifest-path ../../crates/zetesis-ferraris/Cargo.toml --lib --locked --offline

jq -cae '
  .translated.options.dest_file = "evaluator.llbc"
  | .translated.files |= map(.contents = null)
  | .translated.short_names |= sort_by(.key)
' "$replay_dir/native.raw.llbc" > "$replay_dir/native.source.llbc"
printf '%s  %s\n' '91bf2bf73a2a30a3445328c834eb6a9853700123a0f6a31e87a217acdaa62586' \
  "$replay_dir/native.source.llbc" | shasum -a 256 -c -
jq -cae --slurpfile inventory native-replay/inventory.json \
  -f native-replay/normalize.jq "$replay_dir/native.source.llbc" > "$replay_dir/native.llbc"
jq -ce --slurpfile inventory native-replay/inventory.json \
  --slurpfile source "$replay_dir/native.source.llbc" \
  --slurpfile raw "$replay_dir/native.raw.llbc" \
  -f native-replay/restore-check.jq "$replay_dir/native.llbc" > "$replay_dir/normalization.json"
printf '%s  %s\n' '203f6aaf05911572df32080e7ea0a91dab6ebfc1356a63c648382e5908e7878a' \
  "$replay_dir/native.llbc" | shasum -a 256 -c -
.lake/aeneas/aeneas -backend lean -dest "$replay_dir/stock" \
  -split-files -subdir Native -namespace ZetesisNativeExtract -all-computable \
  -no-progress-bar -sequential -warnings-as-errors -abort-on-error "$replay_dir/native.llbc"
cmp Native/Types.lean "$replay_dir/stock/Native/Types.lean"
jq -Rsj '
  "-- Explicit providers for the translated external allocation operations.\nvariable [ArcAllocation] [VectorReservation]\n\n" as $block |
  if (split($block) | length) != 2 then error("expected exactly one provider block")
  else split($block) | join("") end
' Native/Funs.lean > "$replay_dir/restored-Funs.lean"
cmp "$replay_dir/restored-Funs.lean" "$replay_dir/stock/Native/Funs.lean"
shasum -a 256 "$replay_dir/native.raw.llbc" "$replay_dir/native.source.llbc" \
  "$replay_dir/native.llbc" "$replay_dir/stock/Native/Types.lean" \
  "$replay_dir/stock/Native/Funs.lean" > "$replay_dir/artifacts.sha256"
