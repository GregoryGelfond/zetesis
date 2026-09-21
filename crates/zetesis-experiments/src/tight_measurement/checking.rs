use rayon::prelude::*;
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{Interpretation, TightPlan, TightVerdict, Verdict};

use super::{
    Activity, Certificate, Configuration, Decision, Error, Outcome, Reference, fixture::Fixture,
    reserve,
};

pub(super) struct Prepared {
    pub fixture: Fixture,
    pub plan: TightPlan,
    pub certificates: Vec<TightVerdict>,
    pub reference: Vec<zetesis_sat::Check>,
}

pub(super) fn prepare(
    case: super::Case,
    configuration: &Configuration,
    cancellation: &Cancellation,
) -> Result<Prepared, Error> {
    let fixture = super::fixture::build(case)?;
    let plan = TightPlan::compile(&fixture.theory, configuration.plan_limits, cancellation)
        .map_err(Error::Certificate)?;
    let mut reference = reserve(fixture.candidates.len())?;
    let mut certificates = reserve(fixture.candidates.len())?;
    for candidate in &fixture.candidates {
        let check = match case.reference {
            Reference::ExhaustiveReduct => {
                let exact = zetesis_ferraris::check(
                    &fixture.theory,
                    candidate,
                    configuration.reference_limits,
                    cancellation,
                )
                .map_err(Error::Cpu)?;
                match exact.verdict() {
                    Verdict::Stable => zetesis_sat::Check::Stable,
                    Verdict::NotModel { .. } => zetesis_sat::Check::NotModel,
                    Verdict::NonMinimal { witness } => zetesis_sat::Check::NonMinimal(
                        Interpretation::new(&fixture.theory, witness.atoms())
                            .map_err(Error::Admission)?,
                    ),
                }
            }
            Reference::GeneralReduct => zetesis_sat::check(
                &fixture.theory,
                candidate,
                configuration.residual_limits,
                cancellation,
            ),
        };
        validate_witness(candidate, &check, configuration, cancellation)?;
        let certificate = plan
            .check_accounted(candidate, configuration.certificate_limits, cancellation)
            .result
            .map_err(Error::Certificate)?
            .verdict;
        if !compatible(certificate, &check) {
            return Err(Error::Parity);
        }
        validate_original_failure(candidate, certificate, configuration, cancellation)?;
        reference.push(check);
        certificates.push(certificate);
    }
    Ok(Prepared {
        fixture,
        plan,
        certificates,
        reference,
    })
}

pub(super) fn classify(
    prepared: &Prepared,
    configuration: &Configuration,
    pool: Option<&rayon::ThreadPool>,
    cancellation: &Cancellation,
    activity: &mut Activity,
) -> Result<Vec<TightVerdict>, Error> {
    let candidates = &prepared.fixture.candidates;
    let mut attempts = reserve(candidates.len())?;
    let check = |candidate: &Interpretation| {
        prepared
            .plan
            .check_accounted(candidate, configuration.certificate_limits, cancellation)
    };
    if let Some(pool) = pool {
        pool.install(|| {
            candidates
                .par_iter()
                .map(check)
                .collect_into_vec(&mut attempts);
        });
    } else {
        attempts.extend(candidates.iter().map(check));
    }
    // All indexed jobs have joined. Preserve every attempt's charge even if an
    // earlier occurrence failed; the first occurrence error is the reported one.
    // Validated fixtures have C<=256, A<=256, N<=4A and R,P<=A-1.
    // Each attempt charges at most N+R+P+A; the sum is at most 458,240,
    // independently of the caller's max_work ceiling.
    for attempt in &attempts {
        activity.cpu_certificate_attempts += 1;
        activity.cpu_certificate_work += attempt.work;
        if let Ok(check) = &attempt.result {
            activity.classified += 1;
            activity.cpu_certificate_max_bytes = Some(
                activity
                    .cpu_certificate_max_bytes
                    .unwrap_or(0)
                    .max(check.logical_bytes),
            );
            activity.cpu_decisions +=
                usize::from(!matches!(check.verdict, TightVerdict::Residual { .. }));
        }
    }
    let mut verdicts = reserve(attempts.len())?;
    for attempt in attempts {
        verdicts.push(attempt.result.map_err(Error::Certificate)?.verdict);
    }
    Ok(verdicts)
}

pub(super) fn complete(
    prepared: &Prepared,
    verdicts: &[TightVerdict],
    configuration: &Configuration,
    cancellation: &Cancellation,
    activity: &mut Activity,
) -> Result<Vec<zetesis_sat::Check>, Error> {
    cancellation.poll().map_err(Error::Cpu)?;
    if verdicts.len() != prepared.fixture.candidates.len() {
        return Err(Error::Parity);
    }
    let mut checks = reserve(verdicts.len())?;
    for (verdict, candidate) in verdicts.iter().zip(&prepared.fixture.candidates) {
        cancellation.poll().map_err(Error::Cpu)?;
        let check = match verdict {
            TightVerdict::Stable => zetesis_sat::Check::Stable,
            TightVerdict::NotModel { .. } => zetesis_sat::Check::NotModel,
            TightVerdict::Residual { .. } => {
                activity.residual_attempts += 1;
                let check = zetesis_sat::check(
                    &prepared.fixture.theory,
                    candidate,
                    configuration.residual_limits,
                    cancellation,
                );
                if let zetesis_sat::Check::Inconclusive(reason) = check {
                    return Err(Error::Residual(reason));
                }
                activity.residual_completed += 1;
                check
            }
        };
        checks.push(check);
    }
    Ok(checks)
}

pub(super) fn validate(
    prepared: &Prepared,
    verdicts: &[TightVerdict],
    checks: &[zetesis_sat::Check],
    configuration: &Configuration,
    cancellation: &Cancellation,
) -> Result<(), Error> {
    cancellation.poll().map_err(Error::Cpu)?;
    if verdicts != prepared.certificates || checks.len() != prepared.reference.len() {
        return Err(Error::Parity);
    }
    for ((candidate, check), reference) in prepared
        .fixture
        .candidates
        .iter()
        .zip(checks)
        .zip(&prepared.reference)
    {
        cancellation.poll().map_err(Error::Cpu)?;
        validate_witness(candidate, check, configuration, cancellation)?;
        if !same_decision(check, reference) {
            return Err(Error::Parity);
        }
    }
    Ok(())
}

fn compatible(certificate: TightVerdict, exact: &zetesis_sat::Check) -> bool {
    match certificate {
        TightVerdict::Stable => matches!(exact, zetesis_sat::Check::Stable),
        TightVerdict::NotModel { .. } => matches!(exact, zetesis_sat::Check::NotModel),
        TightVerdict::Residual { .. } => matches!(
            exact,
            zetesis_sat::Check::Stable | zetesis_sat::Check::NonMinimal(_)
        ),
    }
}

fn same_decision(left: &zetesis_sat::Check, right: &zetesis_sat::Check) -> bool {
    matches!(
        (left, right),
        (zetesis_sat::Check::Stable, zetesis_sat::Check::Stable)
            | (zetesis_sat::Check::NotModel, zetesis_sat::Check::NotModel)
            | (
                zetesis_sat::Check::NonMinimal(_),
                zetesis_sat::Check::NonMinimal(_)
            )
    )
}

fn validate_witness(
    candidate: &Interpretation,
    check: &zetesis_sat::Check,
    configuration: &Configuration,
    cancellation: &Cancellation,
) -> Result<(), Error> {
    match check {
        zetesis_sat::Check::Inconclusive(reason) => Err(Error::Residual(*reason)),
        zetesis_sat::Check::NonMinimal(witness) => {
            if !candidate.theory().same_instance(witness.theory())
                || !witness.atoms().all(|atom| candidate.contains(atom))
                || !candidate.atoms().any(|atom| !witness.contains(atom))
                || !zetesis_ferraris::models_reduct(
                    candidate.theory(),
                    candidate,
                    witness,
                    configuration.reference_limits,
                    cancellation,
                )
                .map_err(Error::Cpu)?
            {
                return Err(Error::Parity);
            }
            Ok(())
        }
        zetesis_sat::Check::Stable | zetesis_sat::Check::NotModel => Ok(()),
    }
}

pub(super) fn outcomes(
    verdicts: &[TightVerdict],
    checks: &[zetesis_sat::Check],
) -> Result<Vec<Outcome>, Error> {
    let mut outcomes = reserve(verdicts.len())?;
    for (&certificate, check) in verdicts.iter().zip(checks) {
        let decision = match check {
            zetesis_sat::Check::Stable => Decision::Stable,
            zetesis_sat::Check::NotModel => Decision::NotModel,
            zetesis_sat::Check::NonMinimal(witness) => {
                let mut atoms = reserve(witness.theory().atom_count())?;
                atoms.extend(witness.atoms());
                Decision::NonMinimal { witness: atoms }
            }
            zetesis_sat::Check::Inconclusive(reason) => return Err(Error::Residual(*reason)),
        };
        outcomes.push(Outcome {
            certificate: Certificate::from(certificate),
            decision,
        });
    }
    Ok(outcomes)
}

fn validate_original_failure(
    candidate: &Interpretation,
    certificate: TightVerdict,
    configuration: &Configuration,
    cancellation: &Cancellation,
) -> Result<(), Error> {
    if let TightVerdict::NotModel { root } = certificate {
        // This instrument returns before subset enumeration on a nonmodel.
        // A mistaken classification cannot trigger exponential wide reference work.
        let check = zetesis_ferraris::check(
            candidate.theory(),
            candidate,
            zetesis_ferraris::Limits {
                max_subsets: 0,
                ..configuration.reference_limits
            },
            cancellation,
        )
        .map_err(Error::Cpu)?;
        if !matches!(check.verdict(), Verdict::NotModel { root: actual } if *actual == root) {
            return Err(Error::Parity);
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../tests/tight/checking.rs"]
mod tests;
