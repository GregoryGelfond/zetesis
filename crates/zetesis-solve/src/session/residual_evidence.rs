//! Optional qualification evidence for the general GPU route.

use super::Session;
use std::{fs::File, io};
use zetesis_backend::residual_evidence::Header;

impl Session<'_> {
    /// Offer one empty staging file for this run's original GPU residual-world and
    /// committed-batch sample records. The run writes the body at settlement.
    /// Call [`Self::finish_residual_evidence`] to seal a successful capture.
    ///
    /// File and I/O ownership are separate from the solve result and its work
    /// budgets. The staging file must be opened for overwriting and held for
    /// exclusive writing through sealing. Append mode is refused at settlement.
    /// No file names or callbacks enter the solver. Dropping a session before
    /// sealing can leave an incomplete staging file, which its caller owns.
    ///
    /// # Errors
    /// Returns a diagnostic error if the selected execution has no collector,
    /// the run has already settled or received an offer, the file is nonempty,
    /// or the requested ceiling exceeds the wire format's maximum.
    pub fn request_residual_evidence(
        &mut self,
        file: File,
        nonce: [u8; 16],
        max_bytes: u64,
    ) -> io::Result<()> {
        #[cfg(feature = "gpu")]
        if let Some(samples) = self.run_samples_mut() {
            return samples.offer(file, nonce, max_bytes);
        }
        #[cfg(not(feature = "gpu"))]
        let _ = (file, nonce, max_bytes);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "no general GPU residual sample collector",
        ))
    }

    /// Seal a settled capture and transfer its receipt once. Before settlement,
    /// or without an outstanding capture, there is no receipt. The file header's
    /// completeness is taken from the session's whole semantic outcome.
    ///
    /// # Errors
    /// A refused, short or failed write yields the export's error independently
    /// of the solver verdict. The caller publishes only a successful staging file.
    pub fn finish_residual_evidence(&mut self) -> Option<io::Result<Header>> {
        #[cfg(feature = "gpu")]
        {
            let captured = self.run_samples_mut()?.take_export()?;
            let complete =
                self.current_evidence().completion() == Some(crate::Completion::Exhausted);
            Some(captured.and_then(|written| written.seal(complete)))
        }
        #[cfg(not(feature = "gpu"))]
        None
    }

    #[cfg(feature = "gpu")]
    fn run_samples_mut(&mut self) -> Option<&mut crate::residual_handovers::sampling::RunSamples> {
        use super::State;
        match &mut self.state {
            State::Formula(formula) => formula.run_samples_mut(),
            State::Hybrid(hybrid) => hybrid.formula_core_mut().run_samples_mut(),
            State::TerminalDefinitions(terminal) => terminal.formula_core_mut().run_samples_mut(),
            State::Source(_) | State::Stopped(_) => None,
        }
    }
}
