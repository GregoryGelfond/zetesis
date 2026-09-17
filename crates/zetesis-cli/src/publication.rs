//! Presentation consumes the public semantic session and its checked answers.

use std::io::Write;

use zetesis_cpu::Control;
use zetesis_themelios::{OutputSelection, observation::ObservationProgram};

use crate::SolvePhase;
use crate::failure::Progress;
use crate::phase_timing::Recorder;
use crate::presentation::Diagnostics;
use crate::{Options, PreparedInput, PublicationFailure, Session};

pub(crate) fn solve(
    input: PreparedInput<'_>,
    expansion: Option<zetesis_themelios::ExpansionUsage>,
    options: &Options,
    output: &mut impl Write,
    diagnostics: &mut Diagnostics<impl Write>,
    control: &Control,
    phases: &Recorder,
) -> Result<Progress, PublicationFailure> {
    let selection = OutputSelection::default();
    let observations = ObservationProgram::default();
    let metadata = input.metadata();
    let mut display = crate::display::Display {
        selection: metadata.map_or(&selection, zetesis_themelios::SourceMetadata::output),
        observations: metadata.map_or(
            &observations,
            zetesis_themelios::SourceMetadata::observations,
        ),
        options,
        control,
        atoms: zetesis_themelios::observation::json::AtomTable::new(options.max_atoms),
    };
    let mut request = Session::builder(input, options.into(), control.clone()).measurements(phases);
    if input
        .projection()
        .is_some_and(zetesis_themelios::PreparedProjection::is_explicit)
    {
        request = request.projected(zetesis_solve::ProjectionLimits::default());
    }
    let mut session = request.start_observed(diagnostics)?;
    let mut progress = Progress::new();
    progress.expansion = expansion;
    loop {
        let next = session.next_observed(diagnostics);
        progress.apply(session.progress());
        match next {
            Some(Ok(answer)) => {
                let result = phases.measure(SolvePhase::ObservationOutput, || {
                    display.write(
                        output,
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
    if progress.stop.is_none()
        && !options.json
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
    complete(output, diagnostics, progress, phases, options)
}

fn complete(
    output: &mut impl Write,
    diagnostics: &mut impl Write,
    mut progress: Progress,
    phases: &Recorder,
    options: &Options,
) -> Result<Progress, PublicationFailure> {
    if progress.stop.is_none()
        && let Err(cause) = progress.completion()
    {
        return Err(progress.fail(cause));
    }
    let _output = phases.enter(SolvePhase::ObservationOutput);
    let result = (|| {
        if let Some(projection) = progress
            .semantic()
            .and_then(crate::SemanticOutcome::projection)
        {
            writeln!(
                diagnostics,
                "Projection: {} representatives, {} duplicate answers, complete={}",
                projection.representatives, projection.duplicates, projection.complete
            )?;
        }
        if let Some(statistics) = progress
            .semantic()
            .and_then(crate::SemanticOutcome::countermodel_statistics)
        {
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
        crate::driver::finish(output, &progress, options.json, options.color)
    })();
    match result {
        Ok(()) => {
            progress.publication.summary = !options.json;
            Ok(progress)
        }
        Err(error) => Err(progress.fail(error)),
    }
}

pub(crate) fn check_control(
    output: &mut impl Write,
    diagnostics: &mut impl Write,
    control: &Control,
    phases: &Recorder,
    options: &Options,
) -> Result<Option<Progress>, PublicationFailure> {
    match control.poll() {
        Ok(()) => Ok(None),
        Err(stop) => {
            let mut progress = Progress::new();
            progress.apply(crate::SemanticOutcome::interrupted_before_start(
                crate::Interruption::Preparation(stop),
            ));
            complete(output, diagnostics, progress, phases, options).map(Some)
        }
    }
}

#[cfg(test)]
#[path = "../tests/support/batch_orchestration.rs"]
mod batch_publication_tests;

#[cfg(test)]
#[path = "../tests/support/partial_batch.rs"]
mod partial_publication_tests;

#[cfg(test)]
#[path = "../tests/support/prepared_control.rs"]
mod prepared_control_tests;

#[cfg(test)]
#[path = "../tests/support/publication_stops.rs"]
mod stop_tests;
