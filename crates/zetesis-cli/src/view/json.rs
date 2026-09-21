//! One bounded JSON record at a time, with a bounded document atom table.

use std::io::Write;

use zetesis_cpu::Cancellation;
use zetesis_themelios::observation::json::AtomTable;

use crate::{AnswerRenderer, AnswerView, PublicationView, RunError, SummaryDelivery};

/// Streaming JSON renderer over the same typed views as the human renderer.
/// Retains one encoded record and a bounded table of previously spelled atoms,
/// not a complete `WorldView`. It does not poll control while emitting the final
/// failure/stop receipt, so cancellation cannot suppress that retained evidence.
pub struct JsonRenderer<W> {
    output: crate::output::Document<W>,
    atoms: AtomTable,
    max_record_bytes: usize,
    max_atoms: usize,
}

impl<W: Write> JsonRenderer<W> {
    /// Configure inclusive record and distinct document-atom ceilings.
    /// Construction performs no I/O; the controller starts the document.
    #[must_use]
    pub fn new(output: W, max_record_bytes: usize, max_atoms: usize) -> Self {
        Self::from_document(
            crate::output::Document::new(output),
            max_record_bytes,
            max_atoms,
        )
    }

    pub(crate) fn from_document(
        output: crate::output::Document<W>,
        max_record_bytes: usize,
        max_atoms: usize,
    ) -> Self {
        Self {
            output,
            atoms: AtomTable::new(max_atoms),
            max_record_bytes,
            max_atoms,
        }
    }

    /// Recover the sink after publication, including any accepted prefix.
    #[must_use]
    pub fn into_inner(self) -> W {
        self.output.into_inner()
    }
}

impl<W: Write> AnswerRenderer for JsonRenderer<W> {
    fn begin(&mut self) -> Result<(), RunError> {
        // A reused renderer begins a new document, not a continuation whose
        // atom indices could refer to a previous invocation's table.
        self.atoms = AtomTable::new(self.max_atoms);
        self.output.start();
        write!(
            self.output,
            "{{\"schema\":{},\"format\":\"zetesis\",\"models\":[",
            zetesis_themelios::observation::json::RECORD_SCHEMA_VERSION
        )?;
        Ok(())
    }

    fn answer(
        &mut self,
        view: AnswerView<'_>,
        cancellation: &Cancellation,
    ) -> Result<(), RunError> {
        crate::output::write_model_record(
            &mut self.output,
            view.number,
            view.model,
            &mut self.atoms,
            self.max_record_bytes,
            cancellation,
        )
    }

    fn finish(&mut self, view: PublicationView<'_>) -> Result<SummaryDelivery, RunError> {
        if self.output.failed() {
            return Ok(SummaryDelivery::Omitted);
        }
        let record = crate::output::summary(view, self.max_record_bytes)?;
        self.output.write_all(&record)?;
        Ok(SummaryDelivery::Accepted)
    }
}
