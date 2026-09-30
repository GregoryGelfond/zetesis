//! Helpers the crate's unit tests share across its modules.

use std::cell::Cell;

use themelios_base::source::SourceId;
use themelios_base::span::{ByteOffset, Location, Span};

use crate::{GroundingObserver, GroundingOutcome, GroundingPhase, GroundingWork};

/// The empty span at the start of source 0.
pub(crate) fn location() -> Location {
    Location {
        source: SourceId::new(0),
        span: Span::empty(ByteOffset::new(0)),
    }
}

/// A grounding observer that keeps the work of the last phase to exit.
#[derive(Default)]
pub(crate) struct Observer(pub(crate) Cell<GroundingWork>);

impl GroundingObserver for Observer {
    fn enter(&self) {}
    fn exit(&self) {}
    fn details_enabled(&self) -> bool {
        true
    }
    fn phase_exit(
        &self,
        _: GroundingPhase,
        _: Option<Location>,
        _: GroundingOutcome,
        work: GroundingWork,
    ) {
        self.0.set(work);
    }
}
