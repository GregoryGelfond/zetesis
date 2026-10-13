//! The command-line grammar is the same in every build profile, so that the
//! default install's front and its GPU sibling read an invocation identically.
//! The test renders the whole command tree in a canonical form and compares it
//! with one committed record; it runs in every population that builds the CLI,
//! so a grammar that differs with a feature fails one of them.

use super::command;
use clap::{Arg, Command};
use std::fmt::Write as _;

/// The record, beside the crate's other test data.
const RECORD: &str = include_str!("../../tests/fixtures/command-tree.txt");

/// Arguments whose default the host determines, rendered as a fixed marker.
const HOST_DEFAULTS: [&str; 1] = ["memory"];

fn render(command: &Command, depth: usize, out: &mut String) {
    let indent = "  ".repeat(depth);
    let about = command.get_about().map(ToString::to_string).unwrap_or_default();
    let _ = writeln!(out, "{indent}command {} | {about}", command.get_name());
    for argument in command.get_arguments() {
        render_argument(argument, depth + 1, out);
    }
    for subcommand in command.get_subcommands() {
        render(subcommand, depth + 1, out);
    }
}

fn render_argument(argument: &Arg, depth: usize, out: &mut String) {
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
    let help = argument.get_help().map(ToString::to_string).unwrap_or_default();
    let long_help = argument
        .get_long_help()
        .map(ToString::to_string)
        .unwrap_or_default();
    let _ = writeln!(out, "{indent}  help {help:?}");
    let _ = writeln!(out, "{indent}  long {long_help:?}");
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
