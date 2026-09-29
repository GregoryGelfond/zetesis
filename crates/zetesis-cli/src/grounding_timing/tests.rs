//! Grounding snapshots preserve unavailable values and every failed output prefix.

use crate::{
    GroundingMeasurement, GroundingOutcome, GroundingPhase, GroundingWork, SolveMeasurements,
};
use zetesis_test_support::io::BoundedWriter;
use zetesis_themelios::GroundingObserver as _;

#[test]
fn every_attribution_prefix_preserves_writer_failure() {
    let measurements = SolveMeasurements::new(true);
    let observer = measurements.grounding_observer().unwrap();
    observer.enter();
    observer.phase_enter(GroundingPhase::RuleInstantiation, None);
    observer.phase_exit(
        GroundingPhase::RuleInstantiation,
        None,
        GroundingOutcome::Completed,
        GroundingWork::default(),
    );
    observer.exit();
    let timings = measurements.snapshot().unwrap().grounding;
    let mut expected = Vec::new();
    super::write(&mut expected, &timings).unwrap();
    let text = std::str::from_utf8(&expected).unwrap();
    assert!(text.contains("completed=1;"));
    assert!(text.contains("support_join_work=0;"));
    assert!(text.contains("support_head_work=0;"));
    assert_eq!(
        text.matches(": unmeasured\n").count(),
        GroundingPhase::ALL.len() - 1
    );
    for capacity in 0..expected.len() {
        let mut writer = BoundedWriter::new(capacity);
        assert_eq!(
            super::write(&mut writer, &timings).unwrap_err().kind(),
            std::io::ErrorKind::BrokenPipe
        );
        assert_eq!(writer.bytes(), &expected[..capacity]);
    }
    let mut complete = BoundedWriter::new(expected.len());
    super::write(&mut complete, &timings).unwrap();
    assert_eq!(complete.bytes(), expected);
}

#[test]
fn attribution_output_preserves_unavailable_values() {
    let mut measurement = GroundingMeasurement::default();
    measurement.work.join_rows = None;
    measurement.work.support_order_work = None;
    measurement.work.support_join_work = None;
    measurement.work.support_head_work = None;
    measurement.elapsed = None;
    let mut expected = Vec::new();
    super::write_measurement(&mut expected, &measurement).unwrap();
    let text = std::str::from_utf8(&expected).unwrap();
    assert!(text.contains("elapsed_ns=unavailable;"));
    assert!(text.contains("join_rows=unavailable;"));
    assert!(text.contains("support_order_work=unavailable;"));
    assert!(text.contains("support_join_work=unavailable;"));
    assert!(text.contains("support_head_work=unavailable;"));
    for capacity in 0..expected.len() {
        let mut writer = BoundedWriter::new(capacity);
        assert_eq!(
            super::write_measurement(&mut writer, &measurement)
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::BrokenPipe
        );
        assert_eq!(writer.bytes(), &expected[..capacity]);
    }
}

#[test]
fn real_domain_work_reaches_the_shared_output_view() {
    use std::fmt::Write as _;
    use zetesis_themelios::{
        AdmissionOptions, DomainLimits, ExpansionLimits, FormulaLimits, prepare_formula,
    };

    let mut source = String::new();
    for value in 1..=4 {
        write!(source, "a({value}).b({value}).").unwrap();
    }
    for left in 3..=6 {
        for right in 3..=6 {
            write!(source, "c({left},{right}).").unwrap();
        }
    }
    source.push_str("r(X,Y):-a(X),b(Y),c(X,Y).");
    let measurements = SolveMeasurements::new(true);
    let observer = measurements.grounding_observer().unwrap();
    let admitted = prepare_formula(
        source,
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap()
    .with_domain_analysis(Some(DomainLimits::default()))
    .ground_with_observer(Some(&observer))
    .unwrap();
    assert_eq!(
        admitted
            .atoms()
            .iter()
            .filter(|atom| atom.predicate().name() == "r")
            .count(),
        4
    );
    let timings = measurements.snapshot().unwrap().grounding;
    let analysis = timings.get(GroundingPhase::DomainAnalysis).unwrap();
    assert_eq!(analysis.count(GroundingOutcome::Completed), Some(1));
    assert!(analysis.work.domain_prepare_work.unwrap() > 0);
    let rules = timings.get(GroundingPhase::RuleInstantiation).unwrap();
    // Default Indexed matching exposes one shortest c posting. The guards
    // reject two a rows, four b rows, and eight of the sixteen c posting rows.
    assert_eq!(rules.work.domain_rejected_rows, Some(14));
    assert!(rules.work.indexed_probes.unwrap() > 0);
    assert!(rules.work.domain_guard_checks.unwrap() > 0);
    let fields = super::work_fields(&rules.work);
    assert!(fields.contains(&("domain_rejected_rows", Some(14))));
    assert!(fields.contains(&("domain_guard_rows", rules.work.domain_guard_rows)));
    assert!(fields.contains(&("domain_guard_checks", rules.work.domain_guard_checks)));
    assert!(
        super::work_fields(&analysis.work)
            .contains(&("domain_prepare_work", analysis.work.domain_prepare_work))
    );
    // This is the same field view used by output.rs's JSON path. The producer is
    // real library preparation; ordinary command defaults are deliberately off.
    let mut output = Vec::new();
    super::write(&mut output, &timings).unwrap();
    let output = std::str::from_utf8(&output).unwrap();
    assert!(output.contains("grounding domain_analysis:"));
    assert!(output.contains("domain_rejected_rows=14;"));
}
