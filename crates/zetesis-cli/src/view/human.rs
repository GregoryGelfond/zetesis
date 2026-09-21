//! Bounded human records from the shared typed answer and terminal views.

use std::io::Write;

use zetesis_cpu::Cancellation;

use crate::display::record::{Contents, Record};
use crate::{
    AnswerRenderer, AnswerView, ColorMode, Interruption, PublicationView, RunError, SearchState,
    SummaryDelivery, SummaryStage,
};

/// Streaming human renderer with an inclusive per-answer byte ceiling.
/// It buffers one complete record, never the complete answer-set family.
pub struct HumanRenderer<W> {
    output: W,
    color: ColorMode,
    max_record_bytes: usize,
}

impl<W: Write> HumanRenderer<W> {
    /// Use an injected writer and explicit styling; no terminal discovery occurs.
    #[must_use]
    pub const fn new(output: W, color: ColorMode, max_record_bytes: usize) -> Self {
        Self {
            output,
            color,
            max_record_bytes,
        }
    }

    /// Recover the sink after publication, including any accepted prefix.
    #[must_use]
    pub fn into_inner(self) -> W {
        self.output
    }
}

impl<W: Write> AnswerRenderer for HumanRenderer<W> {
    fn summary_stage(&self) -> SummaryStage {
        SummaryStage::SearchFinished
    }

    fn answer(
        &mut self,
        view: AnswerView<'_>,
        cancellation: &Cancellation,
    ) -> Result<(), RunError> {
        let rendered;
        let contents = if view.observations {
            rendered = view
                .model
                .render(view.limits, cancellation)
                .map_err(RunError::Observation)?;
            Contents::Observed(rendered.text())
        } else {
            // The empty observation program still has the same typed model view;
            // its legacy atom-only spelling charges record bytes, not term work.
            Contents::Atoms(view.model)
        };
        let record = Record::prepare(
            view.number,
            contents,
            view.model.score(),
            self.color,
            self.max_record_bytes,
            cancellation,
        )?;
        cancellation.poll().map_err(RunError::PublicationStopped)?;
        self.output.write_all(record.bytes())?;
        Ok(())
    }

    fn finish(&mut self, view: PublicationView<'_>) -> Result<SummaryDelivery, RunError> {
        let Ok(progress) = view.result else {
            return Ok(SummaryDelivery::Omitted);
        };
        if progress.stop.is_none()
            && let Some(best) = view.semantic().and_then(crate::SemanticOutcome::incumbent)
        {
            self.color.metadata(
                &mut self.output,
                "Incumbent ties",
                format_args!(
                    "{}; stable models scored: {}; objective work: {}",
                    best.tied_models, best.scored_models, best.work
                ),
            )?;
        }
        finish(&mut self.output, progress, self.color)?;
        Ok(SummaryDelivery::Accepted)
    }
}

pub(crate) fn finish(
    output: &mut impl Write,
    progress: &crate::failure::Progress,
    color: crate::ColorMode,
) -> Result<(), RunError> {
    let semantic = progress.semantic().ok_or(RunError::CompletionUnavailable)?;
    if let Some(stop) = &progress.stop {
        verdict(output, color, format_args!("INCOMPLETE: {stop}"))?;
        color.metadata(
            output,
            "Publication",
            format_args!(
                "incomplete; complete model records: {}",
                progress.publication.models
            ),
        )?;
        color.metadata(
            output,
            "Search coverage",
            format_args!("{:?}", semantic.completion()),
        )?;
        if semantic.optimum_proved() {
            color.status(output, "Optimum proved; delivery incomplete")?;
        }
        return Ok(());
    }
    let state = semantic
        .search_state()
        .ok_or(RunError::CompletionUnavailable)?;
    state.completion().ok_or(RunError::CompletionUnavailable)?;
    match state {
        SearchState::Exhausted => {
            if semantic.unsatisfiable() {
                color.status(output, "UNSATISFIABLE")?;
            } else if semantic.optimum_proved() {
                color.status(output, "OPTIMUM FOUND")?;
            } else {
                color.status(output, "SATISFIABLE")?;
            }
            color.metadata(output, "Coverage", format_args!("exhausted"))?;
        }
        SearchState::RequestedModels => {
            color.status(output, "SATISFIABLE")?;
            color.metadata(
                output,
                "Coverage",
                format_args!("partial (requested model count reached)"),
            )?;
        }
        SearchState::Interrupted(reason) => {
            verdict(output, color, format_args!("INCOMPLETE: {reason}"))?;
            color.metadata(output, "Coverage", format_args!("partial"))?;
        }
        SearchState::PendingInterruption(_) => return Err(RunError::CompletionUnavailable),
    }
    if semantic.countermodel_statistics().is_some()
        || matches!(semantic.interruption(), Some(Interruption::Countermodel(_)))
    {
        color.metadata(
            output,
            "Models",
            format_args!(
                "{}; candidates examined: {}; gate tuples discovered: n/a (formula search)",
                progress.publication.models,
                semantic.candidate_progress()
            ),
        )?;
    } else {
        let examined = if semantic.shared_execution().is_some() {
            "closure result/control records examined"
        } else {
            "candidates examined"
        };
        color.metadata(
            output,
            "Models",
            format_args!(
                "{}; {examined}: {}; gate tuples discovered: {}",
                progress.publication.models,
                semantic.candidate_progress(),
                semantic.discovered_gate_atoms()
            ),
        )?;
    }
    Ok(())
}

fn verdict(
    output: &mut impl Write,
    color: ColorMode,
    text: std::fmt::Arguments<'_>,
) -> std::io::Result<()> {
    color.styled(output, zetesis_presentation::Role::Conclusion, text)?;
    writeln!(output)
}
