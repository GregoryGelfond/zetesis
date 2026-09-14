use super::{
    Configuration, Error, Progress, StageTimes, fixtures,
    guard::{Budget, Store},
};
use std::time::Instant;
use zetesis_cpu::Control;
use zetesis_ferraris::Theory;

pub(super) fn fixed(
    source: &Theory,
    feedback: bool,
    configuration: Configuration,
    control: &Control,
    progress: &mut Progress,
    elapsed: &mut StageTimes,
) -> Result<Store, Error> {
    let started = Instant::now();
    let mut budget = Budget::new(configuration.construction, control);
    let outcome = fixed_inner(
        source,
        feedback,
        configuration,
        control,
        progress,
        elapsed,
        &mut budget,
    );
    progress.construction.work = budget.work;
    progress.construction.nodes = budget.retained_nodes;
    progress.construction.retained_bytes = budget.retained_bytes;
    progress.construction.peak_build_bytes = budget.peak_build_bytes;
    progress.construction.peak_live_bytes = budget.peak_live_bytes;
    elapsed.total_ns = started.elapsed().as_nanos();
    outcome
}

fn fixed_inner(
    source: &Theory,
    feedback: bool,
    configuration: Configuration,
    control: &Control,
    progress: &mut Progress,
    elapsed: &mut StageTimes,
    budget: &mut Budget<'_>,
) -> Result<Store, Error> {
    progress.stable = super::reserve(64)?;
    progress.witnesses = super::reserve(8)?;
    control.poll().map_err(Error::Control)?;
    let mut store = if feedback {
        Store::new(budget)?
    } else {
        Store { guards: Vec::new() }
    };
    for bits in 0..1 << source.atom_count() {
        control.poll().map_err(Error::Control)?;
        let candidate = fixtures::interpretation(source, bits)?;
        progress.candidates_started += 1;
        let mut allowed = true;
        for guard in &store.guards {
            progress.guard_evaluations += 1;
            let clock = Instant::now();
            let result = guard.allows(&candidate, configuration.max_reference_work, control);
            elapsed.application_ns += clock.elapsed().as_nanos();
            if !result? {
                allowed = false;
                break;
            }
        }
        if allowed {
            let checked = membership(candidate, configuration, control, progress, elapsed)?;
            if feedback && let zetesis_sat::Check::NonMinimal(witness) = checked.verdict() {
                let clock = Instant::now();
                let learned = store.learn(&checked, budget);
                elapsed.construction_ns += clock.elapsed().as_nanos();
                if learned? {
                    progress.construction.guards += 1;
                    progress.witnesses.push(fixtures::bits(witness));
                } else {
                    progress.duplicate_witnesses += 1;
                }
            }
        } else {
            progress.filtered += 1;
            progress.candidates_completed += 1;
        }
    }
    control.poll().map_err(Error::Control)?;
    progress.exhausted = true;
    Ok(store)
}

fn membership(
    candidate: zetesis_ferraris::Interpretation,
    configuration: Configuration,
    control: &Control,
    progress: &mut Progress,
    elapsed: &mut StageTimes,
) -> Result<zetesis_sat::CheckedInterpretation, Error> {
    progress.membership_calls += 1;
    let clock = Instant::now();
    let checked = zetesis_sat::check_interpretation(candidate, configuration.native(), control);
    elapsed.membership_ns += clock.elapsed().as_nanos();
    match checked.verdict() {
        zetesis_sat::Check::Stable => progress.stable.push(fixtures::bits(checked.candidate())),
        zetesis_sat::Check::NotModel => progress.not_models += 1,
        zetesis_sat::Check::NonMinimal(_) => progress.nonminimal += 1,
        zetesis_sat::Check::Inconclusive(error) => return Err(Error::Native(*error)),
    }
    // Membership is established before optional learning can refuse.
    progress.membership_completed += 1;
    progress.candidates_completed += 1;
    Ok(checked)
}

pub(super) fn search(
    source: &Theory,
    guards: Option<&Store>,
    configuration: Configuration,
    control: &Control,
    progress: &mut Progress,
    elapsed: &mut StageTimes,
) -> Result<(), Error> {
    let started = Instant::now();
    let result = search_inner(source, guards, configuration, control, progress, elapsed);
    elapsed.total_ns = started.elapsed().as_nanos();
    result
}
fn search_inner(
    source: &Theory,
    guards: Option<&Store>,
    configuration: Configuration,
    control: &Control,
    progress: &mut Progress,
    elapsed: &mut StageTimes,
) -> Result<(), Error> {
    progress.stable = super::reserve(64)?;
    if let Some(store) = guards {
        progress.pre_acquired_guards = store.guards.len();
        (progress.pre_acquired_nodes, progress.pre_acquired_bytes) = store.storage();
        progress.witnesses = super::reserve(8)?;
        progress
            .witnesses
            .extend(store.guards.iter().map(super::guard::Guard::witness_bits));
    }
    let clock = Instant::now();
    let result = zetesis_sat::StableModels::new(source, configuration.native(), control.clone());
    elapsed.search_setup_ns += clock.elapsed().as_nanos();
    let mut search = result.map_err(Error::Native)?;
    let result = enumerate(&mut search, guards, progress, elapsed);
    progress.native = Some(search.statistics().into());
    progress.restarts = search.statistics().candidate_restrictions;
    progress.exhausted = search.exhausted();
    result
}
fn enumerate(
    search: &mut zetesis_sat::StableModels,
    guards: Option<&Store>,
    progress: &mut Progress,
    elapsed: &mut StageTimes,
) -> Result<(), Error> {
    loop {
        let clock = Instant::now();
        let next = search.next();
        elapsed.membership_ns += clock.elapsed().as_nanos();
        let Some(next) = next else {
            break;
        };
        let stable = next.map_err(Error::Native)?;
        if progress.stable.len() == 64 {
            return Err(Error::Invariant);
        }
        progress.stable.push(fixtures::bits(&stable));
        if progress.stable.len() == 1
            && let Some(store) = guards
        {
            for guard in &store.guards {
                // The source handle, not only its atom count, authorizes the
                // public restriction's dense-ID interpretation.
                guard.owner(search.theory())?;
                progress.installation_attempts += 1;
                let clock = Instant::now();
                let result = search.restrict_candidates(&guard.restriction);
                elapsed.installation_ns += clock.elapsed().as_nanos();
                result.map_err(Error::Native)?;
            }
        }
    }
    if !search.exhausted() {
        return Err(Error::Invariant);
    }
    Ok(())
}
