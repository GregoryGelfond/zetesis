//! The command-line grammar is the same in every build profile, so that the
//! default install's front and its GPU sibling read an invocation identically.
//! The test renders the command tree in a canonical form and compares it with
//! one committed record; it runs in every population that builds the CLI, so
//! a feature-gated difference in what it renders fails one of them. It renders
//! what decides how an invocation is read (each command's settings, aliases
//! and argument groups; each argument's names, position, requirement, action,
//! value count, value names, delimiters, parser type, possible values,
//! defaults and conflicts) and its help text.

use super::command;
use clap::{Arg, Command};
use std::fmt::Write as _;

/// The record, beside the crate's other test data.
const RECORD: &str = include_str!("../../tests/fixtures/command-tree.txt");

/// Arguments whose default the host determines, rendered as a fixed marker.
const HOST_DEFAULTS: [&str; 1] = ["memory"];

fn render(command: &Command, depth: usize, out: &mut String) {
    let indent = "  ".repeat(depth);
    let about = command
        .get_about()
        .map(ToString::to_string)
        .unwrap_or_default();
    let _ = writeln!(out, "{indent}command {} | {about}", command.get_name());
    let settings = [
        ("subcommand_required", command.is_subcommand_required_set()),
        (
            "arg_required_else_help",
            command.is_arg_required_else_help_set(),
        ),
        ("disable_help_flag", command.is_disable_help_flag_set()),
        (
            "disable_version_flag",
            command.is_disable_version_flag_set(),
        ),
        (
            "disable_help_subcommand",
            command.is_disable_help_subcommand_set(),
        ),
        (
            "allow_external_subcommands",
            command.is_allow_external_subcommands_set(),
        ),
        (
            "args_conflicts_with_subcommands",
            command.is_args_conflicts_with_subcommands_set(),
        ),
        (
            "subcommand_negates_reqs",
            command.is_subcommand_negates_reqs_set(),
        ),
        (
            "subcommand_precedence_over_arg",
            command.is_subcommand_precedence_over_arg_set(),
        ),
        (
            "allow_missing_positional",
            command.is_allow_missing_positional_set(),
        ),
        ("trailing_var_arg", command.is_trailing_var_arg_set()),
        (
            "allow_negative_numbers",
            command.is_allow_negative_numbers_set(),
        ),
        ("args_override_self", command.is_args_override_self()),
    ];
    let set: Vec<&str> = settings
        .iter()
        .filter(|(_, set)| *set)
        .map(|(name, _)| *name)
        .collect();
    let aliases: Vec<&str> = command.get_all_aliases().collect();
    let _ = writeln!(out, "{indent}  settings={set:?} aliases={aliases:?}");
    for group in command.get_groups() {
        let members: Vec<&str> = group.get_args().map(clap::Id::as_str).collect();
        // ArgGroup's predicate for multiple takes it mutably.
        let multiple = group.clone().is_multiple();
        let _ = writeln!(
            out,
            "{indent}  group {} args={members:?} multiple={multiple} required={}",
            group.get_id(),
            group.is_required_set()
        );
    }
    for argument in command.get_arguments() {
        render_argument(command, argument, depth + 1, out);
    }
    for subcommand in command.get_subcommands() {
        render(subcommand, depth + 1, out);
    }
}

fn render_argument(command: &Command, argument: &Arg, depth: usize, out: &mut String) {
    let indent = "  ".repeat(depth);
    let id = argument.get_id().as_str();
    let defaults: Vec<String> = if HOST_DEFAULTS.contains(&id) {
        vec!["<host>".to_owned()]
    } else {
        argument
            .get_default_values()
            .iter()
            .map(|value| value.to_string_lossy().into_owned())
            .collect()
    };
    let values: Vec<String> = argument
        .get_possible_values()
        .iter()
        .map(|value| value.get_name().to_owned())
        .collect();
    let _ = writeln!(
        out,
        "{indent}arg {id} long={:?} short={:?} aliases={:?} values={values:?} defaults={defaults:?} \
         hidden={} short_help_hidden={} heading={:?}",
        argument.get_long(),
        argument.get_short(),
        argument.get_all_aliases(),
        argument.is_hide_set(),
        argument.is_hide_short_help_set(),
        argument.get_help_heading(),
    );
    let mut conflicts: Vec<&str> = command
        .get_arg_conflicts_with(argument)
        .iter()
        .map(|other| other.get_id().as_str())
        .collect();
    conflicts.sort_unstable();
    let flags = [
        ("required", argument.is_required_set()),
        ("global", argument.is_global_set()),
        ("exclusive", argument.is_exclusive_set()),
        ("last", argument.is_last_set()),
        ("trailing_var_arg", argument.is_trailing_var_arg_set()),
        ("require_equals", argument.is_require_equals_set()),
        ("ignore_case", argument.is_ignore_case_set()),
        ("allow_hyphen_values", argument.is_allow_hyphen_values_set()),
        (
            "allow_negative_numbers",
            argument.is_allow_negative_numbers_set(),
        ),
    ];
    let set: Vec<&str> = flags
        .iter()
        .filter(|(_, set)| *set)
        .map(|(name, _)| *name)
        .collect();
    let _ = writeln!(
        out,
        "{indent}  index={:?} action={:?} num_args={:?} value_names={:?} delimiter={:?} \
         terminator={:?} parser={} flags={set:?} conflicts={conflicts:?}",
        argument.get_index(),
        argument.get_action(),
        argument.get_num_args(),
        argument
            .get_value_names()
            .map(|names| names.iter().map(ToString::to_string).collect::<Vec<_>>()),
        argument.get_value_delimiter(),
        argument.get_value_terminator().map(ToString::to_string),
        parsed_type(argument),
    );
    let help = argument
        .get_help()
        .map(ToString::to_string)
        .unwrap_or_default();
    let long_help = argument
        .get_long_help()
        .map(ToString::to_string)
        .unwrap_or_default();
    let _ = writeln!(out, "{indent}  help {help:?}");
    let _ = writeln!(out, "{indent}  long {long_help:?}");
}

/// The value type `argument`'s parser produces, named by this table: clap
/// names a parser's type only where it was built with debug assertions. A type
/// missing from the table fails the test until it is named here.
fn parsed_type(argument: &Arg) -> &'static str {
    use std::any::TypeId;
    use std::num::{NonZeroU64, NonZeroUsize};
    let parsed = argument.get_value_parser().type_id();
    [
        (TypeId::of::<bool>(), "bool"),
        (TypeId::of::<u64>(), "u64"),
        (TypeId::of::<usize>(), "usize"),
        (TypeId::of::<NonZeroU64>(), "NonZeroU64"),
        (TypeId::of::<NonZeroUsize>(), "NonZeroUsize"),
        (TypeId::of::<String>(), "String"),
        (TypeId::of::<std::path::PathBuf>(), "PathBuf"),
        (TypeId::of::<crate::Backend>(), "Backend"),
        (TypeId::of::<crate::ColorMode>(), "ColorMode"),
        (TypeId::of::<crate::Grounder>(), "Grounder"),
        (TypeId::of::<crate::Oracle>(), "Oracle"),
        (TypeId::of::<crate::SearchMethod>(), "SearchMethod"),
        (TypeId::of::<crate::SourceBatching>(), "SourceBatching"),
        (
            TypeId::of::<zetesis_themelios::JoinStrategy>(),
            "JoinStrategy",
        ),
    ]
    .into_iter()
    .find_map(|(type_id, name)| (parsed == type_id).then_some(name))
    .unwrap_or_else(|| panic!("name the value type of {}'s parser", argument.get_id()))
}

/// The canonical form of both grammars the CLI parses: the task-scoped one and
/// the legacy file-first one.
fn canonical() -> String {
    let mut out = String::new();
    for legacy in [false, true] {
        let _ = writeln!(out, "legacy={legacy}");
        render(&command(legacy), 0, &mut out);
    }
    out
}

#[test]
fn the_grammar_is_the_recorded_one_in_every_profile() {
    let rendered = canonical();
    assert!(
        rendered == RECORD,
        "the command tree differs from tests/fixtures/command-tree.txt; if the change is \
         intended and holds in every profile, record this rendering:\n{rendered}"
    );
}
