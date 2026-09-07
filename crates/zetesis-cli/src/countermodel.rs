//! Eager finite formula search with scalar CPU or batched hybrid membership.

use std::io::Write;

use zetesis_core::{Atom, Model};
use zetesis_cpu::Control;
use zetesis_ferraris::Theory;
use zetesis_themelios::OutputSelection;

use crate::driver::finish;
use crate::failure::Progress;
use crate::phase_timing::{Recorder, SolvePhase};
use crate::{Completion, Interruption, Options, Report, RunError, RunFailure};

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
    diagnostics: &mut impl Write,
    control: &Control,
    phases: &Recorder,
) -> Result<Progress, RunFailure> {
    let _solving = phases.stage(crate::SolveStage::Solving);
    if let Some(report) = check_control(output, diagnostics, control, phases, options.json)? {
        return Ok(report);
    }
    let display = crate::display::Display {
        selection,
        observations: input.observations,
        options,
        control,
    };
    let mut execution = phases.measure(SolvePhase::ExecutionSetup, || {
        crate::formula_execution::Execution::new(options, diagnostics)
    })?;
    FormulaRun { input, display }.solve(output, diagnostics, &mut execution, phases)
}

/// Original formula, observation projection and limits shared by every membership stream.
struct FormulaRun<'a> {
    input: Input<'a>,
    display: crate::display::Display<'a>,
}

impl FormulaRun<'_> {
    /// Search and scoring precede final publication. On unrelated failure the
    /// retained incumbent is evidence only: no new Answer or summary is emitted.
    fn solve(
        self,
        output: &mut impl Write,
        diagnostics: &mut impl Write,
        execution: &mut impl crate::formula_execution::MembershipExecution,
        phases: &Recorder,
    ) -> Result<Progress, RunFailure> {
        let mut progress = Progress::new(self.input.gate_atoms);
        let mut incumbents = crate::optimization::Incumbents::default();
        let result = self.search(
            output,
            diagnostics,
            execution,
            phases,
            &mut progress.report,
            &mut incumbents,
        );
        progress.verified_models = progress
            .report
            .countermodel_statistics
            .map_or(0, |stats| stats.stable_models);
        let completion = match result {
            Ok(completion) => completion,
            Err(error) => {
                progress.report.optimization = incumbents.into_optimization();
                return Err(progress.fail(error));
            }
        };
        progress.completion = Some(completion);
        progress.report.completion = completion;
        let written = phases.measure(SolvePhase::ObservationOutput, || {
            incumbents.write(output, &mut progress.report, &self.display)
        });
        progress.report.optimization = incumbents.into_optimization();
        if let Err(error) = written {
            return Err(progress.fail(error));
        }
        complete(
            output,
            diagnostics,
            progress,
            phases,
            self.display.options.json,
        )
    }

    /// Always snapshot the still-owned candidate stream after the fallible loop,
    /// including a failed restriction diagnostic or a pending checker batch.
    fn search(
        &self,
        output: &mut impl Write,
        diagnostics: &mut impl Write,
        execution: &mut impl crate::formula_execution::MembershipExecution,
        phases: &Recorder,
        report: &mut Report,
        incumbents: &mut crate::optimization::Incumbents,
    ) -> Result<Completion, RunError> {
        let input = self.input;
        let options = self.display.options;
        let control = self.display.control;
        writeln!(
            diagnostics,
            "Formula: {} atoms, {} nodes, {} roots",
            input.theory.atom_count(),
            input.theory.nodes().len(),
            input.theory.roots().len()
        )?;
        let mut models = match phases.measure(SolvePhase::CandidateSetup, || {
            zetesis_sat::StableModels::new(input.theory, search_limits(options), control.clone())
        }) {
            Ok(models) => models,
            Err(error) => {
                report.interruption = Some(Interruption::Countermodel(error));
                return Ok(Completion::Interrupted);
            }
        };
        if options.stats {
            models.enable_phase_timing();
        }
        let result = (|| {
            if let Some(error) = prepare_certificate(&mut models, options, diagnostics, phases)? {
                report.interruption = Some(Interruption::Countermodel(error));
                return Ok(Completion::Interrupted);
            }
            let mut bounds =
                if input.objectives.is_present() && options.max_objective_bound_work != 0 {
                    phases.measure(SolvePhase::ObjectiveFeedback, || {
                        crate::objective_bounds::Bounds::new(input, options, diagnostics, control)
                    })?
                } else {
                    crate::objective_bounds::Bounds::new(input, options, diagnostics, control)?
                };
            while let Some(result) = execution.next(&mut models, options, control, phases) {
                let interpretation = match result {
                    Ok(model) => model,
                    Err(crate::formula_execution::Failure::Search(error)) => {
                        report.interruption = Some(Interruption::Countermodel(error));
                        return Ok(Completion::Interrupted);
                    }
                    Err(crate::formula_execution::Failure::Run(error)) => return Err(error),
                };
                let model =
                    Model::new(interpretation.atoms().map(|atom| input.atoms[atom].clone()));
                if input.objectives.is_present() {
                    match phases.measure(SolvePhase::ObjectiveScoringRetention, || {
                        incumbents.consider(input.objectives, model, options, control)
                    }) {
                        Ok(true) => {
                            let _feedback = (options.max_objective_bound_work != 0)
                                .then(|| phases.start(SolvePhase::ObjectiveFeedback));
                            bounds.improve(
                                incumbents
                                    .score()
                                    .expect("retained improvement has a score"),
                                &mut models,
                                options,
                                diagnostics,
                                control,
                            )?;
                        }
                        Ok(false) => {}
                        Err(reason) => {
                            report.interruption = Some(reason);
                            return Ok(Completion::Interrupted);
                        }
                    }
                } else {
                    phases.measure(SolvePhase::ObservationOutput, || {
                        self.display.write(output, report.models + 1, &model, None)
                    })?;
                    report.models += 1;
                    if options.models != 0 && report.models >= options.models {
                        return Ok(Completion::RequestedModels);
                    }
                }
            }
            debug_assert!(models.exhausted());
            Ok(Completion::Exhausted)
        })();
        phases.search(models.statistics().phase_timings);
        report.checked = models.statistics().candidates;
        report.countermodel_statistics = Some(models.statistics());
        report.formula_execution = execution.statistics(&models);
        result
    }
}

/// An optional setup interruption is retained by the same search report. A
/// diagnostic failure propagates separately and never erases the owned stream.
fn prepare_certificate(
    models: &mut zetesis_sat::StableModels,
    options: &Options,
    diagnostics: &mut impl Write,
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
        Ok(true) => writeln!(
            diagnostics,
            "Membership: checked tight support certificate; exact reduct residual completion"
        )?,
        Ok(false) => writeln!(
            diagnostics,
            "Membership: general reduct; tight certificate refused: {}",
            models
                .statistics()
                .certified
                .and_then(|s| s.refusal)
                .expect("refused certificate records its reason")
        )?,
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
) -> Result<Option<Progress>, RunFailure> {
    match control.poll() {
        Ok(()) => Ok(None),
        Err(error) => {
            let mut progress = Progress::new(0);
            progress.completion = Some(Completion::Interrupted);
            progress.report.completion = Completion::Interrupted;
            progress.report.interruption = Some(Interruption::Countermodel(error.into()));
            complete(output, diagnostics, progress, phases, json).map(Some)
        }
    }
}

fn search_limits(options: &Options) -> zetesis_sat::Limits {
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
) -> Result<Progress, RunFailure> {
    let _output = phases.start(SolvePhase::ObservationOutput);
    let result = (|| {
        if let Some(statistics) = progress.report.countermodel_statistics {
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
        finish(output, &progress.report, json)
    })();
    match result {
        Ok(()) => {
            progress.summary_published = !json;
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
