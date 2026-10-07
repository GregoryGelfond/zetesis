//! Replay already checked record prefixes to isolate terminal admission/faults.
//! This fixture retains the real tracked writer and invokes the public renderer
//! through the production finalization controller; it does not classify outcomes.

use crate::failure::Progress;
use crate::{PublicationFailure, PublicationOutcome, PublicationView, RunError};
use std::io::{self, Write};

pub(crate) struct Document<W> {
    output: super::Document<W>,
}

impl<W: Write> Document<W> {
    pub(crate) fn new(output: W, json: bool) -> Result<Self, PublicationFailure> {
        assert!(json, "fixture replays JSON documents only");
        let mut output = super::Document::new(output);
        write!(
            output,
            "{{\"schema\":{},\"format\":\"zetesis\",\"models\":[",
            zetesis_themelios::observation::json::RECORD_SCHEMA_VERSION
        )?;
        Ok(Self { output })
    }

    pub(crate) fn finish(
        self,
        result: Result<Progress, PublicationFailure>,
        record_bytes: usize,
    ) -> Result<PublicationOutcome, PublicationFailure> {
        let mut renderer =
            crate::JsonRenderer::from_document(self.output, record_bytes, usize::MAX);
        crate::publication::finalize(&mut renderer, result)
    }
}

impl<W: Write> Write for Document<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.output.write(bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.output.flush()
    }
}

pub(crate) fn summary(
    result: &Result<Progress, PublicationFailure>,
    maximum: usize,
) -> Result<Vec<u8>, RunError> {
    super::summary(
        PublicationView {
            result: result.as_ref(),
        },
        maximum,
    )
}
