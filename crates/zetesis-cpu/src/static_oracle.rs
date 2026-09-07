//! Packed integer closure scans over an explicitly compiled static graph.

use zetesis_core::{AtomId, GroundProgram, GroundRule, Interpretation, Program, Seed};

use crate::{Control, Limits, Stop};

/// Work counters for a completed dense CPU invocation. These measure this
/// algorithm's operations, not equivalent work by the lazy or GPU backends.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StaticStatistics {
    /// Completed scans over enabled consequence rules, including the final
    /// unchanged scan. Zero when no consequence rule is enabled.
    pub passes: u64,
    /// Charged word initialization, seed packing, rule selection/scanning,
    /// membership tests, consequence insertion, and projection operations.
    pub work: u64,
    /// Distinct atoms in the completed least consequence closure.
    pub derived_atoms: usize,
    /// Rules selected by the frozen seed, including constraints.
    pub enabled_rules: usize,
}

/// A fully completed static reduct check with canonical packed closure words.
/// Resource stops never construct this value. The words use the dense IDs of
/// the [`GroundProgram`] supplied to [`check_static`].
#[derive(Clone, Debug)]
pub struct StaticCheck {
    program: Program,
    closure_words: Vec<u32>,
    constraint_violated: bool,
    seed_mismatch: bool,
    statistics: StaticStatistics,
}

impl StaticCheck {
    /// The admitted source instance underlying the checked graph. Constant time.
    #[must_use]
    pub const fn program(&self) -> &Program {
        &self.program
    }

    /// Decode the completed closure only with a graph of the checked instance.
    /// Compiled graphs of the same immutable program have the same canonical
    /// carrier order. This scans the carrier and clones selected atom payload into
    /// a tree set; it performs no grounding or membership check.
    /// Tree construction and payload copying use infallible allocation, not a
    /// typed resource refusal. See [`GroundProgram::interpretation_from_words`].
    ///
    /// # Errors
    /// Rejects a foreign program before decoding, even when word counts match.
    /// An invalid packed shape reports an internal admitted-invariant failure.
    pub fn interpretation(&self, graph: &GroundProgram) -> Result<Interpretation, Stop> {
        if !self.program.same_instance(graph.program()) {
            return Err(Stop::WrongProgram);
        }
        graph
            .interpretation_from_words(&self.closure_words)
            .map_err(|_| Stop::InvalidProgram)
    }

    /// Decode an accepted closure into a stable receipt for the checked program.
    /// A rejected check returns `None`; the original check remains available.
    /// Decoding has the cost of [`Self::interpretation`], with a shared program
    /// handle. No reduct computation is repeated.
    ///
    /// # Errors
    /// Rejects foreign graph identity even for a rejected check; an invalid word
    /// shape returns [`Stop::InvalidProgram`].
    pub fn stable_interpretation(
        &self,
        graph: &GroundProgram,
    ) -> Result<Option<crate::StableInterpretation>, Stop> {
        if !self.program.same_instance(graph.program()) {
            return Err(Stop::WrongProgram);
        }
        if !self.accepted() {
            return Ok(None);
        }
        Ok(Some(crate::StableInterpretation::new(
            self.program.clone(),
            self.interpretation(graph)?,
        )))
    }

    /// Exact least closure of the selected positive reduct, even for rejection.
    /// Word count matches the graph and all unused tail bits are zero.
    #[must_use]
    pub fn closure_words(&self) -> &[u32] {
        &self.closure_words
    }

    /// Whether the completed closure has no violated constraint and agrees with
    /// the candidate on the entire conservative gate carrier.
    #[must_use]
    pub fn accepted(&self) -> bool {
        !self.constraint_violated && !self.seed_mismatch
    }

    /// An enabled constraint's positive body is true in the final closure.
    #[must_use]
    pub fn constraint_violated(&self) -> bool {
        self.constraint_violated
    }

    /// The final closure's gate projection differs from the frozen candidate.
    #[must_use]
    pub fn seed_mismatch(&self) -> bool {
        self.seed_mismatch
    }

    /// Counters for this completed invocation.
    #[must_use]
    pub fn statistics(&self) -> StaticStatistics {
        self.statistics
    }
}

struct Work<'a> {
    limits: Limits,
    control: &'a Control,
    statistics: StaticStatistics,
}

impl Work<'_> {
    fn tick(&mut self) -> Result<(), Stop> {
        self.control.poll()?;
        if self.statistics.work >= self.limits.max_work {
            return Err(Stop::WorkLimit);
        }
        self.statistics.work += 1;
        Ok(())
    }

    fn words(&mut self, count: usize) -> Result<Vec<u32>, Stop> {
        let mut words = Vec::new();
        words
            .try_reserve_exact(count)
            .map_err(|_| Stop::Allocation)?;
        for _ in 0..count {
            self.tick()?;
            words.push(0);
        }
        Ok(words)
    }
}

fn contains(words: &[u32], atom: AtomId) -> bool {
    words[(atom / u32::BITS) as usize] & (1 << (atom % u32::BITS)) != 0
}

fn insert(words: &mut [u32], atom: AtomId) {
    words[(atom / u32::BITS) as usize] |= 1 << (atom % u32::BITS);
}

fn all_present(words: &[u32], atoms: &[AtomId], work: &mut Work<'_>) -> Result<bool, Stop> {
    for &atom in atoms {
        work.tick()?;
        if !contains(words, atom) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn enabled(rule: &GroundRule, frozen: &[u32], work: &mut Work<'_>) -> Result<bool, Stop> {
    if !all_present(frozen, rule.gate_true(), work)? {
        return Ok(false);
    }
    for &atom in rule.gate_false() {
        work.tick()?;
        if contains(frozen, atom) {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Check an already compiled graph using packed integer bits. Gates are frozen
/// once, then enabled consequence rules update the empty closure in place until
/// a full scan changes nothing. Constraints and the full gate projection are
/// checked against the completed closure. No compilation or carrier generation
/// occurs here; this is the explicit static CPU profile.
///
/// [`Limits::max_work`] bounds the operations listed in [`StaticStatistics::work`].
/// Cancellation and deadlines are polled before each charged operation, before
/// invocation validation, and before returning a completed result. The atom
/// limit bounds distinct consequences, not the graph's already compiled carrier.
///
/// # Errors
/// Returns [`Stop`] for foreign program identity, cancellation, deadline, work or
/// consequence limits, allocation refusal, or an invalid admitted invariant.
/// No partial closure is returned as a logical decision.
pub fn check_static(
    graph: &GroundProgram,
    seed: &Seed,
    limits: Limits,
    control: &Control,
) -> Result<StaticCheck, Stop> {
    control.poll()?;
    if !graph.program().same_instance(seed.program()) {
        return Err(Stop::WrongProgram);
    }
    let mut work = Work {
        limits,
        control,
        statistics: StaticStatistics::default(),
    };
    let mut frozen = work.words(graph.word_count())?;
    let mut closure = work.words(graph.word_count())?;
    for atom in seed.atoms() {
        work.tick()?;
        insert(
            &mut frozen,
            graph.atom_id(atom).ok_or(Stop::InvalidProgram)?,
        );
    }
    let mut consequences = Vec::new();
    let mut constraints = Vec::new();
    for rule in graph.rules() {
        work.tick()?;
        if enabled(rule, &frozen, &mut work)? {
            let selected = if rule.head().is_some() {
                &mut consequences
            } else {
                &mut constraints
            };
            selected.try_reserve(1).map_err(|_| Stop::Allocation)?;
            selected.push(rule);
            work.statistics.enabled_rules += 1;
        }
    }
    if !consequences.is_empty() {
        close(&consequences, &mut closure, &mut work)?;
    }
    let mut constraint_violated = false;
    for rule in constraints {
        work.tick()?;
        if all_present(&closure, rule.positive(), &mut work)? {
            constraint_violated = true;
            break;
        }
    }
    let mut seed_mismatch = false;
    for &atom in graph.gate_atom_ids() {
        work.tick()?;
        if contains(&closure, atom) != contains(&frozen, atom) {
            seed_mismatch = true;
            break;
        }
    }
    control.poll()?;
    Ok(StaticCheck {
        program: graph.program().clone(),
        closure_words: closure,
        constraint_violated,
        seed_mismatch,
        statistics: work.statistics,
    })
}

fn close(rules: &[&GroundRule], closure: &mut [u32], work: &mut Work<'_>) -> Result<(), Stop> {
    loop {
        work.tick()?;
        let mut changed = false;
        for rule in rules {
            work.tick()?;
            let head = rule.head().ok_or(Stop::InvalidProgram)?;
            if contains(closure, head) || !all_present(closure, rule.positive(), work)? {
                continue;
            }
            work.tick()?;
            if work.statistics.derived_atoms >= work.limits.max_derived_atoms {
                return Err(Stop::DerivedAtomLimit);
            }
            insert(closure, head);
            work.statistics.derived_atoms += 1;
            changed = true;
        }
        work.statistics.passes += 1;
        if !changed {
            return Ok(());
        }
    }
}
