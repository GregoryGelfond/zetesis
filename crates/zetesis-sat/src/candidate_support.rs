//! Necessary ordinary disjunctive support constrains only the outer query.

use zetesis_ferraris::{SupportError, SupportLimits, Theory, support_restriction};

use crate::search::Budget;
use crate::{AdmissionError, Cnf, Incomplete, Limits, encoding};

/// Disposition of optional complete-theory candidate support construction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SupportStatus {
    /// A necessary support restriction was appended to the outer query.
    Applied,
    /// The theory has no ordinary disjunctive head, or an asserted head lies
    /// outside the complete certificate grammar. General search remains active.
    NotApplicable,
    /// The optional formula's shape exceeded its construction dimensions.
    FormulaLimit,
    /// The optional encoding exceeded the outer CNF's remaining dimensions.
    EncodingLimit(AdmissionError),
}

/// Work spent on the initial candidate support attempt, including declined
/// grammar and optional shape refusals. These are subsets of search work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SupportStatistics {
    /// Whether necessary support was applied or why general search was retained.
    pub status: SupportStatus,
    /// Complete-root recognition and formula construction operations.
    pub construction_work: u64,
    /// CNF encoding operations, including an encoding rolled back on refusal.
    pub encoding_work: u64,
}

pub(super) fn restrict(
    cnf: &mut Cnf,
    theory: &Theory,
    limits: Limits,
    budget: &mut Budget<'_>,
) -> Result<SupportStatistics, Incomplete> {
    // This optional formula has separate shape dimensions derived from the
    // caller's SAT admission. It never borrows the original theory's identity.
    let attempt = support_restriction(
        theory,
        SupportLimits {
            admission: zetesis_ferraris::AdmissionLimits {
                max_atoms: limits.admission.max_variables,
                max_nodes: limits.admission.max_literals,
                max_roots: limits.admission.max_clauses,
            },
            max_work: budget
                .limits
                .max_work
                .saturating_sub(budget.statistics.work),
        },
        budget.control,
    );
    // Construction admits operations against exactly the coordinator's
    // remaining quota. No parallel worker exists during initial setup.
    budget.statistics.work += attempt.work;
    let mut statistics = SupportStatistics {
        status: SupportStatus::NotApplicable,
        construction_work: attempt.work,
        encoding_work: 0,
    };
    match attempt.result {
        Ok(Some(restriction)) => {
            let before = budget.statistics.work;
            let encoded = encoding::restrict(cnf, &restriction, budget);
            statistics.encoding_work = budget.statistics.work - before;
            statistics.status = match encoded {
                Ok(()) => SupportStatus::Applied,
                Err(Incomplete::Admission(error @ AdmissionError::Limit { .. })) => {
                    SupportStatus::EncodingLimit(error)
                }
                Err(error) => return Err(error),
            };
        }
        Ok(None) => {}
        Err(SupportError::Admission(zetesis_ferraris::AdmissionError::Limit)) => {
            statistics.status = SupportStatus::FormulaLimit;
        }
        Err(SupportError::Admission(zetesis_ferraris::AdmissionError::Allocation)) => {
            return Err(Incomplete::Allocation);
        }
        Err(SupportError::Admission(_)) => return Err(Incomplete::InvalidWitness),
        Err(SupportError::Stopped(zetesis_cpu::Stop::WorkLimit)) => {
            return Err(Incomplete::WorkLimit);
        }
        Err(SupportError::Stopped(stop)) => return Err(stop.into()),
    }
    Ok(statistics)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Control, SearchStatistics};
    use zetesis_ferraris::Node;

    fn input() -> Theory {
        Theory::new(
            2,
            vec![Node::Atom(0), Node::Atom(1), Node::Or(0, 1)],
            vec![2],
            zetesis_ferraris::AdmissionLimits::default(),
        )
        .unwrap()
    }

    fn budget(control: &Control) -> Budget<'_> {
        Budget {
            quota: crate::search::LocalQuota,
            limits: Limits::default().search,
            control,
            statistics: SearchStatistics::default(),
        }
    }

    #[test]
    fn every_interrupted_support_attempt_keeps_spent_work() {
        let theory = input();
        let control = Control::default();
        let limits = Limits::default();
        let mut complete = budget(&control);
        let mut cnf = encoding::encode(&theory, None, limits.admission, &mut complete).unwrap();
        let before = complete.statistics.work;
        let clauses: Vec<Vec<_>> = cnf
            .clauses()
            .map(|clause| clause.iter().collect())
            .collect();
        let variables = cnf.variables();
        let statistics = restrict(&mut cnf, &theory, limits, &mut complete).unwrap();
        assert_eq!(statistics.status, SupportStatus::Applied);
        assert_eq!(
            complete.statistics.work - before,
            statistics.construction_work + statistics.encoding_work
        );
        // Fail at every construction/encoding step. The coordinator records
        // exactly its inclusive quota even when construction cannot return a
        // formula or the appended encoding is rolled back.
        for max_work in before..complete.statistics.work {
            let mut interrupted = budget(&control);
            let mut cnf =
                encoding::encode(&theory, None, limits.admission, &mut interrupted).unwrap();
            interrupted.limits.max_work = max_work;
            assert_eq!(
                restrict(&mut cnf, &theory, limits, &mut interrupted),
                Err(Incomplete::WorkLimit)
            );
            assert_eq!(interrupted.statistics.work, max_work);
            assert_eq!(cnf.variables(), variables);
            assert!(
                cnf.clauses()
                    .map(|clause| clause.iter().collect::<Vec<_>>())
                    .eq(clauses.iter().cloned())
            );
        }
    }

    #[test]
    fn cancelled_support_attempt_cannot_fall_back() {
        let theory = input();
        let control = Control::default();
        let limits = Limits::default();
        let mut budget = budget(&control);
        let mut cnf = encoding::encode(&theory, None, limits.admission, &mut budget).unwrap();
        let before = budget.statistics;
        control.cancel();
        assert_eq!(
            restrict(&mut cnf, &theory, limits, &mut budget),
            Err(Incomplete::Cancelled)
        );
        assert_eq!(budget.statistics, before);
    }
}
