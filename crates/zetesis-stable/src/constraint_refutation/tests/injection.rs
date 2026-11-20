//! Test-only source injections retain worker lifetime and teardown behavior.
//! Production uses the concrete admitted `StreamedConstraints` owner.

use std::fmt::Debug;

use zetesis_algebra::region::Region;
use zetesis_ferraris::Theory;
use zetesis_themelios::ProgramSite;

use crate::streamed_constraints::ConstraintConsequence;
use crate::{Cancellation, Incomplete, RegionFeasibility, subject::Pass};

pub(crate) trait Source: Debug + Send + Sync {
    fn worker(
        &self,
        theory: &Theory,
        cancellation: &Cancellation,
    ) -> Result<Box<dyn Worker + '_>, Incomplete>;
}

pub(crate) trait Worker: Send {
    fn check(
        &mut self,
        theory: &Theory,
        region: &Region,
        cancellation: &Cancellation,
    ) -> Result<RegionFeasibility, Incomplete>;

    fn consequence(
        &mut self,
        theory: &Theory,
        region: &Region,
        cancellation: &Cancellation,
        _pass: Pass,
    ) -> Result<ConstraintConsequence, Incomplete> {
        self.check(theory, region, cancellation)
            .map(|verdict| match verdict {
                RegionFeasibility::NotRefuted => ConstraintConsequence::NoConsequence,
                RegionFeasibility::Refuted => ConstraintConsequence::Refuted {
                    site: ProgramSite::program(),
                },
            })
    }
}
