//! Invocation-local recorded formula checks and the original raised program.

use std::sync::Arc;

use themelios_program::program::Program;
use themelios_program::raise::{raise as raise_program, raise_occurrences};
use themelios_syntax::ast;
use themelios_syntax::parse::Parse;

use crate::{
    AdmissionFailure, AdmissionOptions, ExpansionFailure, FormulaFailure, MetadataStorageLimits,
    metadata, profile,
};

/// Recording never changes the extended attempt's verdict. Formula metadata
/// failures are deferred to their original source position in formula admission.
pub(crate) struct Recorder {
    profiles: Vec<Result<usize, AdmissionFailure>>,
    formula_profiles_passed: bool,
    metadata: metadata::Builder,
    metadata_failure: Option<(usize, FormulaFailure)>,
    program: Option<Arc<Program>>,
}

impl Recorder {
    pub(crate) fn new(limits: MetadataStorageLimits) -> Self {
        Self {
            profiles: Vec::new(),
            formula_profiles_passed: true,
            metadata: metadata::Builder::new(limits),
            metadata_failure: None,
            program: None,
        }
    }

    pub(crate) fn check_profile(
        &mut self,
        parsed: &Parse<ast::Program>,
        options: AdmissionOptions,
        includes: bool,
    ) -> Result<usize, AdmissionFailure> {
        let [extended, formula] = profile::check_automatic(parsed, options, includes);
        #[cfg(test)]
        let formula = if super::testing::rejects_recorded_profile() {
            profile::check_formula(
                parsed,
                AdmissionOptions {
                    max_syntax_nodes: 0,
                    ..options
                },
                includes,
            )
        } else {
            formula
        };
        self.formula_profiles_passed &= formula.is_ok();
        self.profiles.push(formula);
        extended
    }

    /// Called only after every source check, raise and extended metadata
    /// collection succeeded, and the final relational compilation refused.
    pub(crate) fn retain_program(&mut self, program: Program) {
        self.program = Some(Arc::new(program));
    }

    pub(crate) fn into_handoff(self) -> Option<Handoff> {
        let program = self.program?;
        // Guard against future formula-only profile restrictions: a failed
        // recorded verdict sends the original owner through ordinary preparation.
        let nodes = self
            .profiles
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .ok()?;
        Some(Handoff {
            metadata: self.metadata,
            replay: Replay {
                program,
                nodes,
                metadata_failure: self.metadata_failure,
            },
        })
    }
}

/// Raise through the same occurrence stream as formula admission only while
/// recording. The retained extended APIs keep their ordinary raising path.
pub(crate) fn raise(
    parsed: &Parse<ast::Program>,
    recorder: Option<&mut Recorder>,
) -> Result<Program, ExpansionFailure> {
    let Some(recorder) = recorder else {
        let raised = raise_program(parsed);
        if !raised.diagnostics().is_empty() {
            return Err(AdmissionFailure::Raise(raised.diagnostics().to_vec()).into());
        }
        return Ok(raised.into_program());
    };
    let occurrences = raise_occurrences(parsed);
    #[cfg(test)]
    super::testing::note_raise();
    if !occurrences.diagnostics().is_empty() {
        return Err(AdmissionFailure::Raise(occurrences.diagnostics().to_vec()).into());
    }
    if recorder.metadata_failure.is_none()
        && recorder.formula_profiles_passed
        && let Err(error) = metadata::collect_occurrences(&occurrences, &mut recorder.metadata)
    {
        recorder.metadata_failure = Some((recorder.profiles.len() - 1, error.into()));
    }
    Ok(occurrences.into_raised().into_program())
}

/// A handoff can only be constructed by a complete, guarded recording under
/// the same invocation's source, expansion and formula limits.
pub(crate) struct Handoff {
    metadata: metadata::Builder,
    replay: Replay,
}

pub(crate) struct Replay {
    pub(crate) program: Arc<Program>,
    pub(crate) nodes: Vec<usize>,
    metadata_failure: Option<(usize, FormulaFailure)>,
}

impl Replay {
    /// The only saved refusal is read at the original raise/metadata position.
    pub(crate) fn metadata_at(&mut self, index: usize) -> Result<(), FormulaFailure> {
        if self
            .metadata_failure
            .as_ref()
            .is_some_and(|(source, _)| *source == index)
        {
            return Err(self
                .metadata_failure
                .take()
                .expect("checked source index")
                .1);
        }
        Ok(())
    }
}

pub(crate) fn split(
    handoff: Option<Handoff>,
    limits: MetadataStorageLimits,
) -> (metadata::Builder, Option<Replay>) {
    match handoff {
        Some(handoff) => {
            #[cfg(test)]
            super::testing::note_replay();
            (handoff.metadata, Some(handoff.replay))
        }
        None => (metadata::Builder::new(limits), None),
    }
}
