//! Ordinary source execution preserves exact reduct, objective and publication contracts.
use clap::Parser;
use std::io;
use zetesis_cli::{
    Completion, Options, Oracle, Report, RunFailure, SolvePhase, run_detailed_with_diagnostics,
};
use zetesis_cpu::Control;

fn options(oracle: Oracle, workers: usize) -> Options {
    let mut o =
        Options::try_parse_from(["zetesis", "--backend", "cpu", "--models", "0", "--stats"])
            .unwrap();
    o.oracle = oracle;
    o.completion_workers = std::num::NonZeroUsize::new(workers).unwrap();
    o
}
fn solve(source: &str, o: &Options) -> (Report, String, String) {
    let mut out = Vec::new();
    let mut diag = Vec::new();
    let r =
        run_detailed_with_diagnostics(source.into(), o, &mut out, &mut diag, &Control::default())
            .unwrap();
    (
        r,
        String::from_utf8(out).unwrap(),
        String::from_utf8(diag).unwrap(),
    )
}
fn answers(text: &str) -> Vec<&str> {
    let mut a = text
        .lines()
        .collect::<Vec<_>>()
        .windows(2)
        .filter_map(|w| w[0].starts_with("Answer:").then_some(w[1]))
        .collect::<Vec<_>>();
    a.sort_unstable();
    a
}
#[test]
fn ordinary_certified_models_optimum_ties_and_hidden_displays_match_explicit_reduct() {
    for source in [
        "1{a;b}1.",
        "1{a;b}1. {hidden}. #minimize{1,a:a;1,b:b}. #show.",
        "1{a;b}1. #minimize{1,a:a;2,b:b}. #show picked:a.",
    ] {
        let (baseline, bout, _) = solve(source, &options(Oracle::Countermodel, 1));
        assert!(
            baseline
                .countermodel_statistics
                .unwrap()
                .certified
                .is_none()
        );
        for workers in [1, 4] {
            let (r, out, diag) = solve(source, &options(Oracle::Auto, workers));
            assert_eq!(r.completion, Completion::Exhausted);
            assert_eq!(answers(&out), answers(&bout));
            assert_eq!(r.models, baseline.models);
            assert_eq!(
                format!("{:?}", r.optimization),
                format!("{:?}", baseline.optimization)
            );
            let s = r.countermodel_statistics.unwrap();
            let c = s.certified.unwrap();
            assert!(c.plan.is_some());
            assert!(c.stable > 0);
            assert_eq!(s.countermodel_queries, 0);
            assert_eq!(c.failed, 0);
            assert!(diag.contains("Membership: checked tight support certificate"));
            assert!(diag.contains("tight certificate: eligible=true"));
            assert!(diag.contains("storage limit=268435456"));
            let timing = r.phase_timings.unwrap();
            assert_eq!(timing.get(SolvePhase::CertificateSetup).unwrap().calls, 1);
            assert_eq!(
                timing.get(SolvePhase::CertifiedMembership).unwrap().calls,
                c.checks
            );
            assert!(timing.get(SolvePhase::ExactReductMembership).is_none());
            if workers > 1 {
                let e = r.formula_execution.unwrap();
                assert_eq!(e.gpu_candidates, 0);
                assert!(e.adapter.is_empty());
                assert_eq!(e.pending_candidates, 0);
                assert_eq!(e.queued_models, 0);
                assert_eq!(e.completion.entered, s.candidates);
                assert_eq!(e.completion.residuals, 0);
            }
        }
    }
}
#[test]
fn unsupported_class_falls_back_and_explicit_general_oracle_keeps_comparison_path() {
    let source = "a|b. a:-b. b:-a.";
    let (general, expected, _) = solve(source, &options(Oracle::Countermodel, 1));
    let (auto, actual, diag) = solve(source, &options(Oracle::Auto, 1));
    assert_eq!(answers(&actual), answers(&expected));
    let s = auto.countermodel_statistics.unwrap();
    assert_eq!(
        s.countermodel_queries,
        general
            .countermodel_statistics
            .unwrap()
            .countermodel_queries
    );
    assert!(s.certified.unwrap().refusal.is_some());
    assert!(diag.contains("tight certificate refused"));
    let mut limited = options(Oracle::Auto, 1);
    limited.max_completion_scratch_bytes = 0;
    let (r, _, diag) = solve("1{a;b}1.", &limited);
    assert_eq!(r.completion, Completion::Exhausted);
    assert!(r.countermodel_statistics.unwrap().countermodel_queries > 0);
    assert!(diag.contains("tight certificate refused"));
    assert!(diag.contains("storage limit=0"));
}
struct Broken;
impl io::Write for Broken {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::new(
            io::ErrorKind::BrokenPipe,
            "test output failure",
        ))
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
#[test]
fn early_requested_models_and_writer_failure_keep_unpublished_certified_models_explicit() {
    let mut o = options(Oracle::Auto, 4);
    o.models = 1;
    let (r, _, _) = solve("1{a;b;c}1.", &o);
    assert_eq!(r.completion, Completion::RequestedModels);
    assert_eq!(r.models, 1);
    let e = r.formula_execution.unwrap();
    assert_eq!(e.queued_models, 2);
    assert_eq!(r.countermodel_statistics.unwrap().stable_models, 3);
    let failure: RunFailure = run_detailed_with_diagnostics(
        "1{a;b;c}1.".into(),
        &o,
        &mut Broken,
        &mut Vec::new(),
        &Control::default(),
    )
    .unwrap_err();
    let p = failure.partial_report.unwrap();
    assert_eq!(p.published_models, 0);
    assert_eq!(p.verified_models, 3);
    assert!(!p.summary_published);
    assert_eq!(
        p.countermodel_statistics.unwrap().certified.unwrap().stable,
        3
    );
    assert_eq!(p.formula_execution.unwrap().queued_models, 2);
}
