//! Helpers the crate's unit tests share.

use zetesis_algebra::{Cancellation, Stop};

use crate::{
    AdmissionLimits, FormulaNodes, FormulaParts, FormulaTransaction, Interpretation, Knowledge,
    LearningLimits, LearningOwner, Narrower, Narrowing, NarrowingScratch, NarrowingStatistics,
    Node, NodeView, OriginalSubject, Region, RegionLimits, Theory,
};

/// The interpretation of `theory` holding the atoms whose bits `mask` sets.
pub(crate) fn world(theory: &Theory, mask: usize) -> Interpretation {
    Interpretation::new(
        theory,
        (0..theory.atom_count()).filter(|atom| mask & (1 << atom) != 0),
    )
    .unwrap()
}

/// The theory over `atoms` atoms with the formula nodes `nodes` and the roots
/// `roots`, under the default admission limits.
pub(crate) fn theory(atoms: usize, nodes: Vec<Node>, roots: Vec<usize>) -> Theory {
    Theory::new(
        atoms,
        FormulaParts::new(nodes, vec![]).unwrap(),
        roots,
        AdmissionLimits::default(),
    )
    .unwrap()
}

/// The owner of rows of `theory`'s answer sets under the default limits, for
/// a test's carrier subject.
pub(crate) fn learning(theory: &Theory) -> LearningOwner {
    LearningOwner::theory(theory, LearningLimits::default())
}

/// Push `node` into `transaction` with unbounded allowances, returning its
/// index.
pub(crate) fn push(transaction: &mut FormulaTransaction<'_>, node: NodeView<'_>) -> usize {
    transaction.push(node, usize::MAX, usize::MAX).unwrap()
}

/// Push `node` into `owner` in a transaction of its own, with unbounded
/// allowances, returning its index.
pub(crate) fn push_committed(owner: &mut FormulaNodes, node: NodeView<'_>) -> usize {
    let mut transaction = owner.transaction();
    let index = push(&mut transaction, node);
    transaction.commit();
    index
}

/// The theory over `atoms` atoms whose first nodes are those atoms and whose
/// further nodes and roots `construct` builds, under the default admission
/// limits.
pub(crate) fn constructed_theory(
    atoms: usize,
    construct: impl FnOnce(&mut FormulaTransaction<'_>) -> Vec<usize>,
) -> Theory {
    let mut nodes = FormulaNodes::default();
    let mut transaction = nodes.transaction();
    for atom in 0..atoms {
        push(&mut transaction, NodeView::Atom(atom));
    }
    let roots = construct(&mut transaction);
    transaction.commit();
    Theory::new(atoms, nodes.into_parts(), roots, AdmissionLimits::default()).unwrap()
}

/// A narrowing quota that grants at most `remaining` units in all and then
/// refuses with `failure`.
pub(crate) struct Allowance {
    pub(crate) remaining: u64,
    pub(crate) failure: Stop,
}

impl zetesis_algebra::Quota for Allowance {
    type Error = Stop;
    fn reserve(&mut self, wanted: u64) -> Result<u64, Stop> {
        let granted = wanted.min(self.remaining);
        if granted == 0 {
            return Err(self.failure);
        }
        self.remaining -= granted;
        Ok(granted)
    }

    fn refund(&mut self, unspent: u64) {
        self.remaining += unspent;
    }
}

/// Close `region` by `index`'s known narrowing of `theory`'s original
/// subject, under the default region limits.
pub(crate) fn close(
    theory: &Theory,
    index: &Narrower,
    region: &mut Region,
    knowledge: &mut Knowledge,
    scratch: &mut NarrowingScratch,
) -> (Narrowing, NarrowingStatistics) {
    let attempt = index.narrow_known(
        OriginalSubject::new(theory, None),
        region,
        knowledge,
        scratch,
        RegionLimits::default(),
        &Cancellation::default(),
    );
    (attempt.result.unwrap(), attempt.statistics)
}

/// A narrowing quota that grants one permit per request, `remaining` in all,
/// then refuses with `failure`, counting its requests in `calls`: single
/// permits expose the exact read at which a refusal is observed.
pub(crate) struct SinglePermits {
    pub(crate) remaining: u64,
    pub(crate) failure: Stop,
    pub(crate) calls: u64,
}

impl SinglePermits {
    /// A quota of `remaining` single permits refusing with `failure`.
    pub(crate) fn new(remaining: u64, failure: Stop) -> Self {
        Self {
            remaining,
            failure,
            calls: 0,
        }
    }
}

impl zetesis_algebra::Quota for SinglePermits {
    type Error = Stop;
    fn reserve(&mut self, _wanted: u64) -> Result<u64, Stop> {
        self.calls += 1;
        if self.remaining == 0 {
            return Err(self.failure);
        }
        self.remaining -= 1;
        Ok(1)
    }

    fn refund(&mut self, unspent: u64) {
        self.remaining += unspent;
    }
}

/// Compare the historical Stop-valued local and reserved narrowing traces.
/// Typed-refusal tests inspect the unconverted public attempt separately.
pub(crate) fn stop_narrowing(
    attempt: &crate::NarrowingAttempt<zetesis_algebra::Failure<Stop>>,
) -> crate::NarrowingAttempt {
    crate::NarrowingAttempt {
        result: attempt.result.map_err(zetesis_algebra::Failure::into_stop),
        statistics: attempt.statistics,
        preferred: attempt.preferred,
    }
}

/// Observe a Stop-valued quota preference beside the local reference trace.
pub(crate) fn stop_preference(
    attempt: &crate::PreferenceAttempt<zetesis_algebra::Failure<Stop>>,
) -> crate::PreferenceAttempt {
    crate::PreferenceAttempt {
        result: attempt.result.map_err(zetesis_algebra::Failure::into_stop),
        work: attempt.work,
    }
}

/// Observe the original sharing trace under a Stop-valued account.
pub(crate) fn stop_learning(
    attempt: &crate::LearningAttempt<zetesis_algebra::Failure<Stop>>,
) -> crate::LearningAttempt {
    crate::LearningAttempt {
        result: attempt.result.map_err(zetesis_algebra::Failure::into_stop),
        work: attempt.work,
    }
}

/// Keep storage payloads intact when comparing local and reserved preparation.
pub(crate) fn stop_generator(
    attempt: crate::GeneratorOrderAttempt<zetesis_algebra::Failure<Stop>>,
) -> crate::GeneratorOrderAttempt {
    crate::GeneratorOrderAttempt {
        result: attempt.result.map_err(|error| match error {
            crate::GeneratorOrderError::Storage { required, limit } => {
                crate::GeneratorOrderError::Storage { required, limit }
            }
            crate::GeneratorOrderError::Stopped(failure) => {
                crate::GeneratorOrderError::Stopped(failure.into_stop())
            }
        }),
        work: attempt.work,
        peak_bytes: attempt.peak_bytes,
    }
}
