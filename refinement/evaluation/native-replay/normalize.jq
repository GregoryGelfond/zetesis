# Guarded adaptation of the maintained REPRODUCING.md convention.
# $inventory is an explicit checked identity inventory for the pinned snapshot.
# No executable statement, function declaration or method index is rewritten.

def functions_match($expected):
  .translated as $t |
  all($expected.functions[];
    . as $entry |
    $t.fun_decls[$entry.id] as $function |
    $function.def_id == $entry.id
    and $function.item_meta.name == $entry.name
    and $function.src == $entry.src
    and $function.item_meta.is_local == true
    and $function.item_meta.opacity == "Transparent"
    and $function.body.Structured.locals.arg_count == $entry.arguments
    and ([$t.ordered_decls[] |
      select(. == {"Fun":{"NonRec":$entry.id}})] | length) == 1
    and (if $entry.rename then
      $function.body.Structured.locals.locals[$entry.rename.index].index == $entry.rename.index
      and $function.body.Structured.locals.locals[$entry.rename.index].name == $entry.rename.old
      and $function.body.Structured.locals.locals[$entry.rename.index].ty == $entry.rename.type
    else true end));

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
  and $debug.methods[0].skip_binder.id == 244
  and $t.fun_decls[244] == null
  and $debug.vtable.id == 15
  and $t.global_decls[15] == null
  and $t.ordered_decls[216] == {"TraitImpl":{"NonRec":25}}
  and ([$t.ordered_decls[] | select(. == {"TraitImpl":{"NonRec":25}})] | length) == 1
  and ([[$t.type_decls, $t.fun_decls, $t.global_decls, $t.trait_decls,
          ($t.trait_impls | to_entries | map(select(.key != 25) | .value))]
        | .. | objects | select(
            .TraitImpl? == 25 or .TraitImpl?.id? == 25
            or .impl_ref?.id? == 25 or .trait_impl?.id? == 25
            or .trait_impl_id? == 25
            or .Fun? == 244 or .Regular? == 244
            or .fun_id? == 244 or .function_id? == 244
            or .Global? == 15 or .Global?.id? == 15
            or .global_id? == 15)] | length) == 0;

def step_method($slot; $name; $function):
  .translated as $t |
  $t.trait_decls[7].methods[$slot] as $decl |
  $t.trait_impls[14].methods[$slot] as $impl |
  $decl.kind == {"TraitMethod":[7,$slot]}
  and $decl.skip_binder.name == $name
  and $impl.kind == {"TraitMethod":[7,$slot]}
  and $impl.skip_binder.id == $function
  and $t.fun_decls[$function].body == "Opaque"
  and $t.fun_decls[$function].item_meta.name == [
    {"Ident":["core",0]}, {"Ident":["iter",0]}, {"Ident":["range",0]},
    {"Impl":{"Trait":14}}, {"Ident":[$name,0]}]
  and $t.fun_decls[$function].src.TraitImpl.impl_ref.id == 14
  and $t.fun_decls[$function].src.TraitImpl.trait_ref.id == 7
  and $t.fun_decls[$function].src.TraitImpl.item_id == $slot;

def step_is_unused:
  .translated as $t |
  $t.trait_decls[7].item_meta.name == [
    {"Ident":["core",0]}, {"Ident":["iter",0]}, {"Ident":["range",0]},
    {"Ident":["Step",0]}]
  and $t.trait_impls[14].item_meta.name == [
    {"Ident":["core",0]}, {"Ident":["iter",0]}, {"Ident":["range",0]},
    {"Impl":{"Trait":14}}]
  and $t.trait_impls[14].impl_trait.id == 7
  and $t.trait_impls[14].impl_trait.generics.types == [{"Deduplicated":0}]
  and $t.item_names[1].value[2].Impl.Ty.params.const_generics[0].ty ==
    {"Value":[0,{"Scalar":{"Integer":{"Unsigned":"Usize"}}}]}
  and $t.trait_impls[14].vtable == null
  and step_method(2; "forward_overflowing"; 208)
  and step_method(6; "backward_overflowing"; 212)
  and ((.translated.trait_decls[7].methods[2] = null
    | .translated.trait_decls[7].methods[6] = null
    | .translated.trait_impls[14].methods[2] = null
    | .translated.trait_impls[14].methods[6] = null
    # Ignore only these two functions' own trait-registration source records
    # during the use check; those source records remain in the output.
    | .translated.fun_decls[208] |= del(.src)
    | .translated.fun_decls[212] |= del(.src)
    | [.translated.type_decls, .translated.fun_decls,
       .translated.global_decls, .translated.trait_decls,
       .translated.trait_impls]
    | walk(if type == "object" then del(.item_meta) else . end)
    | [.. | objects | select(
        .Fun? == 208 or .Fun? == 212
        or .Regular? == 208 or .Regular? == 212
        or .fun_id? == 208 or .fun_id? == 212
        or .function_id? == 208 or .function_id? == 212
        or .TraitMethod? == [7,2] or .TraitMethod? == [7,6]
        or (.trait_ref?.id? == 7 and (.item_id? == 2 or .item_id? == 6))
        or (.impl_ref?.id? == 14 and (.item_id? == 2 or .item_id? == 6)))])
      | length) == 0;

$inventory[0] as $expected |
if $expected.raw_sha256 != "f180f792197c60fb3c1b463d1bf2670beccdb8a961ac23bee657bb9d948a5f15"
then error("unexpected snapshot identity")
elif functions_match($expected) | not
then error("unexpected selected declaration, export order or local debug name")
elif debug_is_unused | not
then error("unexpected or referenced derived Debug registration")
elif step_is_unused | not
then error("unexpected or referenced Step compatibility method")
else
  reduce ($expected.functions[] | select(.rename)) as $entry
    (.; .translated.fun_decls[$entry.id].body.Structured.locals.locals[$entry.rename.index].name = $entry.rename.new)
  | .translated.trait_impls[25] = null
  | del(.translated.ordered_decls[216])
  | .translated.trait_decls[7].methods[2] = null
  | .translated.trait_decls[7].methods[6] = null
  | .translated.trait_impls[14].methods[2] = null
  | .translated.trait_impls[14].methods[6] = null
end
