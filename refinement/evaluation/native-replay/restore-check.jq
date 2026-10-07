# Check the exact inverse against both the normalized source and raw parsed
# snapshot, restoring only the explicitly changed paths from their originals.
$inventory[0] as $expected |
. as $adapted |
(reduce ($expected.functions[] | select(.rename)) as $entry
  (.; .translated.fun_decls[$entry.id].body.Structured.locals.locals[$entry.rename.index].name = $entry.rename.old)
 | .translated.trait_impls[25] = $source[0].translated.trait_impls[25]
 | .translated.ordered_decls = (.translated.ordered_decls[:216]
     + [$source[0].translated.ordered_decls[216]] + .translated.ordered_decls[216:])
 | .translated.trait_decls[7].methods[2] = $source[0].translated.trait_decls[7].methods[2]
 | .translated.trait_decls[7].methods[6] = $source[0].translated.trait_decls[7].methods[6]
 | .translated.trait_impls[14].methods[2] = $source[0].translated.trait_impls[14].methods[2]
 | .translated.trait_impls[14].methods[6] = $source[0].translated.trait_impls[14].methods[6]
) as $restored |
if $restored != $source[0]
then error("inverse differs from the complete parsed normalized source")
elif ($restored
 | .translated.options.dest_file = $raw[0].translated.options.dest_file
 | .translated.files = $raw[0].translated.files
 | .translated.short_names = $raw[0].translated.short_names) != $raw[0]
then error("inverse differs from the complete parsed raw snapshot")
elif ($adapted.translated.fun_decls | map(select(. != null) | .def_id)) !=
     ($raw[0].translated.fun_decls | map(select(. != null) | .def_id))
then error("function inventory changed")
else {
  reference_snapshot_sha256: $expected.raw_sha256,
  normalized_source_restored: true,
  complete_parsed_raw_restored: true,
  executable_bodies_preserved_except_local_debug_names: true,
  function_inventory_preserved: true,
  local_debug_names_changed: ($expected.rename_ids | length),
  unused_debug_impl_removed: 25,
  step_trait: 7,
  step_impl: 14,
  cleared_method_slots: [2,6]
} end
