//! A terminal count is published only from complete semantic evidence.

use std::io::Write;

use zetesis_cpu::Cancellation;

use crate::{AnswerRenderer, AnswerView, PublicationView, RunError, SummaryDelivery};

/// One decimal integer and a newline for a completely counted family.
/// Pair with [`crate::PublicationMode::Count`] to skip answer display and
/// projection. This renderer retains no models and prints no partial count.
/// Its record is bounded by a u64's 20 decimal digits plus the newline.
/// A writer or flush failure leaves the summary unacknowledged.
pub struct CountRenderer<W> {
    output: W,
}

impl<W: Write> CountRenderer<W> {
    /// Use an injected writer without terminal discovery or display limits.
    #[must_use]
    pub const fn new(output: W) -> Self {
        Self { output }
    }

    /// Recover the sink, including any prefix accepted before a writer failure.
    #[must_use]
    pub fn into_inner(self) -> W {
        self.output
    }
}

impl<W: Write> AnswerRenderer for CountRenderer<W> {
    fn answer(&mut self, _: AnswerView<'_>, _: &Cancellation) -> Result<(), RunError> {
        Ok(())
    }

    fn finish(&mut self, view: PublicationView<'_>) -> Result<SummaryDelivery, RunError> {
        if view.failure().is_some() || view.stop().is_some() {
            return Ok(SummaryDelivery::Omitted);
        }
        let Some(count) = view
            .semantic()
            .and_then(crate::SemanticOutcome::complete_answer_count)
        else {
            return Ok(SummaryDelivery::Omitted);
        };
        writeln!(self.output, "{count}")?;
        self.output.flush()?;
        Ok(SummaryDelivery::Accepted)
    }
}
