//! Views of completed checks and the authoritative independent query cache.

use serde::{Serialize, Serializer, ser::SerializeStruct};
use zetesis_cpu::{Check, PreparationStatistics, QueryStatistics};

/// Completed independent candidate work, aggregated after the sample timer.
///
/// Operation, round, binding and atom counts are sums across the ordered check
/// occurrences, including duplicate seeds and rejected candidates. The named
/// closure peak is the maximum individual candidate envelope, not their sum or
/// a simultaneous batch peak. Work units are not machine instructions.
/// Scalar's one-shot work includes query preparation for each candidate. Rayon
/// records its shared preparation separately in the pool's query snapshot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct IndependentWork {
    /// Sum of each completed candidate's charged operations.
    pub work: u128,
    /// Sum of catalog operations, already included in `work`.
    pub catalog_work: u128,
    /// Sum of completed source rounds, including each final no-delta round.
    /// An empty source needs no rounds.
    pub rounds: u128,
    /// Sum of fully matched enabled/filter-valid bindings across all rounds.
    pub bindings: u128,
    /// Sum of final closure atom counts, preserving occurrence multiplicity.
    pub derived_atoms: u128,
    /// Maximum individual named closure envelope. Excludes returned models and
    /// allocator overhead; not total batch storage, process peak RSS or bytes saved.
    pub peak_closure_bytes: usize,
}

impl IndependentWork {
    pub(in crate::lazy_measurement) fn from_checks(checks: &[Check]) -> Self {
        // Configuration admits at most 256 occurrences. Each source counter is
        // u64 or a target-sized usize, so these u128 sums cannot overflow.
        checks.iter().fold(Self::default(), |mut total, check| {
            let statistics = check.statistics();
            total.work += u128::from(statistics.work);
            total.catalog_work += u128::from(statistics.catalog_work);
            total.rounds += u128::from(statistics.rounds);
            total.bindings += u128::from(statistics.bindings);
            total.derived_atoms += statistics.derived_atoms as u128;
            total.peak_closure_bytes = total.peak_closure_bytes.max(statistics.peak_closure_bytes);
            total
        })
    }
}

#[expect(
    clippy::ref_option,
    reason = "Serde serialize_with passes a reference to the original Option field."
)]
pub(super) fn queries<S: Serializer>(
    statistics: &Option<QueryStatistics>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    statistics.as_ref().map(QueryView).serialize(serializer)
}

struct QueryView<'a>(&'a QueryStatistics);

impl Serialize for QueryView<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let statistics = self.0;
        let mut view = serializer.serialize_struct("QueryStatistics", 7)?;
        view.serialize_field(
            "preparation",
            &statistics.preparation.as_ref().map(PreparationView),
        )?;
        view.serialize_field("preparation_builds", &statistics.preparation_builds)?;
        view.serialize_field("retained_workspaces", &statistics.retained_workspaces)?;
        view.serialize_field("active_workspaces", &statistics.active_workspaces)?;
        view.serialize_field("reused_workspaces", &statistics.reused_workspaces)?;
        view.serialize_field("retained_bytes", &statistics.retained_bytes)?;
        view.serialize_field("reserved_bytes", &statistics.reserved_bytes)?;
        view.end()
    }
}

struct PreparationView<'a>(&'a PreparationStatistics);

impl Serialize for PreparationView<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut view = serializer.serialize_struct("PreparationStatistics", 2)?;
        view.serialize_field("work", &self.0.work)?;
        view.serialize_field("retained_bytes", &self.0.retained_bytes)?;
        view.end()
    }
}
