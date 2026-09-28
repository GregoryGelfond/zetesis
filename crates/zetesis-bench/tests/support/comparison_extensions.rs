//! Extended comparisons reuse the ordinary runner with independent fixtures.
use super::{Fixture, Path, Value, executable, fs};
use zetesis_validation::performance::{self, Decision, Phase, Schedule};

const SHORTEST: &str = "scenarios/shortest-path/variant-01/01-basic.lp";
const NO_PATH: &str = "scenarios/shortest-path/variant-01/04-no-path.lp";

fn selected(fixture: &Fixture) -> performance::Request<'_> {
    let mut request = fixture.request();
    request.schedule = Schedule::for_cases(vec![SHORTEST.into(), NO_PATH.into()], 0, 1).unwrap();
    request
}

#[test]
fn explicit_cases_preserve_manifest_contracts() {
    let fixture = Fixture::new("", |_| {});
    let report = performance::run(&selected(&fixture)).unwrap();
    assert!(report.passed(), "{:?}", report.faults());
    assert_eq!(report.samples().len(), 10);
    assert!(
        report
            .samples()
            .iter()
            .filter(|sample| sample.slot().case.path() == NO_PATH)
            .all(|sample| sample.selected_models() == Some(0))
    );
}

#[test]
fn selected_reports_identify_their_schema() {
    let fixture = Fixture::new("", |_| {});
    let report = performance::run(&selected(&fixture)).unwrap();
    report.publish().unwrap();
    let json: Value = serde_json::from_slice(&fs::read(&fixture.report).unwrap()).unwrap();
    assert_eq!(json["schema"], 2);
    assert_eq!(json["schedule"]["selected"][0]["selected"], SHORTEST);
}

#[test]
fn unknown_selected_cases_fail_before_launch() {
    let fixture = Fixture::new("", |_| {});
    let mut request = fixture.request();
    request.schedule = Schedule::for_cases(vec!["not-a-case.lp".into()], 0, 1).unwrap();
    assert!(matches!(
        performance::run(&request),
        Err(performance::Error::Configuration(_))
    ));
    assert!(!fixture.report.exists());
}

#[test]
fn selected_paths_require_unique_normal_components() {
    for paths in [
        vec![],
        vec!["../escape.lp".into()],
        vec!["/absolute.lp".into()],
        vec!["same.lp".into(), "same.lp".into()],
        vec!["a".repeat(1025)],
    ] {
        assert!(Schedule::for_cases(paths, 0, 1).is_err());
    }
}

#[test]
fn memory_positions_leave_direct_positions_unchanged() {
    let schedule = Schedule::for_cases(vec![SHORTEST.into(), NO_PATH.into()], 1, 2).unwrap();
    let memory = schedule.clone().with_memory(2).unwrap();
    assert_eq!(
        memory
            .slots()
            .into_iter()
            .filter(|slot| slot.phase != Phase::Memory)
            .collect::<Vec<_>>(),
        schedule.slots()
    );
    assert_eq!(memory.expected_samples(), schedule.expected_samples() + 8);
}

#[test]
fn resource_runs_require_a_fresh_helper() {
    let fixture = Fixture::new("", |_| {});
    let mut request = selected(&fixture);
    request.schedule = request.schedule.with_memory(1).unwrap();
    assert!(matches!(
        performance::run(&request),
        Err(performance::Error::Configuration(_))
    ));
}

#[test]
fn resource_helper_paths_cannot_use_implicit_search() {
    let fixture = Fixture::new("", |_| {});
    let marker = fixture.directory.path().join("must-not-launch");
    // Even metadata commands must wait until every executable owner is
    // admitted. This marker distinguishes refusal from a partial campaign.
    let body = format!("printf launched > {}; exit 0", super::quote(&marker));
    executable(&fixture.native, &body);
    executable(&fixture.reference, &body);
    let mut request = selected(&fixture);
    request.schedule = request.schedule.with_memory(1).unwrap();
    let error = performance::run_with_runner(&request, Path::new("zetesis-perf")).unwrap_err();
    assert!(matches!(
        error,
        performance::Error::Configuration("memory helper must be absolute")
    ));
    assert!(!marker.exists());
    assert!(!fixture.report.exists());
}

#[test]
fn memory_samples_retain_independent_solver_evidence() {
    let fixture = Fixture::new("", |_| {});
    let mut request = selected(&fixture);
    request.schedule = request.schedule.with_memory(1).unwrap();
    let report =
        performance::run_with_runner(&request, Path::new(env!("CARGO_BIN_EXE_zetesis-bench")))
            .unwrap();
    assert!(report.passed(), "{:?}", report.samples());
    let memory: Vec<_> = report
        .samples()
        .iter()
        .filter(|sample| sample.slot().phase == Phase::Memory)
        .collect();
    assert_eq!(memory.len(), 4);
    assert!(memory.iter().all(|sample| {
        sample
            .memory()
            .is_some_and(zetesis_validation::process::memory::Measurement::valid)
            && sample.memory_record().is_some()
    }));
    assert!(
        report
            .samples()
            .iter()
            .filter(|sample| sample.slot().phase == Phase::Timed)
            .all(|sample| sample.memory().is_none()
                && sample.capture().executable() != Path::new(env!("CARGO_BIN_EXE_zetesis-bench")))
    );
}

#[test]
fn invalid_memory_records_remain_inspectable() {
    let fixture = Fixture::new("", |_| {});
    let helper = fixture.directory.path().join("malformed-helper");
    executable(&helper, "printf '{broken' > \"$2\"; exit 0");
    let mut request = selected(&fixture);
    request.schedule = request.schedule.with_memory(1).unwrap();
    let report = performance::run_with_runner(&request, &helper).unwrap();
    assert!(!report.passed());
    let last = report.samples().last().unwrap();
    assert_eq!(last.decision(), Decision::InvalidMemory);
    assert_eq!(last.memory_record(), Some(b"{broken".as_slice()));
}

fn native_metadata(fixture: &Fixture, branch: &str) {
    let script = fs::read_to_string(&fixture.native).unwrap();
    fs::write(
        &fixture.native,
        script.replace(
            "if [ \"$#\" -eq 1 ]; then echo native-fixture; exit 0; fi",
            branch,
        ),
    )
    .unwrap();
}

#[test]
fn styled_short_help_selects_full_defaults_evidence() {
    let fixture = Fixture::new("", |_| {});
    native_metadata(
        &fixture,
        r#"if [ "$#" -eq 1 ]; then case "$1" in --help) printf '\033[34m--help-all\033[0m\n' >&2;; --help-all) echo complete-defaults;; *) echo version;; esac; exit 0; fi"#,
    );
    let report = performance::run(&selected(&fixture)).unwrap();
    assert!(report.passed());
    assert!(
        report
            .metadata()
            .iter()
            .any(|capture| capture.stdout() == b"complete-defaults\n")
    );
}

#[test]
fn unadvertised_full_help_preserves_legacy_defaults() {
    let fixture = Fixture::new("", |_| {});
    let report = performance::run(&selected(&fixture)).unwrap();
    assert!(report.passed());
    assert_eq!(report.metadata().len(), 4);
    assert!(report.metadata().iter().all(|capture| {
        !capture
            .arguments()
            .iter()
            .any(|argument| argument == "--help-all")
    }));
}

#[test]
fn full_help_failure_prevents_solver_launch() {
    let fixture = Fixture::new("", |_| {});
    native_metadata(
        &fixture,
        "if [ \"$#\" -eq 1 ]; then case \"$1\" in --help) echo --help-all;; --help-all) printf retained >&2; exit 9;; *) echo version;; esac; exit 0; fi",
    );
    let report = performance::run(&selected(&fixture)).unwrap();
    assert!(!report.passed());
    assert!(report.samples().is_empty());
    assert_eq!(report.metadata().last().unwrap().stderr(), b"retained");
}

#[test]
fn full_help_obeys_the_process_output_ceiling() {
    let fixture = Fixture::new("", |_| {});
    native_metadata(
        &fixture,
        "if [ \"$#\" -eq 1 ]; then case \"$1\" in --help) echo --help-all;; --help-all) printf '012345678901234567890123456789';; *) echo version;; esac; exit 0; fi",
    );
    let mut request = selected(&fixture);
    request.limits.process.max_output_bytes = 20;
    let report = performance::run(&request).unwrap();
    assert!(!report.passed());
    assert_eq!(
        report.metadata().last().unwrap().stop(),
        Some(zetesis_validation::process::Stop::OutputLimit)
    );
    assert_eq!(
        report.total_capture_bytes(),
        report
            .metadata()
            .iter()
            .map(|capture| capture.stdout().len() + capture.stderr().len())
            .sum::<usize>()
    );
}

#[test]
fn selected_include_mutation_prevents_campaign_setup() {
    let fixture = Fixture::new("", |_| {});
    let corpus = zetesis_validation::examples::load(
        &fixture.corpus,
        zetesis_validation::examples::Limits::default(),
    )
    .unwrap();
    let source = corpus
        .cases()
        .iter()
        .find(|case| case.path() == SHORTEST)
        .unwrap()
        .transitive_source_paths()
        .iter()
        .find(|path| path.as_str() != SHORTEST)
        .unwrap();
    fs::write(fixture.corpus.join(source), "changed.\n").unwrap();
    assert!(matches!(
        performance::run(&selected(&fixture)),
        Err(performance::Error::Corpus(_))
    ));
}

#[test]
fn explicit_selection_has_no_named_preset() {
    let schedule = Schedule::for_cases(vec![SHORTEST.into()], 0, 1).unwrap();
    assert_eq!(schedule.suite(), None);
}

#[test]
fn resource_summaries_keep_distinct_populations() {
    let fixture = Fixture::new("", |_| {});
    let mut request = selected(&fixture);
    request.schedule = request.schedule.with_memory(2).unwrap();
    let report =
        performance::run_with_runner(&request, Path::new(env!("CARGO_BIN_EXE_zetesis-bench")))
            .unwrap();
    let summaries = report.summaries().unwrap();
    assert_eq!(summaries.len(), 4);
    assert!(
        summaries
            .iter()
            .all(|summary| summary.wall_nanoseconds.samples == 1
                && summary.child_peak_rss_bytes.as_ref().unwrap().samples == 2)
    );
}

#[test]
fn incomplete_campaigns_publish_no_summary() {
    let fixture = Fixture::new("exit 4\n", |_| {});
    let report = performance::run(&selected(&fixture)).unwrap();
    assert!(report.summaries().is_none());
    report.publish().unwrap();
    let json: Value = serde_json::from_slice(&fs::read(&fixture.report).unwrap()).unwrap();
    assert!(json.get("summary").is_none());
}

#[test]
fn resource_record_cannot_identify_the_helper_as_solver() {
    let fixture = Fixture::new("", |_| {});
    let helper = fixture.directory.path().join("self-reporting-helper");
    let unit = if cfg!(target_os = "macos") {
        "bytes"
    } else {
        "kibibytes"
    };
    executable(
        &helper,
        &format!(
            r#"printf '{{"schema":1,"child":%s,"exit_code":0,"signal":null,"raw_max_rss":0,"raw_unit":"{unit}","peak_rss_bytes":0}}' "$$" > "$2"; exit 0"#
        ),
    );
    let mut request = selected(&fixture);
    request.schedule = request.schedule.with_memory(1).unwrap();
    let report = performance::run_with_runner(&request, &helper).unwrap();
    let last = report.samples().last().unwrap();
    assert_eq!(last.decision(), Decision::InvalidMemory);
    assert!(last.detail().unwrap().contains("helper identity"));
}

#[test]
fn resource_record_must_use_the_host_platform_unit() {
    let fixture = Fixture::new("", |_| {});
    let helper = fixture.directory.path().join("wrong-unit-helper");
    let unit = if cfg!(target_os = "macos") {
        "kibibytes"
    } else {
        "bytes"
    };
    executable(
        &helper,
        &format!(
            r#"printf '{{"schema":1,"child":2,"exit_code":0,"signal":null,"raw_max_rss":0,"raw_unit":"{unit}","peak_rss_bytes":0}}' > "$2"; exit 0"#
        ),
    );
    let mut request = selected(&fixture);
    request.schedule = request.schedule.with_memory(1).unwrap();
    let report = performance::run_with_runner(&request, &helper).unwrap();
    assert_eq!(
        report.samples().last().unwrap().decision(),
        Decision::InvalidMemory
    );
}

#[test]
fn cli_selected_memory_campaign_seals_its_runner() {
    let fixture = Fixture::new("", |_| {});
    let capture = super::cli_options(
        &fixture,
        "1",
        None,
        &[
            "--case".into(),
            SHORTEST.into(),
            "--memory-runs".into(),
            "1".into(),
        ],
    );
    assert_eq!(
        capture.exit().unwrap().code,
        Some(0),
        "{}",
        String::from_utf8_lossy(capture.stderr())
    );
    let report: Value = serde_json::from_slice(&fs::read(&fixture.report).unwrap()).unwrap();
    assert_eq!(report["passed"], true);
    assert_eq!(report["schema"], 2);
    assert_eq!(report["samples"].as_array().unwrap().len(), 7);
    assert_eq!(report["summary"].as_array().unwrap().len(), 2);
    assert!(
        report["before"]
            .as_array()
            .unwrap()
            .iter()
            .any(|seal| seal["requested"] == env!("CARGO_BIN_EXE_zetesis-bench"))
    );
}

#[test]
fn excessive_memory_populations_are_refused() {
    assert!(Schedule::new(0, 1).unwrap().with_memory(42).is_err());
}

#[test]
fn selected_capture_budget_includes_help_failures() {
    let fixture = Fixture::new("", |_| {});
    let mut request = selected(&fixture);
    request.limits.max_total_capture_bytes = 1;
    let report = performance::run(&request).unwrap();
    assert_eq!(report.total_capture_bytes(), 1);
    assert!(report.samples().is_empty());
    assert_eq!(report.metadata()[0].stdout(), b"n");
    assert!(!report.passed());
}
