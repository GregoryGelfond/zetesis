//! One borrowed native search, exported through the upstream fused-run protocol.
//!
//! The Solver owns the per-run admitted input. This run borrows that
//! owner through Session and metadata; it never stores an owner beside a borrow
//! into itself. Its constructor is private to the adapter: the entry point must
//! prepare with `FormulaPurpose::AnswerSets` and create unrestricted, models=0
//! enumeration. An arbitrary native Session does not establish those invariants.

use themelios_program::program::Program;
use themelios_solve::{
    contract::Fault,
    outcome::{Conclusion, Model, Run as SolveRun},
};
use zetesis_cpu::{Cancellation, CancellationRun};
use zetesis_solve::Session;
use zetesis_themelios::SourceMetadata;

use crate::{faults, model};

pub(crate) struct Run<'a> {
    // Retire before dropping the session and its worker resources, also when
    // the caller abandons a handle without pulling it to completion.
    window: Option<CancellationRun>,
    state: State<'a>,
}

struct Search<'a> {
    session: Session<'a>,
    original: &'a Program,
    metadata: &'a SourceMetadata,
    exporter: model::Exporter,
}

enum State<'a> {
    Open(Box<Search<'a>>),
    Pending(Conclusion),
    Concluded(Conclusion),
    Faulted,
}

impl<'a> Run<'a> {
    /// Transfer the window opened before preparation. The session must use a
    /// clone of this window's token; it cannot have independent request control.
    pub(crate) fn new(
        session: Session<'a>,
        original: &'a Program,
        metadata: &'a SourceMetadata,
        window: CancellationRun,
        limits: model::OutputLimits,
    ) -> Self {
        Self {
            window: Some(window),
            state: State::Open(Box::new(Search {
                session,
                original,
                metadata,
                exporter: model::Exporter::new(limits),
            })),
        }
    }

    /// Request control can end preparation before an admitted owner exists.
    /// Other native stops remain typed faults at their original failure boundary.
    /// The first pull acknowledges the end; conclusion is absent before it.
    pub(crate) fn pending(conclusion: Conclusion, window: CancellationRun) -> Self {
        Self {
            window: Some(window),
            state: State::Pending(conclusion),
        }
    }

    fn finish(&mut self, result: Result<Conclusion, Fault>) -> Option<Result<Model, Fault>> {
        drop(self.window.take());
        match result {
            Ok(conclusion) => {
                self.state = State::Concluded(conclusion);
                None
            }
            Err(fault) => {
                self.state = State::Faulted;
                Some(Err(fault))
            }
        }
    }
}

impl Search<'_> {
    fn completed(&self, cancellation: &Cancellation) -> Result<Conclusion, Fault> {
        match faults::completed(self.session.outcome().as_ref()) {
            Ok(Conclusion::Exhausted) => cancellation
                .poll()
                .map(|()| Conclusion::Exhausted)
                .or_else(|stop| model::OutputError::Control(stop).finish(self.original)),
            result => result,
        }
    }
}

impl SolveRun for Run<'_> {
    fn next_model(&mut self) -> Option<Result<Model, Fault>> {
        if let State::Pending(conclusion) = self.state {
            // Preparation stopped, but this live handle has not acknowledged
            // its end. Preserve the native cancellation-before-deadline order
            // for a pull made while the caller held the pending handle.
            let conclusion = self
                .window
                .as_ref()
                .expect("a pending adapter run owns its cancellation window")
                .cancellation()
                .poll()
                .err()
                .and_then(faults::control)
                .unwrap_or(conclusion);
            return self.finish(Ok(conclusion));
        }
        let State::Open(search) = &mut self.state else {
            return None;
        };
        let cancellation = self
            .window
            .as_ref()
            .expect("an open adapter run owns its cancellation window")
            .cancellation();
        // Preserve a concrete native fault, even if the control flag also fired.
        // A buffered successful answer is checked against control before export
        // and after upstream model construction, so it cannot bypass the token.
        // A native clean end must also observe request control: closure may have
        // buffered its last answer before the consumer paused between pulls.
        let result = match search.session.next() {
            Some(Ok(answer)) => {
                match search
                    .exporter
                    .convert(&answer, search.metadata, cancellation)
                {
                    Ok(model) => return Some(Ok(model)),
                    Err(error) => error.finish(search.original),
                }
            }
            Some(Err(error)) => faults::session(error, search.original),
            None => search.completed(cancellation),
        };
        self.finish(result)
    }

    fn conclusion(&self) -> Option<Conclusion> {
        match self.state {
            State::Concluded(conclusion) => Some(conclusion),
            State::Open(_) | State::Pending(_) | State::Faulted => None,
        }
    }
}

#[cfg(test)]
mod tests;
