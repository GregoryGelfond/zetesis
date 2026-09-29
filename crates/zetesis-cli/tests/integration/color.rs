//! Styling changes human presentation without changing semantic/publication evidence.

use std::io;
use std::process::{Command, Stdio};

use clap::Parser;
use zetesis_cli::{
    ColorMode, Completion, Options, RunError, run_detailed_with_diagnostics,
    run_finalized_with_diagnostics,
};
use zetesis_cpu::Cancellation;
use zetesis_test_support::io::BoundedWriter;

/// One worker: the bytes of two runs are compared, and several walkers of
/// the region tree deliver models in the schedule's order.
fn options(arguments: &[&str]) -> Options {
    Options::try_parse_from(
        [
            "zetesis",
            "--backend",
            "cpu",
            "--workers",
            "1",
            "--models",
            "0",
        ]
        .into_iter()
        .chain(arguments.iter().copied()),
    )
    .unwrap()
}

fn output(source: &str, options: &Options) -> String {
    let mut bytes = Vec::new();
    let report = run_detailed_with_diagnostics(
        source.into(),
        options,
        &mut bytes,
        &mut io::sink(),
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(report.completion, Completion::Exhausted);
    String::from_utf8(bytes).unwrap()
}

fn without_styles(text: &str) -> String {
    let mut plain = text.to_owned();
    for escape in [
        "\u{1b}[1;36m",
        "\u{1b}[22;36m",
        "\u{1b}[1;3;90m",
        "\u{1b}[3;32m",
        "\u{1b}[34m",
        "\u{1b}[3;90m",
        "\u{1b}[0m",
    ] {
        plain = plain.replace(escape, "");
    }
    assert!(!plain.contains('\u{1b}'), "unexpected terminal sequence");
    plain
}

#[test]
fn automatic_color_requires_an_eligible_terminal() {
    assert_eq!(options(&[]).color, ColorMode::Auto);
    for (terminal, disabled, expected) in [
        (true, false, ColorMode::Always),
        (true, true, ColorMode::Never),
        (false, false, ColorMode::Never),
        (false, true, ColorMode::Never),
    ] {
        assert_eq!(ColorMode::Auto.resolve(terminal, disabled), expected);
    }
}

#[test]
fn explicit_color_policy_overrides_terminal_detection() {
    for mode in [ColorMode::Always, ColorMode::Never] {
        for terminal in [false, true] {
            for disabled in [false, true] {
                assert_eq!(mode.resolve(terminal, disabled), mode);
            }
        }
    }
}

#[test]
fn styled_model_headings_leave_atom_lines_plain() {
    for source in ["a.", "a. #show seen : a."] {
        let plain = output(source, &options(&["--color", "never"]));
        let colored = output(source, &options(&["--color", "always"]));
        let mut lines = colored.lines();
        assert_eq!(
            lines.next(),
            Some("\u{1b}[1;36mAnswer:\u{1b}[22;36m 1\u{1b}[0m")
        );
        assert_eq!(lines.next(), plain.lines().nth(1));
        assert_eq!(lines.next(), Some("\u{1b}[1;3;90mSATISFIABLE\u{1b}[0m"));
        assert_eq!(
            lines.next(),
            Some("\u{1b}[34mCoverage:\u{1b}[3;90m exhausted\u{1b}[0m")
        );
        let models = lines.next().unwrap();
        assert!(models.starts_with("\u{1b}[34mModels:\u{1b}[3;90m "));
        assert!(models.ends_with("\u{1b}[0m"));
        assert_eq!(lines.next(), None);
        assert_eq!(without_styles(&colored), plain);
    }
}

#[test]
fn objective_metadata_resets_its_style_before_the_status() {
    let source = "a. #minimize{2:a}.";
    let plain = output(source, &options(&["--color", "never"]));
    let colored = output(source, &options(&["--color", "always"]));
    assert!(plain.contains("Optimization: 2\n"));
    assert!(colored.starts_with(
        "\u{1b}[1;36mAnswer:\u{1b}[22;36m 1\u{1b}[0m\na\n\u{1b}[3;32mOptimization: 2\u{1b}[0m\n"
    ));
    let ties = colored.lines().nth(3).unwrap();
    assert!(ties.starts_with("\u{1b}[34mIncumbent ties:\u{1b}[3;90m "));
    assert!(ties.ends_with("\u{1b}[0m"));
    assert!(colored.contains("\u{1b}[0m\n\u{1b}[1;3;90mOPTIMUM FOUND\u{1b}[0m\n\u{1b}[34mCoverage:\u{1b}[3;90m exhausted\u{1b}[0m\n"));
    assert_eq!(without_styles(&colored), plain);
}

#[test]
fn automatic_library_output_is_plain() {
    assert_eq!(
        output("a.", &options(&[])),
        output("a.", &options(&["--color", "never"]))
    );
}

#[test]
fn color_policy_cannot_change_json() {
    for source in ["a.", ":-.", "a. #minimize{2:a}."] {
        let plain = output(source, &options(&["--json", "--color", "never"]));
        let colored = output(source, &options(&["--json", "--color", "always"]));
        let value: serde_json::Value = serde_json::from_str(&colored).unwrap();
        assert_eq!(value["outcome"]["completion"], "exhausted");
        assert_eq!(colored, plain);
        assert!(!colored.contains('\u{1b}'));
    }
}

#[test]
fn satisfiable_status_is_untagged_styled_metadata() {
    for source in ["a.", "a|b."] {
        let rendered = output(source, &options(&["--color", "always"]));
        assert!(
            rendered
                .lines()
                .any(|line| line == "\u{1b}[1;3;90mSATISFIABLE\u{1b}[0m")
        );
        assert!(
            rendered.contains("\u{1b}[0m\n\u{1b}[34mCoverage:\u{1b}[3;90m exhausted\u{1b}[0m\n")
        );
        assert_eq!(
            without_styles(&rendered),
            output(source, &options(&["--color", "never"]))
        );
    }
}

#[test]
fn unsatisfiable_status_is_untagged_styled_metadata() {
    for source in [":-.", "1{}1."] {
        let rendered = output(source, &options(&["--color", "always"]));
        assert!(
            rendered.starts_with("\u{1b}[1;3;90mUNSATISFIABLE\u{1b}[0m\n\u{1b}[34mCoverage:\u{1b}[3;90m exhausted\u{1b}[0m\n")
        );
        assert_eq!(
            without_styles(&rendered),
            output(source, &options(&["--color", "never"]))
        );
    }
}

#[test]
fn requested_model_completion_styles_its_status() {
    let mut settings = options(&["--color", "always"]);
    settings.models = 1;
    for source in ["a.", "a|b."] {
        let mut bytes = Vec::new();
        let report = run_detailed_with_diagnostics(
            source.into(),
            &settings,
            &mut bytes,
            &mut io::sink(),
            &Cancellation::default(),
        )
        .unwrap();
        assert_eq!(report.completion, Completion::RequestedModels);
        let colored = String::from_utf8(bytes).unwrap();
        assert!(colored.contains(
            "\u{1b}[1;3;90mSATISFIABLE\u{1b}[0m\n\u{1b}[34mCoverage:\u{1b}[3;90m partial"
        ));
        settings.color = ColorMode::Never;
        let mut plain = Vec::new();
        let plain_report = run_detailed_with_diagnostics(
            source.into(),
            &settings,
            &mut plain,
            &mut io::sink(),
            &Cancellation::default(),
        )
        .unwrap();
        settings.color = ColorMode::Always;
        assert_eq!(plain_report.completion, Completion::RequestedModels);
        assert_eq!(without_styles(&colored).as_bytes(), plain);
    }
}

#[test]
fn partial_status_output_cannot_acknowledge_summary() {
    let settings = options(&["--color", "always"]);
    for source in ["a.", ":-.", "a|b.", "1{}1."] {
        let complete = output(source, &settings);
        let start = complete.find("\u{1b}[1;3;90m").unwrap();
        let end = complete[start..].find('\n').unwrap() + start + 1;
        for maximum in start..end {
            let mut prefix = BoundedWriter::new(maximum);
            let failure = run_finalized_with_diagnostics(
                source.into(),
                &settings,
                &mut prefix,
                &mut io::sink(),
                &Cancellation::default(),
            )
            .unwrap_err();
            assert!(matches!(*failure.cause, RunError::Output(_)));
            assert_eq!(
                failure.semantic().unwrap().completion(),
                Some(Completion::Exhausted)
            );
            assert!(!failure.publication().unwrap().summary());
            assert_eq!(prefix.bytes(), &complete.as_bytes()[..maximum]);
        }
    }
}

#[test]
fn styling_bytes_obey_the_complete_record_limit() {
    let mut settings = options(&["--color", "always"]);
    let full = output("a.", &settings);
    let record_end = full.find("\na\n").unwrap() + "\na\n".len();
    settings.max_observation_bytes = record_end;
    assert_eq!(output("a.", &settings), full);
    settings.max_observation_bytes -= 1;
    let mut bytes = Vec::new();
    let failure = run_detailed_with_diagnostics(
        "a.".into(),
        &settings,
        &mut bytes,
        &mut io::sink(),
        &Cancellation::default(),
    )
    .unwrap_err();
    assert!(matches!(
        *failure.cause,
        RunError::ObservationOutputLimit { .. }
    ));
    assert!(bytes.is_empty());
    assert_eq!(failure.partial_report.unwrap().published_models, 0);
}

#[test]
fn interrupted_styled_records_are_not_published_models() {
    let settings = options(&["--color", "always"]);
    let source = "a. #minimize{2:a}.";
    let complete = output(source, &settings);
    let marker = "Optimization: 2\u{1b}[0m\n";
    let record_end = complete.find(marker).unwrap() + marker.len();
    for maximum in 0..record_end {
        let mut prefix = BoundedWriter::new(maximum);
        let failure = run_detailed_with_diagnostics(
            source.into(),
            &settings,
            &mut prefix,
            &mut io::sink(),
            &Cancellation::default(),
        )
        .unwrap_err();
        assert!(matches!(*failure.cause, RunError::Output(_)));
        assert_eq!(failure.partial_report.unwrap().published_models, 0);
        assert_eq!(prefix.bytes(), &complete.as_bytes()[..maximum]);
    }
}

#[test]
fn redirected_process_output_defaults_to_plain_text() {
    let result = Command::new(env!("CARGO_BIN_EXE_zetesis"))
        .args(["--backend", "cpu", "--models", "0"])
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/network-repair.lp"
        ))
        .env_remove("NO_COLOR")
        .env("TERM", "xterm-256color")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(result.status.success());
    assert!(result.stdout.starts_with(b"Answer: 1\n"));
    assert!(!result.stdout.contains(&0x1b));
}
