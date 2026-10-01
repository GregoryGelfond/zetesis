//! Bounded human records from the shared typed answer and terminal views.

use std::io::Write;

use zetesis_cpu::Cancellation;

use crate::display::record::{Contents, Record};
use crate::{
    AnswerRenderer, AnswerView, ColorMode, ConfigurationView, PublicationView, RunError,
    SearchState, SummaryDelivery, SummaryStage,
};

mod summary;

/// Streaming human renderer with an inclusive per-answer byte ceiling.
/// It buffers one complete record, never the complete answer-set family.
/// The terminal callback flushes the sink before acknowledging the summary,
/// so subsequent statistics cannot overtake buffered human output. A flush
/// failure preserves semantic evidence without acknowledging the summary;
/// successful flushing does not establish durable storage.
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
    fn needs_stage_timings(&self) -> bool {
        true
    }

    fn begin(&mut self) -> Result<(), RunError> {
        self.color.styled(
            &mut self.output,
            zetesis_presentation::Role::Metadata,
            format_args!("zetesis {}", crate::command::VERSION_INFORMATION),
        )?;
        writeln!(self.output)?;
        Ok(())
    }

    fn configuration(&mut self, view: ConfigurationView<'_>) -> Result<(), RunError> {
        summary::configuration(&mut self.output, self.color, view)?;
        writeln!(self.output)?;
        Ok(())
    }

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
        finish(&mut self.output, progress, self.color)?;
        if let Some(timings) = view.phase_timings() {
            writeln!(self.output)?;
            summary::timing(&mut self.output, self.color, &timings.stages)?;
        }
        self.output.flush()?;
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
        }
        SearchState::RequestedModels => {
            color.status(output, "SATISFIABLE")?;
        }
        SearchState::Interrupted(reason) => {
            verdict(output, color, format_args!("INCOMPLETE: {reason}"))?;
        }
        SearchState::PendingInterruption(_) => return Err(RunError::CompletionUnavailable),
    }
    let qualification = match state {
        SearchState::Exhausted => "",
        SearchState::RequestedModels => " (answer limit reached)",
        SearchState::Interrupted(_) => " (search incomplete)",
        SearchState::PendingInterruption(_) => return Err(RunError::CompletionUnavailable),
    };
    color.metadata(
        output,
        "Models",
        format_args!("{}{qualification}", progress.publication.models),
    )?;
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
