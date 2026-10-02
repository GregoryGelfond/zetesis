use crate::{
    EvaluationError, EvaluationLimits, EvaluationWorkspace, FrozenReduct, Interpretation, Node,
    Theory,
};
use zetesis_cpu::{Cancellation, Stop};

/// Per-call bounds for exact finite checking. Exceeding a bound is incomplete,
/// never a proof of stability or nonminimality.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Charged node evaluations, root tests, atom scans and subset-bit operations.
    pub max_work: u64,
    /// Maximum number of proper subsets checked against a frozen reduct.
    pub max_subsets: u64,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            max_work: 100_000_000,
            max_subsets: 1_048_576,
        }
    }
}

/// Logical work performed before a completed decision.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Statistics {
    /// Charged primitive operations, including classical candidate evaluation.
    pub work: u64,
    /// Proper subsets evaluated; the original candidate is never counted.
    pub subsets: u64,
}

/// Completed membership verdict data with a checkable rejection witness. The
/// public variants do not retain the original checked subject; use
/// [`crate::check_interpretation`] for an owned subject-bound native decision.
#[derive(Clone, Debug)]
pub enum Verdict {
    /// No proper subset satisfies the candidate's frozen formula reduct.
    Stable,
    /// The original interpretation fails this asserted root node.
    NotModel {
        /// Circuit node index of a false asserted formula.
        root: usize,
    },
    /// This proper subset models the frozen reduct.
    NonMinimal {
        /// Program-bound counterexample to subset minimality.
        witness: Interpretation,
    },
}

/// Completed decision and exact logical-work accounting.
#[derive(Clone, Debug)]
pub struct Check {
    verdict: Verdict,
    statistics: Statistics,
}
impl Check {
    /// Stable-model membership decision, or the reason for rejection.
    #[must_use]
    pub fn verdict(&self) -> &Verdict {
        &self.verdict
    }
    /// True only for a completed minimality proof.
    #[must_use]
    pub fn accepted(&self) -> bool {
        matches!(self.verdict, Verdict::Stable)
    }
    /// Work consumed by this completed call.
    #[must_use]
    pub fn statistics(&self) -> Statistics {
        self.statistics
    }
}

pub(super) struct Work<'a> {
    pub(super) limits: Limits,
    pub(super) cancellation: &'a Cancellation,
    pub(super) statistics: Statistics,
}
impl Work<'_> {
    pub(super) fn tick(&mut self) -> Result<(), Stop> {
        self.cancellation.poll()?;
        if self.statistics.work >= self.limits.max_work {
            return Err(Stop::WorkLimit);
        }
        self.statistics.work += 1;
        Ok(())
    }
}

pub(super) fn identities(theory: &Theory, interpretation: &Interpretation) -> Result<(), Stop> {
    if theory.same_instance(interpretation.theory()) {
        Ok(())
    } else {
        Err(Stop::WrongProgram)
    }
}

pub(super) fn reserve<T>(count: usize) -> Result<Vec<T>, Stop> {
    let mut vector = Vec::new();
    vector
        .try_reserve_exact(count)
        .map_err(|_| Stop::Allocation)?;
    Ok(vector)
}

pub(super) fn evaluate(
    theory: &Theory,
    interpretation: &Interpretation,
    frozen: Option<&[bool]>,
    output: &mut Vec<bool>,
    work: &mut Work<'_>,
) -> Result<(), Stop> {
    output.clear();
    for (index, node) in theory.nodes().iter().enumerate() {
        work.tick()?;
        let value = match *node {
            Node::Atom(atom) => interpretation.contains(atom),
            Node::False => false,
            Node::And(a, b) => output[a] && output[b],
            Node::Or(a, b) => output[a] || output[b],
            Node::Implies(a, b) => !output[a] || output[b],
        };
        // A maximal M-false subformula becomes falsum. Masking every M-false
        // node has the same root meaning and avoids materializing a new DAG.
        output.push(value && frozen.is_none_or(|mask| mask[index]));
    }
    Ok(())
}

pub(super) fn failed_root(
    theory: &Theory,
    values: &[bool],
    work: &mut Work<'_>,
) -> Result<Option<usize>, Stop> {
    for root in theory.roots() {
        work.tick()?;
        if !values[*root] {
            return Ok(Some(*root));
        }
    }
    Ok(None)
}

/// Classical satisfaction of a finite theory.
///
/// # Errors
/// Refuses foreign interpretations, cancellation, deadlines, allocation, or work limits.
pub fn models(
    theory: &Theory,
    interpretation: &Interpretation,
    limits: Limits,
    cancellation: &Cancellation,
) -> Result<bool, Stop> {
    identities(theory, interpretation)?;
    EvaluationWorkspace::default()
        .evaluate(
            interpretation,
            EvaluationLimits {
                max_work: limits.max_work,
                max_bytes: usize::MAX,
            },
            cancellation,
        )
        .result
        .map(|evaluation| evaluation.is_model())
        .map_err(|error| match error {
            EvaluationError::Stopped(stop) => stop,
            // The convenience operation imposes no finite byte ceiling. A
            // charge beyond usize is an unrepresentable allocation request.
            EvaluationError::Storage { .. } => Stop::Allocation,
        })
}

/// Satisfaction in `tested` of the formula reduct frozen in `candidate`.
/// No subset relation is required; minimality search imposes that separately.
/// This call freezes the candidate anew and shares one work budget between
/// freezing and testing. Use [`FrozenReduct`] to reuse a freeze across tests,
/// with a separate budget for construction and each satisfaction query.
///
/// # Errors
/// Refuses foreign interpretations, cancellation, deadlines, allocation, or work limits.
pub fn models_reduct(
    theory: &Theory,
    candidate: &Interpretation,
    tested: &Interpretation,
    limits: Limits,
    cancellation: &Cancellation,
) -> Result<bool, Stop> {
    identities(theory, candidate)?;
    identities(theory, tested)?;
    cancellation.poll()?;
    let mut work = Work {
        limits,
        cancellation,
        statistics: Statistics::default(),
    };
    let mut values = reserve(theory.nodes().len())?;
    let reduct = FrozenReduct::freeze(candidate, &mut work)?;
    reduct.satisfied_by(tested, &mut values, &mut work)
}

/// Append the candidate's atom coordinates in increasing order. The caller has
/// checked theory identity and reserved an empty destination for the universe.
/// A stop retains the selected prefix and work charged before the refused tick.
fn select_atoms(
    theory: &Theory,
    candidate: &Interpretation,
    selected: &mut Vec<usize>,
    work: &mut Work<'_>,
) -> Result<(), Stop> {
    for atom in 0..theory.atom_count() {
        work.tick()?;
        if candidate.contains(atom) {
            selected.push(atom);
        }
    }
    Ok(())
}

/// Advance the selected coordinates as a binary counter. The caller supplies
/// distinct in-universe coordinates, a subset supported on those coordinates,
/// and its population. `check` calls this only for a proper subset. A stop retains
/// preceding bit and population updates; the refused tick performs no update.
fn advance_subset(
    selected: &[usize],
    subset: &mut Interpretation,
    present: &mut usize,
    work: &mut Work<'_>,
) -> Result<(), Stop> {
    for atom in selected {
        work.tick()?;
        let packed = &mut subset.words[*atom / 64];
        let bit = 1 << (*atom % 64);
        if *packed & bit == 0 {
            *packed |= bit;
            *present += 1;
            break;
        }
        *packed &= !bit;
        *present -= 1;
    }
    Ok(())
}

/// Decide stable-model membership by classical satisfaction and exhaustive
/// proper-subset checking of the Ferraris formula reduct. This reference kernel
/// is exponential in the candidate size and does not assume a least reduct model.
///
/// # Errors
/// Refuses foreign candidates, cancellation, deadlines, allocation, or exhausted
/// work/subset bounds. A subset bound uses `Stop::CandidateLimit` because it
/// bounds candidate countermodels; no partial search returns `Stable`.
pub fn check(
    theory: &Theory,
    candidate: &Interpretation,
    limits: Limits,
    cancellation: &Cancellation,
) -> Result<Check, Stop> {
    identities(theory, candidate)?;
    cancellation.poll()?;
    let mut work = Work {
        limits,
        cancellation,
        statistics: Statistics::default(),
    };
    let mut frozen = reserve(theory.nodes().len())?;
    evaluate(theory, candidate, None, &mut frozen, &mut work)?;
    if let Some(root) = failed_root(theory, &frozen, &mut work)? {
        return Ok(Check {
            verdict: Verdict::NotModel { root },
            statistics: work.statistics,
        });
    }
    let mut selected = reserve(theory.atom_count())?;
    select_atoms(theory, candidate, &mut selected, &mut work)?;
    let mut words = reserve(candidate.words.len())?;
    words.resize(candidate.words.len(), 0);
    let mut subset = Interpretation {
        theory: theory.clone(),
        words,
    };
    let mut values = reserve(theory.nodes().len())?;
    // Empty is the first proper subset unless M itself is empty. Incrementing
    // over selected atom indices avoids machine-word cardinality restrictions.
    let mut present = 0;
    while present < selected.len() {
        cancellation.poll()?;
        if work.statistics.subsets >= limits.max_subsets {
            return Err(Stop::CandidateLimit);
        }
        work.statistics.subsets += 1;
        evaluate(theory, &subset, Some(&frozen), &mut values, &mut work)?;
        if failed_root(theory, &values, &mut work)?.is_none() {
            return Ok(Check {
                verdict: Verdict::NonMinimal { witness: subset },
                statistics: work.statistics,
            });
        }
        advance_subset(&selected, &mut subset, &mut present, &mut work)?;
    }
    Ok(Check {
        verdict: Verdict::Stable,
        statistics: work.statistics,
    })
}

#[cfg(test)]
mod tests {
    use super::{Limits, Statistics, Work, advance_subset, reserve, select_atoms};
    use crate::{AdmissionLimits, Interpretation, Theory};
    use zetesis_cpu::{Cancellation, Stop};

    fn work(cancellation: &Cancellation, allowance: u64) -> Work<'_> {
        Work {
            limits: Limits {
                max_work: 7 + allowance,
                max_subsets: 5,
            },
            cancellation,
            statistics: Statistics {
                work: 7,
                subsets: 3,
            },
        }
    }

    #[test]
    fn selection_stops_retain_the_exact_scanned_prefix() {
        let theory = Theory::new(130, vec![], vec![], AdmissionLimits::default()).unwrap();
        let atoms = [0, 63, 64, 129];
        let candidate = Interpretation::new(&theory, atoms).unwrap();
        let cancellation = Cancellation::default();
        for allowance in [0, 1, 63, 64, 65, 129, 130, 131] {
            let mut selected = reserve(theory.atom_count()).unwrap();
            let mut work = work(&cancellation, u64::try_from(allowance).unwrap());
            let result = select_atoms(&theory, &candidate, &mut selected, &mut work);
            let expected = atoms
                .into_iter()
                .filter(|atom| *atom < allowance)
                .collect::<Vec<_>>();
            assert_eq!(selected, expected, "allowance {allowance}");
            assert_eq!(
                result,
                if allowance < 130 {
                    Err(Stop::WorkLimit)
                } else {
                    Ok(())
                },
                "allowance {allowance}"
            );
            assert_eq!(
                work.statistics,
                Statistics {
                    work: 7 + u64::try_from(allowance.min(130)).unwrap(),
                    subsets: 3,
                }
            );
        }
    }

    #[test]
    fn carry_stops_retain_completed_bit_updates() {
        let theory = Theory::new(130, vec![], vec![], AdmissionLimits::default()).unwrap();
        let cancellation = Cancellation::default();
        // The carry clears 0 and 63, sets 64, then stops before reaching 129.
        let expected: [&[usize]; 5] = [&[0, 63], &[63], &[], &[64], &[64]];
        for (allowance, expected) in expected.into_iter().enumerate() {
            let mut subset = Interpretation::new(&theory, [0, 63]).unwrap();
            let mut present = 2;
            let mut work = work(&cancellation, u64::try_from(allowance).unwrap());
            let result = advance_subset(&[0, 63, 64, 129], &mut subset, &mut present, &mut work);
            assert_eq!(subset.atoms().collect::<Vec<_>>(), expected);
            assert_eq!(present, expected.len());
            assert_eq!(
                result,
                if allowance < 3 {
                    Err(Stop::WorkLimit)
                } else {
                    Ok(())
                }
            );
            assert_eq!(
                work.statistics,
                Statistics {
                    work: 7 + u64::try_from(allowance.min(3)).unwrap(),
                    subsets: 3,
                }
            );
        }
    }

    #[test]
    fn empty_atom_scan_performs_no_poll() {
        let theory = Theory::new(0, vec![], vec![], AdmissionLimits::default()).unwrap();
        let candidate = Interpretation::new(&theory, []).unwrap();
        let cancellation = Cancellation::default();
        cancellation.cancel();
        let mut selected = reserve(0).unwrap();
        let mut work = work(&cancellation, 0);
        let before = work.statistics;
        assert_eq!(
            select_atoms(&theory, &candidate, &mut selected, &mut work),
            Ok(())
        );
        assert!(selected.is_empty());
        assert_eq!(work.statistics, before);
    }

    #[test]
    fn empty_carry_performs_no_poll() {
        let theory = Theory::new(0, vec![], vec![], AdmissionLimits::default()).unwrap();
        let mut subset = Interpretation::new(&theory, []).unwrap();
        let cancellation = Cancellation::default();
        cancellation.cancel();
        let mut present = 0;
        let mut work = work(&cancellation, 0);
        let before = work.statistics;
        assert_eq!(
            advance_subset(&[], &mut subset, &mut present, &mut work),
            Ok(())
        );
        assert!(subset.atoms().next().is_none());
        assert_eq!(present, 0);
        assert_eq!(work.statistics, before);
    }

    #[test]
    fn unrepresentable_workspace_returns_allocation_stop() {
        // Capacity overflow is deterministic and needs no process allocator hook.
        // Every frozen truth mask and tested workspace uses this reservation.
        assert_eq!(reserve::<bool>(usize::MAX).unwrap_err(), Stop::Allocation);
    }
}
