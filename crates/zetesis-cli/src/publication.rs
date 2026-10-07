//! Presentation consumes the public semantic session and its checked answers.

use std::io::Write;

use zetesis_cpu::Cancellation;
use zetesis_themelios::{OutputSelection, observation::ObservationProgram};

use crate::SolvePhase;
use crate::failure::Progress;
use crate::phase_timing::Recorder;
use crate::presentation::Diagnostics;
use crate::view::configuration::{Configuration, Observer};
use crate::{
    AnswerRenderer, PreparedInput, PublicationConfig, PublicationFailure, PublicationView, Session,
    SummaryDelivery, SummaryStage,
};

pub(crate) fn solve(
    input: PreparedInput<'_>,
    expansion: Option<zetesis_themelios::ExpansionUsage>,
    config: &PublicationConfig,
    renderer: &mut impl AnswerRenderer,
    diagnostics: &mut Diagnostics<impl Write>,
    cancellation: &Cancellation,
    phases: &Recorder,
) -> Result<Progress, PublicationFailure> {
    let selection = OutputSelection::default();
    let observations = ObservationProgram::default();
    let metadata = input.metadata();
    let mut display = crate::display::Display::new(
        metadata.map_or(&selection, zetesis_themelios::SourceMetadata::output),
        metadata.map_or(
            &observations,
            zetesis_themelios::SourceMetadata::observations,
        ),
        config.observations,
        cancellation,
    );
    let mut request =
        Session::builder(input, config.solve, cancellation.clone()).measurements(phases);
    if input
        .projection()
        .is_some_and(zetesis_themelios::PreparedProjection::is_explicit)
    {
        request = request.projected(zetesis_solve::ProjectionLimits {
            max_keys: usize::try_from(config.solve.max_projection_entries).unwrap_or(usize::MAX),
            max_bytes: config.solve.max_projection_bytes,
            max_work: config.solve.max_search_work,
        });
    }
    let mut configuration = Configuration::new(config.solve.workers);
    let mut session = request.start_observed(&mut Observer::new(
        renderer,
        diagnostics,
        &mut configuration,
        phases,
    ))?;
    let mut progress = Progress::new();
    progress.expansion = expansion;
    loop {
        let next = session.next_observed(&mut Observer::new(
            renderer,
            diagnostics,
            &mut configuration,
            phases,
        ));
        progress.apply(session.progress());
        match next {
            Some(Ok(answer)) => {
                let result = phases.measure(SolvePhase::ObservationOutput, || {
                    display.write(
                        renderer,
                        progress.publication.models + 1,
                        answer.interpretation(),
                        answer.score(),
                    )
                });
                match result {
                    Ok(std::ops::ControlFlow::Continue(())) => progress.publication.models += 1,
                    Ok(std::ops::ControlFlow::Break(stop)) => {
                        progress.stop = Some(stop);
                        break;
                    }
                    Err(error) => return Err(progress.fail(error)),
                }
            }
            Some(Err(error)) => return Err(progress.fail((*error.cause).into())),
            None => break,
        }
    }
    complete(renderer, diagnostics, progress, phases)
}

fn complete(
    renderer: &mut impl AnswerRenderer,
    diagnostics: &mut Diagnostics<impl Write>,
    mut progress: Progress,
    phases: &Recorder,
) -> Result<Progress, PublicationFailure> {
    if progress.stop.is_none()
        && let Err(cause) = progress.completion()
    {
        return Err(progress.fail(cause));
    }
    // Human summaries run before the later statistics snapshot. Supply the
    // available timing evidence now without claiming that output has finished.
    progress.phase_timings = phases.snapshot();
    let _output = phases.enter(SolvePhase::ObservationOutput);
    let result = (|| {
        if let Some(projection) = progress
            .semantic()
            .and_then(crate::SemanticOutcome::projection)
        {
            diagnostics.information(format_args!(
                "Projection: {} representatives, {} duplicate answers, complete={}",
                projection.representatives, projection.duplicates, projection.complete
            ))?;
        }
        if let Some(statistics) = progress
            .semantic()
            .and_then(crate::SemanticOutcome::countermodel_statistics)
        {
            diagnostics.information(format_args!(
                "Reduct search: {} work, {} decisions, {} classical candidates, {} reduct queries, {} countermodels",
                statistics.search.work,
                statistics.search.decisions,
                statistics.candidates,
                statistics.countermodel_queries,
                statistics.countermodels
            ))?;
        }
        if renderer.summary_stage() == SummaryStage::SearchFinished {
            acknowledge(renderer, &mut progress)?;
        }
        Ok(())
    })();
    match result {
        Ok(()) => Ok(progress),
        Err(error) => Err(progress.fail(error)),
    }
}

pub(crate) fn check_cancellation(
    renderer: &mut impl AnswerRenderer,
    diagnostics: &mut Diagnostics<impl Write>,
    cancellation: &Cancellation,
    phases: &Recorder,
) -> Result<Option<Progress>, PublicationFailure> {
    match cancellation.poll() {
        Ok(()) => Ok(None),
        Err(stop) => interrupted(stop, renderer, diagnostics, phases).map(Some),
    }
}

/// Final reporting never replaces an earlier failure with a reporting failure.
pub(crate) fn finalize(
    renderer: &mut impl AnswerRenderer,
    mut result: Result<Progress, PublicationFailure>,
) -> Result<crate::PublicationOutcome, PublicationFailure> {
    if renderer.summary_stage() == SummaryStage::Finalized {
        match renderer.finish(PublicationView {
            result: result.as_ref(),
        }) {
            Ok(SummaryDelivery::Accepted) => match &mut result {
                Ok(progress) => progress.publication.summary = true,
                Err(failure) => failure.acknowledge_summary(),
            },
            Ok(SummaryDelivery::Omitted) => {}
            Err(error) => {
                return Err(match result {
                    Ok(progress) => progress.fail(error),
                    Err(mut failure) => {
                        failure.record_summary(match error {
                            crate::RunError::Output(error) => error,
                            other => std::io::Error::other(other),
                        });
                        failure
                    }
                });
            }
        }
    }
    result.and_then(Progress::finalize)
}

fn acknowledge(
    renderer: &mut impl AnswerRenderer,
    progress: &mut Progress,
) -> Result<(), crate::RunError> {
    if renderer.finish(PublicationView {
        result: Ok(progress),
    })? == SummaryDelivery::Accepted
    {
        progress.publication.summary = true;
    }
    Ok(())
}

/// Publish an interruption established by a checked preparation operation.
/// Callers pass its typed stop, rather than polling after an unrelated failure;
/// this preserves the operation's diagnostic and failure precedence.
pub(crate) fn interrupted(
    reason: zetesis_cpu::Stop,
    renderer: &mut impl AnswerRenderer,
    diagnostics: &mut Diagnostics<impl Write>,
    phases: &Recorder,
) -> Result<Progress, PublicationFailure> {
    let mut progress = Progress::new();
    progress.apply(crate::SemanticOutcome::interrupted_before_start(
        crate::Interruption::Preparation(reason),
    ));
    complete(renderer, diagnostics, progress, phases)
}

#[cfg(test)]
mod tests;
