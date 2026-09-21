//! Task-scoped CLI parsing. All solving arguments reuse the legacy typed options.

use std::ffi::OsString;
use std::num::NonZeroUsize;

use clap::{Arg, ArgAction, ArgMatches, Command, CommandFactory, FromArgMatches, Subcommand};

use crate::benchmark::BenchCommand;
use crate::testing::TestCommand;
use crate::{Options, StatisticsView};

const VERSION_INFORMATION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    " | Copyright (c) 2026 Gregory Gelfond | MIT License"
);

/// One parsed user task. Parsing does not load a source or initialize a device.
#[derive(Debug)]
pub enum Invocation {
    /// Solve the supplied source roots using the library's typed settings.
    Solve(Box<Options>),
    /// List available execution devices.
    Devices,
    /// Run a selected conformance check.
    Test(TestCommand),
    /// Measure workloads or compare retained measurements.
    Bench(BenchCommand),
}

impl Invocation {
    /// Parse commands and the retained file-first/option-first solving syntax.
    ///
    /// Bare invocation displays help. An explicit `solve` requires a file or
    /// `-` for standard input; legacy solving keeps its standard-input default.
    /// Help and version requests return clap's successful display errors, before
    /// any command can perform I/O other than rendering that text.
    ///
    /// # Errors
    /// Returns argument diagnostics, or `DisplayHelp`/`DisplayVersion` for a
    /// requested informational view. The caller owns writing these diagnostics.
    pub fn try_parse_from<I, T>(arguments: I) -> Result<Self, clap::Error>
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString>,
    {
        let mut arguments: Vec<OsString> = arguments.into_iter().map(Into::into).collect();
        if arguments.is_empty() {
            arguments.push("zetesis".into());
        }
        if arguments.len() == 1 {
            arguments.push("--help".into());
        }
        let first = arguments[1].to_str();
        let legacy = !matches!(
            first,
            Some(
                "solve"
                    | "test"
                    | "bench"
                    | "devices"
                    | "help"
                    | "version"
                    | "-h"
                    | "--help"
                    | "--help-all"
                    | "-V"
                    | "--version"
            )
        );
        if legacy || first == Some("--help-all") {
            arguments.insert(1, "solve".into());
        }
        let matches = command(legacy).try_get_matches_from(arguments)?;
        match matches.subcommand() {
            Some(("solve", matches)) => solve(matches, legacy),
            Some(("devices", _)) => Ok(Self::Devices),
            Some(("test", matches)) => TestCommand::from_arg_matches(matches).map(Self::Test),
            Some(("bench", matches)) => BenchCommand::from_arg_matches(matches).map(Self::Bench),
            Some(("help", matches)) => Err(help(matches)),
            // Use clap's informational formatter, not a raw error message.
            // This fixed flag exits parsing before subcommand dispatch.
            Some(("version", _)) => Self::try_parse_from(["zetesis", "--version"]),
            _ => Err(command(legacy).error(
                clap::error::ErrorKind::MissingSubcommand,
                "choose solve, test, bench, devices, help or version",
            )),
        }
    }
}

fn command(legacy: bool) -> Command {
    Command::new("zetesis")
        .version(VERSION_INFORMATION)
        .propagate_version(true)
        .about("Answer-set solving through the reduct")
        .subcommand_required(true)
        .disable_help_subcommand(true)
        .subcommand(solve_command(legacy))
        .subcommand(
            TestCommand::augment_subcommands(
                Command::new("test")
                    .subcommand_required(true)
                    .arg_required_else_help(true),
            )
            .about("Check example programs or an execution device"),
        )
        .subcommand(
            BenchCommand::augment_subcommands(
                Command::new("bench")
                    .subcommand_required(true)
                    .arg_required_else_help(true),
            )
            .about("Measure performance or compare saved reports"),
        )
        .subcommand(Command::new("devices").about("List available execution devices"))
        .subcommand(
            Command::new("help")
                .about("Show help for a command")
                .arg(Arg::new("topic").value_name("COMMAND").num_args(0..))
                .arg(
                    Arg::new("advanced")
                        .long("advanced")
                        .action(ArgAction::SetTrue)
                        .help("Show every solve control and its resource contract"),
                ),
        )
        .subcommand(Command::new("version").about("Show the program version"))
        .after_help(
            "Start with: zetesis solve program.lp\nUse 'zetesis help solve' for solving options.",
        )
}

fn solve_command(legacy: bool) -> Command {
    let options = Options::command();
    Command::new("solve")
        .about("Find answer sets; use - explicitly for standard input")
        .disable_help_flag(true)
        .args(options.get_arguments().cloned().map(solve_argument))
        .mut_arg("input", |argument| {
            if legacy {
                argument.index(1)
            } else {
                argument.index(1).default_value(None::<&str>).required(true)
            }
        })
        .mut_arg("additional_inputs", |argument| argument.index(2))
        .arg(
            Arg::new("answers")
                .long("answers")
                .value_name("N")
                .value_parser(clap::value_parser!(NonZeroUsize))
                .conflicts_with_all(["all", "models"])
                .help_heading("Answers")
                .help("Display at most N answer sets [default: 1]")
                .long_help("Display at most N answer sets (default: 1). Without objectives, stop after this many answers. Optimization still seeks a proved optimum, then displays up to N tied optima; an interrupted run may report unproved incumbents."),
        )
        .arg(
            Arg::new("all")
                .long("all")
                .action(ArgAction::SetTrue)
                .conflicts_with_all(["answers", "models"])
                .help_heading("Answers")
                .help("Enumerate all answer sets, or all tied optima when optimizing"),
        )
        .after_help("Examples:\n  zetesis solve program.lp --all\n  zetesis solve - --answers 3\n  zetesis help solve --advanced\n\nResource limits and interruption leave enumeration incomplete; they do not prove unsatisfiability.")
}

fn solve_argument(argument: Arg) -> Arg {
    let name = argument.get_id().as_str();
    match name {
        "models" => argument.hide(true),
        "backend" => argument
            .visible_alias(None::<&str>)
            .long("device")
            .alias("backend")
            .value_name("DEVICE")
            .help("Choose a device; auto currently selects CPU")
            .help_heading("Execution"),
        "workers" => argument
            .visible_alias(None::<&str>)
            .long("threads")
            .alias("workers")
            .hide_short_help(false)
            .help_heading("Execution"),
        "grounder" => argument.help_heading("Execution"),
        "memory" => argument
            .visible_alias(None::<&str>)
            .long("memory-budget")
            .alias("memory")
            .hide_short_help(false)
            .help_heading("Limits"),
        "time_limit" => argument.help_heading("Limits"),
        "stats" | "json" | "color" => argument.help_heading("Output"),
        "input" | "additional_inputs" => argument,
        "help" | "help-all" => argument.help_heading("Help"),
        _ => argument.help_heading("Advanced controls"),
    }
}

fn solve(matches: &ArgMatches, legacy: bool) -> Result<Invocation, clap::Error> {
    let mut options = Options::from_arg_matches(matches)?;
    if let Some(answers) = matches.get_one::<NonZeroUsize>("answers") {
        options.models = answers.get();
    } else if matches.get_flag("all") {
        options.models = 0;
    }
    if !legacy && !options.json {
        options.statistics_view = StatisticsView::Human;
    }
    Ok(Invocation::Solve(Box::new(options)))
}

fn help(matches: &ArgMatches) -> clap::Error {
    let mut arguments = vec![OsString::from("zetesis")];
    if let Some(topics) = matches.get_many::<String>("topic") {
        arguments.extend(topics.map(OsString::from));
    }
    arguments.push(
        if matches.get_flag("advanced") {
            "--help-all"
        } else {
            "--help"
        }
        .into(),
    );
    match command(false).try_get_matches_from(arguments) {
        Err(error) => error,
        Ok(_) => clap::Error::raw(
            clap::error::ErrorKind::DisplayHelp,
            command(false).render_help(),
        ),
    }
}
