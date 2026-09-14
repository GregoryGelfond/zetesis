//! Actual formula sessions import independent cumulative timing prefixes once.

use super::{FormulaSession, Input};
use crate::execution_observation::Ignore;
use crate::formula_execution::Execution;
use crate::{
    AnswerSelection, Backend, ExecutionResources, Oracle, SolveConfig, SolveMeasurements,
    SolvePhase,
};
use zetesis_cpu::Control;
use zetesis_sat::{PhaseMeasurement, SearchPhaseTimings};
use zetesis_themelios::{AdmissionOptions, ExpansionLimits, FormulaLimits, admit_formula};

fn phases(timing: SearchPhaseTimings) -> [(SolvePhase, PhaseMeasurement); 5] {
    [
        (SolvePhase::CandidateGeneration, timing.candidates),
        (SolvePhase::OriginalValidation, timing.original_validation),
        (SolvePhase::ReductPreparation, timing.reduct_preparation),
        (SolvePhase::ExactReductMembership, timing.reduct),
        (SolvePhase::CertifiedMembership, timing.certified),
    ]
}

#[test]
fn interleaved_sessions_preserve_all_phase_attempts() {
    let admitted = admit_formula(
        "a | b.".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    let measurements = SolveMeasurements::new(true);
    measurements.measure(SolvePhase::CandidateGeneration, || ());
    let caller = measurements.snapshot().unwrap();
    let recorder = measurements.recorder();
    let config = SolveConfig {
        backend: Backend::Cpu,
        oracle: Oracle::Countermodel,
        models: 0,
        stats: true,
        ..SolveConfig::default()
    };
    let control = Control::default();
    let mut observations = Ignore;
    let input = Input {
        theory: admitted.theory(),
        atoms: admitted.atom_catalog(),
        objectives: admitted.objectives(),
        gate_atoms: 0,
        certificate_order: zetesis_sat::CertificateOrder::TightFirst,
    };
    let mut sessions = [(), ()].map(|()| {
        FormulaSession::with_selection(
            input,
            Execution::with_resources(&config, &ExecutionResources::default(), &mut observations)
                .unwrap(),
            &config,
            &mut observations,
            &control,
            recorder,
            AnswerSelection::All,
        )
    });
    let mut answers = [0; 2];
    while sessions.iter().any(|session| !session.finished()) {
        for (session, answers) in sessions.iter_mut().zip(&mut answers) {
            if let Some(answer) = session.next(&config, &mut observations, &control, recorder) {
                answer.unwrap();
                *answers += 1;
            }
            let before = measurements.snapshot().unwrap();
            session.outcome(recorder);
            let after = measurements.snapshot().unwrap();
            for (phase, _) in phases(SearchPhaseTimings::default()) {
                assert_eq!(before.get(phase), after.get(phase));
            }
            session.outcome(recorder);
            let repeated = measurements.snapshot().unwrap();
            for (phase, _) in phases(SearchPhaseTimings::default()) {
                assert_eq!(after.get(phase), repeated.get(phase));
            }
        }
    }
    assert_eq!(answers, [2, 2]);
    let [first, second] = sessions.map(|session| {
        session
            .outcome(recorder)
            .countermodel_statistics()
            .unwrap()
            .phase_timings
            .unwrap()
    });
    assert!(first.candidates.calls > 0);
    assert!(second.candidates.calls > 0);
    let accumulated = measurements.snapshot().unwrap();
    for ((phase, first), (_, second)) in phases(first).into_iter().zip(phases(second)) {
        let prefix = caller.get(phase).unwrap_or_default();
        let calls = prefix.calls + first.calls + second.calls;
        let expected = (calls != 0).then_some(PhaseMeasurement {
            calls,
            elapsed: prefix.elapsed + first.elapsed + second.elapsed,
            overflowed: false,
        });
        assert_eq!(accumulated.get(phase), expected);
    }
}
