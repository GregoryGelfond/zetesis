//! Disjoint preparation receipts, including the prefix of a stopped phase.

/// Logical work by preparation phase. Nested phases are counted only in their
/// own field; their enclosing phase excludes them. These are observations of
/// the existing work meter, not additional charges or an allowance of their own.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PreparationWork {
    /// Source decoding, affine hulls and row materialization.
    pub recognition: u64,
    /// Independent source rereads authenticating proposed consequences.
    pub provenance: u64,
    /// Parity elimination and checked cardinality reductions.
    pub reduction: u64,
    /// Bounded discovery of candidate cardinality families.
    pub discovery: u64,
    /// Independent checking of proposed partitions and bound directions.
    pub candidate_checks: u64,
    /// Incidence-index preparation.
    pub index: u64,
    /// Construction and publication of the region seeded by root units.
    pub root_seed: u64,
}

impl PreparationWork {
    /// Total executed preparation work, including any abandoned prefix.
    #[must_use]
    pub const fn total(self) -> u64 {
        self.recognition
            + self.provenance
            + self.reduction
            + self.discovery
            + self.candidate_checks
            + self.index
            + self.root_seed
    }

    pub(super) fn since(self, earlier: Self) -> Self {
        Self {
            recognition: self.recognition - earlier.recognition,
            provenance: self.provenance - earlier.provenance,
            reduction: self.reduction - earlier.reduction,
            discovery: self.discovery - earlier.discovery,
            candidate_checks: self.candidate_checks - earlier.candidate_checks,
            index: self.index - earlier.index,
            root_seed: self.root_seed - earlier.root_seed,
        }
    }

    pub(super) fn record(&mut self, phase: Phase, work: u64) {
        let field = match phase {
            Phase::Recognition => &mut self.recognition,
            Phase::Provenance => &mut self.provenance,
            Phase::Reduction => &mut self.reduction,
            Phase::Discovery => &mut self.discovery,
            Phase::CandidateChecks => &mut self.candidate_checks,
            Phase::Index => &mut self.index,
            Phase::RootSeed => &mut self.root_seed,
        };
        *field += work;
    }
}

#[derive(Clone, Copy)]
pub(super) enum Phase {
    Recognition,
    Provenance,
    Reduction,
    Discovery,
    CandidateChecks,
    Index,
    RootSeed,
}
