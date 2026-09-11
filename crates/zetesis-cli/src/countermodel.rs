//! Eager finite formula search with scalar CPU or batched hybrid membership.

use crate::ExecutionObservation as Event;
use crate::execution_observation::ExecutionSink;
use crate::presentation::Diagnostics;
use std::io::Write;

use zetesis_core::Atom;
use zetesis_cpu::Control;
use zetesis_ferraris::Theory;
use zetesis_themelios::OutputSelection;

use crate::driver::finish;
use crate::failure::Progress;
use crate::phase_timing::{Recorder, SolvePhase};
use crate::{Completion, Interruption, Options, RunError, SolveConfig, SolveFailure};

#[derive(Clone, Copy)]
pub(crate) struct Input<'a> {
    pub(crate) theory: &'a Theory,
    pub(crate) atoms: &'a [Atom],
    pub(crate) gate_atoms: usize,
    pub(crate) objectives: &'a zetesis_objective::ObjectiveProgram,
    pub(crate) observations: &'a zetesis_themelios::observation::ObservationProgram,
}

pub(crate) fn run_formula(
    input: Input<'_>,
    selection: &OutputSelection,
    options: &Options,
    output: &mut impl Write,
    diagnostics: &mut Diagnostics<impl Write>,
    control: &Control,
    phases: &Recorder,
) -> Result<Progress, SolveFailure> {
    let _solving = phases.stage(crate::SolveStage::Solving);
    if let Some(report) = check_control(
        output,
        diagnostics,
        control,
        phases,
        options.json,
        Some(crate::Subject::Theory(input.theory.clone())),
    )? {
        return Ok(report);
    }
    let display = crate::display::Display {
        selection,
        observations: input.observations,
        options,
        control,
    };
    let mut execution = phases.measure(SolvePhase::ExecutionSetup, || {
        crate::formula_execution::Execution::new(&options.into(), diagnostics)
    })?;
    FormulaRun { input, display }.solve(output, diagnostics, &mut execution, phases)
}

/// Original formula, observation projection and limits shared by every membership stream.
struct FormulaRun<'a> {
    input: Input<'a>,
    display: crate::display::Display<'a>,
}

impl FormulaRun<'_> {
    fn solve(
        self,
        output: &mut impl Write,
        diagnostics: &mut impl Write,
        execution: &mut impl crate::formula_execution::MembershipExecution,
        phases: &Recorder,
    ) -> Result<Progress, crate::SolveFailure> {
        let config = self.display.options.into();
        let mut observations = Diagnostics::new(&mut *diagnostics, self.display.options.color);
        let mut session = crate::formula_session::FormulaSession::new(
            self.input,
            execution,
            &config,
            &mut observations,
            self.display.control,
            phases,
        );
        let mut progress = Progress::new();
        loop {
            let next = session.next(&config, &mut observations, self.display.control, phases);
            progress.apply(session.outcome(phases));
            match next {
                Some(Ok((model, score))) => {
                    let result = phases.measure(SolvePhase::ObservationOutput, || {
                        self.display.write(
                            output,
                            progress.publication.models + 1,
                            &model,
                            score.as_ref(),
                        )
                    });
                    if let Err(error) = result {
                        return Err(progress.fail(error));
                    }
                    progress.publication.models += 1;
                }
                Some(Err(error)) => return Err(progress.fail(error)),
                None => break,
            }
        }
        if !self.display.options.json
            && let Some(best) = progress
                .semantic()
                .and_then(crate::SemanticOutcome::incumbent)
        {
            let result = phases.measure(SolvePhase::ObservationOutput, || {
                writeln!(
                    output,
                    "Incumbent ties: {}; stable models scored: {}; objective work: {}",
                    best.tied_models, best.scored_models, best.work
                )
            });
            if let Err(error) = result {
                return Err(progress.fail(error.into()));
            }
        }
        complete(
            output,
            diagnostics,
            progress,
            phases,
            self.display.options.json,
            self.display.options.color,
        )
    }
}

/// An optional setup interruption is retained by the same search report. A
/// diagnostic failure propagates separately and never erases the owned stream.
pub(crate) fn prepare_certificate(
    models: &mut zetesis_sat::StableModels,
    options: &SolveConfig,
    diagnostics: &mut impl ExecutionSink,
    phases: &Recorder,
) -> Result<Option<zetesis_sat::Incomplete>, RunError> {
    if options.oracle != crate::Oracle::Auto
        || !matches!(options.backend, crate::Backend::Auto | crate::Backend::Cpu)
    {
        return Ok(None);
    }
    let eligibility = phases.measure(SolvePhase::CertificateSetup, || {
        models.enable_certified_checking(zetesis_ferraris::TightPlanLimits {
            max_bytes: options.max_completion_scratch_bytes,
            ..Default::default()
        })
    });
    match eligibility {
        Ok(true) => diagnostics.record(Event::TightMembership)?,
        Ok(false) => diagnostics.record(Event::GeneralMembership(
            models
                .statistics()
                .certified
                .and_then(|s| s.refusal)
                .expect("refused certificate records its reason"),
        ))?,
        Err(error) => return Ok(Some(error)),
    }
    Ok(None)
}

pub(crate) fn check_control(
    output: &mut impl Write,
    diagnostics: &mut impl Write,
    control: &Control,
    phases: &Recorder,
    json: bool,
    subject: Option<crate::Subject>,
) -> Result<Option<Progress>, SolveFailure> {
    match control.poll() {
        Ok(()) => Ok(None),
        Err(error) => {
            let mut progress = Progress::new();
            progress.apply(crate::SemanticOutcome {
                subject,
                selection: None,
                verified: 0,
                scored: 0,
                retained: 0,
                completion: Some(Completion::Interrupted),
                interruption: Some(Interruption::Countermodel(error.into())),
                optimization: None,
                checked: 0,
                gate_atoms: 0,
                countermodel_statistics: None,
                formula_execution: None,
                lazy_execution: None,
                shared_execution: None,
            });
            complete(
                output,
                diagnostics,
                progress,
                phases,
                json,
                crate::ColorMode::Never,
            )
            .map(Some)
        }
    }
}

pub(crate) fn search_limits(options: &SolveConfig) -> zetesis_sat::Limits {
    zetesis_sat::Limits {
        search: zetesis_sat::SearchLimits {
            max_work: options.max_search_work,
            max_decisions: options.max_search_decisions,
        },
        max_candidates: options.max_candidates,
        max_verification_work: options.max_work,
        ..Default::default()
    }
}

fn complete(
    output: &mut impl Write,
    diagnostics: &mut impl Write,
    mut progress: Progress,
    phases: &Recorder,
    json: bool,
    color: crate::ColorMode,
) -> Result<Progress, SolveFailure> {
    let report = match progress.report() {
        Ok(report) => report,
        Err(cause) => return Err(progress.fail(cause)),
    };
    let _output = phases.start(SolvePhase::ObservationOutput);
    let result = (|| {
        if let Some(statistics) = report.countermodel_statistics {
            writeln!(
                diagnostics,
                "Reduct search: {} work, {} decisions, {} classical candidates, {} reduct queries, {} countermodels",
                statistics.search.work,
                statistics.search.decisions,
                statistics.candidates,
                statistics.countermodel_queries,
                statistics.countermodels
            )?;
        }
        finish(output, &report, json, color)
    })();
    match result {
        Ok(()) => {
            progress.publication.summary = !json;
            Ok(progress)
        }
        Err(error) => Err(progress.fail(error)),
    }
}

#[cfg(test)]
#[path = "../tests/support/batch_orchestration.rs"]
mod batch_contract_tests;

#[cfg(test)]
#[path = "../tests/support/partial_batch.rs"]
mod partial_batch_tests;

#[cfg(test)]
#[path = "../tests/support/prepared_control.rs"]
mod prepared_control_tests;
