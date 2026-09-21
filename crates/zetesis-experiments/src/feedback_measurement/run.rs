use super::{
    Case, Configuration, ConstructionLimits, Error, Event, Progress, Route, Sample, StageTimes,
    fixtures,
    guard::{self, Budget},
    replay,
};
use std::io;
use zetesis_cpu::Cancellation;
use zetesis_ferraris::Theory;

/// Run complete refinement checks and the finite whole-owner observations.
///
/// # Errors
/// Preserves the first failed route prefix and the original typed cause. Prior
/// published samples remain complete; an error never emits a completion event.
pub fn measure(
    configuration: &Configuration,
    observe: impl FnMut(&Event<'_>) -> io::Result<()>,
) -> Result<(), Error> {
    measure_with_cancellation(configuration, &Cancellation::default(), observe)
}

/// Same finite study under caller-owned cancellation and an absolute deadline.
/// Native operations retain their existing polling; no background control task.
///
/// # Errors
/// Returns control, admission, construction, native, parity or publication failure.
pub fn measure_with_cancellation(
    configuration: &Configuration,
    cancellation: &Cancellation,
    mut observe: impl FnMut(&Event<'_>) -> io::Result<()>,
) -> Result<(), Error> {
    configuration.validate()?;
    observe(&Event::Start {
        schema: 1,
        configuration,
        native_limits: configuration.native().into(),
        qualification_construction: ConstructionLimits::default(),
        reference_subsets: super::reference_limits(configuration.max_reference_work).max_subsets,
    })
    .map_err(Error::Output)?;
    let mut samples = 0;
    for case in Case::ALL {
        cancellation.poll().map_err(Error::Control)?;
        let source = case.theory()?;
        let (expected, pairs) = qualify(&source, configuration.max_reference_work, cancellation)?;
        observe(&Event::Qualified {
            case,
            atoms: source.atom_count(),
            nodes: source.nodes(),
            roots: source.roots(),
            stable: &expected,
            pairs,
        })
        .map_err(Error::Output)?;
        for (phase, repetitions) in [
            ("qualification", 1),
            ("warmup", configuration.warmups),
            ("timed", configuration.repetitions),
        ] {
            for repetition in 0..repetitions {
                let mut guards = None;
                for route in [
                    Route::Direct,
                    Route::Feedback,
                    Route::Search,
                    Route::Restricted,
                ] {
                    let mut sample = Sample {
                        case,
                        route,
                        phase,
                        repetition,
                        progress: Progress::default(),
                        elapsed: StageTimes::default(),
                    };
                    let result = match route {
                        Route::Direct | Route::Feedback => replay::fixed(
                            &source,
                            route == Route::Feedback,
                            *configuration,
                            cancellation,
                            &mut sample.progress,
                            &mut sample.elapsed,
                        )
                        .map(|store| {
                            if route == Route::Feedback {
                                guards = Some(store);
                            }
                        }),
                        Route::Search | Route::Restricted => replay::search(
                            &source,
                            if route == Route::Restricted {
                                guards.as_ref()
                            } else {
                                None
                            },
                            *configuration,
                            cancellation,
                            &mut sample.progress,
                            &mut sample.elapsed,
                        ),
                    };
                    // Both complete-family comparison and publication occur
                    // after the measured route. The output preserves delivery order.
                    let result = result.and_then(|()| family(&sample.progress, &expected));
                    if let Err(error) = result {
                        return Err(report_failure(&mut observe, &sample, error));
                    }
                    observe(&Event::Sample(&sample)).map_err(Error::Output)?;
                    samples += 1;
                }
            }
        }
    }
    observe(&Event::Complete { samples }).map_err(Error::Output)
}

fn report_failure(
    observe: &mut impl FnMut(&Event<'_>) -> io::Result<()>,
    sample: &Sample,
    original: Error,
) -> Error {
    match observe(&Event::Failed {
        sample,
        error: &original,
    }) {
        Ok(()) => original,
        Err(output) => Error::FailureOutput {
            original: Box::new(original),
            output,
        },
    }
}

pub(super) fn family(progress: &Progress, expected: &[u64]) -> Result<(), Error> {
    if !progress.exhausted || progress.stable.len() != expected.len() {
        return Err(Error::Parity);
    }
    let mut seen = 0_u64;
    for &candidate in &progress.stable {
        let Some(bit) = 1_u64.checked_shl(u32::try_from(candidate).map_err(|_| Error::Parity)?)
        else {
            return Err(Error::Parity);
        };
        if seen & bit != 0 || !expected.contains(&candidate) {
            return Err(Error::Parity);
        }
        seen |= bit;
    }
    Ok(())
}

pub(super) fn qualify(
    source: &Theory,
    max_work: u64,
    cancellation: &Cancellation,
) -> Result<(Vec<u64>, u64), Error> {
    fixtures::shape(source)?;
    let mut expected = super::reserve(64)?;
    let limit = 1_u64 << source.atom_count();
    let reference = super::reference_limits(max_work);
    for bits in 0..limit {
        let candidate = fixtures::interpretation(source, bits)?;
        if zetesis_ferraris::check(source, &candidate, reference, cancellation)
            .map_err(Error::Control)?
            .accepted()
        {
            expected.push(bits);
        }
    }
    let mut pairs = 0;
    for witness_bits in 0..limit {
        let witness = fixtures::interpretation(source, witness_bits)?;
        // Qualification has fixed construction maxima, separately from the
        // caller's measured construction ceilings. No timing is taken here.
        let mut budget = Budget::new(ConstructionLimits::default(), cancellation);
        let guard = guard::compile(source, &witness, &mut budget)?;
        for candidate_bits in 0..limit {
            let candidate = fixtures::interpretation(source, candidate_bits)?;
            let proper = witness_bits != candidate_bits && witness_bits & !candidate_bits == 0;
            let models = zetesis_ferraris::models_reduct(
                source,
                &candidate,
                &witness,
                reference,
                cancellation,
            )
            .map_err(Error::Control)?;
            let allows = guard.allows(&candidate, max_work, cancellation)?;
            if allows == (proper && models) || (!allows && expected.contains(&candidate_bits)) {
                return Err(Error::Parity);
            }
            pairs += 1;
        }
    }
    Ok((expected, pairs))
}
