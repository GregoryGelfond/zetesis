//! Bounded original-source formula-admission profiling.
//!
//! This serial experiment compares fresh native admissions under three observer
//! modes. It does not measure whole-process solving, relational eager/lazy
//! grounding, optimization, or a GPU. Exact subject comparison and complete
//! native stable-model enumeration qualify each sample outside its admission
//! timer. Native defaults are retained by the command adapter; library callers
//! may supply explicit alternative ceilings in [`Configuration`].

use std::{io::Write, path::Path, time::Instant};

use serde::Serialize;
use zetesis_themelios::{
    AdmittedFormulaBundle, GroundingObserver, GroundingOptions, JoinStrategy, SourceBundle,
    prepare_bundle_formula,
};

mod config;
mod error;
mod fingerprint;
mod observer;
mod report;
mod semantic;
mod storage;

pub use config::{CaptureLimits, Configuration, Joins, Options};
pub use error::Error;
pub use fingerprint::{FingerprintUnavailable, SubjectFingerprint};
pub use observer::{CaptureRefusal, PhaseRecord, SourceSpan};
pub use report::{Report, Sample, SourceIdentity, write_report};
pub use semantic::Models;

/// Instrumentation condition, without changing native semantics or limits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// No frontend observer and no actual-grounding clock.
    Unobserved,
    /// Existing actual-grounding enter/exit callbacks only.
    Boundary,
    /// Actual-grounding boundary plus bounded phase clocks and selected counters.
    Detailed,
}

impl Mode {
    /// First-round condition order; successive rounds rotate left once.
    pub const ALL: [Self; 3] = [Self::Unobserved, Self::Boundary, Self::Detailed];
}

/// Profile an original source file and its native include graph.
///
/// One unmeasured indexed-join admission and complete native solve establish the reference.
/// Each round then loads/parses the original graph again, verifies exact bytes
/// and include identities, preallocates observer records, and times admission.
/// A failed timed attempt retains its duration and phase prefix. Objective
/// declarations are refused explicitly, including empty or inactive objectives.
///
/// Cost includes one reference and `3 * repetitions` native admissions/solves.
/// Each uses its independent native ceilings; no end-to-end deadline is imposed.
/// At most two admitted subjects are live together. Retained samples occupy
/// O(repetitions * (phase records + models + model atom indices)) cells, plus
/// bounded source, atom text and native subject storage. Model copying scans the
/// native carrier twice per accepted model; multiset sorting adds comparison
/// work outside timing. Execution-subject fingerprinting adds a borrowed scan
/// bounded by `capture.max_subject_bytes`, with fixed-size hashing scratch and
/// no retained byte encoding. Captured durations include observer overhead.
///
/// # Errors
/// Invalid configuration or initial report allocation returns `Err`. Later
/// loading, admission, capture, model or identity failures are retained in
/// [`Report::failure`] with `complete == false`, without discarding the prefix.
pub fn profile(path: impl AsRef<Path>, configuration: Configuration) -> Result<Report, Error> {
    let configuration = configuration.validate()?;
    let mut report = Report::new(&configuration)?;
    let outcome = qualify(path.as_ref(), &mut report);
    report.complete = outcome.is_ok();
    report.failure = outcome.err();
    Ok(report)
}

fn qualify(path: &Path, report: &mut Report) -> Result<(), Error> {
    let config = report.configuration;
    let bundle = SourceBundle::load(path, config.bundle).map_err(Error::Source)?;
    report.sources = report::catalog(&bundle, config.capture.max_source_path_bytes)?;
    let reference = admit(
        bundle,
        &Configuration {
            grounding: GroundingOptions {
                joins: JoinStrategy::Indexed,
            },
            ..config
        },
        None,
    )?;
    semantic::objective_free(&reference)?;
    report.atoms = semantic::catalog(&reference, config.capture.max_atom_text_bytes)?;
    report.nodes = Some(reference.theory().nodes().len());
    report.roots = Some(reference.theory().roots().len());
    report.subject_fingerprint = Some(fingerprint::subject(
        &reference,
        config.capture.max_subject_bytes,
    )?);
    let (models, failure) = semantic::enumerate(&reference, &config);
    report.qualification = Some(models);
    if let Some(error) = failure {
        return Err(error);
    }
    // The bounded round and mode indices strictly advance. The reference stays
    // live throughout, making retained native memory comparable across modes.
    for round in 0..config.repetitions {
        for position in 0..Mode::ALL.len() {
            let mode = Mode::ALL[(round + position) % Mode::ALL.len()];
            sample(path, &reference, report, round, mode)?;
        }
    }
    Ok(())
}

fn admit(
    bundle: SourceBundle,
    config: &Configuration,
    observer: Option<&dyn GroundingObserver>,
) -> Result<AdmittedFormulaBundle, Error> {
    let preparation =
        prepare_bundle_formula(bundle, config.admission, config.expansion, config.formula)
            .map_err(|error| Error::Admission(Box::new(error)))?;
    preparation
        .with_grounding_options(config.grounding)
        .ground_with_observer(observer)
        .map_err(|error| Error::Admission(Box::new(error)))
}

fn sample(
    path: &Path,
    reference: &AdmittedFormulaBundle,
    report: &mut Report,
    round: usize,
    mode: Mode,
) -> Result<(), Error> {
    let config = report.configuration;
    let bundle = SourceBundle::load(path, config.bundle).map_err(Error::Source)?;
    if !report::same_sources(reference.bundle(), &bundle) {
        return Err(Error::SourceChanged);
    }
    let (mut sample, admitted, refusal) = measure(bundle, &config, round, mode)?;
    // Keep the measured record while checking later conditions, then append it
    // even when one of those checks fails.
    let result = (|| {
        let subject = admitted?;
        if let Some(error) = refusal {
            return Err(error);
        }
        let equal = semantic::same_subject(reference, &subject);
        sample.subject_equal = Some(equal);
        if !equal {
            return Err(Error::SubjectChanged);
        }
        sample.subject_fingerprint = Some(fingerprint::subject(
            &subject,
            config.capture.max_subject_bytes,
        )?);
        let (models, failure) = semantic::enumerate(&subject, &config);
        let equal = report
            .qualification
            .as_ref()
            .is_some_and(|reference| models.interpretations == reference.interpretations);
        sample.models = Some(models);
        if let Some(error) = failure {
            return Err(error);
        }
        if !equal {
            return Err(Error::ModelsChanged);
        }
        Ok(())
    })();
    report.samples.push(sample);
    result
}

type Measurement = (Sample, Result<AdmittedFormulaBundle, Error>, Option<Error>);

fn measure(
    bundle: SourceBundle,
    config: &Configuration,
    round: usize,
    mode: Mode,
) -> Result<Measurement, Error> {
    let observer = observer::Observer::new(mode, config.capture.max_phase_records)?;
    let selected = (mode != Mode::Unobserved).then_some(&observer as &dyn GroundingObserver);
    let start = Instant::now();
    let admitted = admit(bundle, config, selected);
    let admission_elapsed_ns = u64::try_from(start.elapsed().as_nanos()).ok();
    let (grounding_elapsed_ns, phases, capture) = observer.finish();
    let refusal = capture.map(Error::Capture).or_else(|| {
        admission_elapsed_ns
            .is_none()
            .then_some(Error::ClockOverflow)
    });
    let sample = Sample {
        round,
        mode,
        admission_elapsed_ns,
        grounding_elapsed_ns,
        phases,
        capture_refusal: capture,
        admitted: admitted.is_ok(),
        subject_equal: None,
        subject_fingerprint: None,
        models: None,
    };
    Ok((sample, admitted, refusal))
}

/// Execute the command adapter and publish its one bounded JSON record.
///
/// # Errors
/// Returns operational/publication failure, or the report's first qualification
/// refusal after publishing that incomplete report. Only a completed report with
/// a successful full write returns `Ok(())`.
pub fn run(options: &Options, output: &mut impl Write) -> Result<(), Error> {
    let mut report = profile(&options.source, options.configuration())?;
    write_report(&report, output)?;
    if let Some(error) = report.failure.take() {
        return Err(error);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
