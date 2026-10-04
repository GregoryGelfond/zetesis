//! One resource and cancellation account shared by evaluation and rendering.
//! This account retains neither a query program nor a term authority.

use crate::ProgramSite;

use super::{Cancellation, ConstructionLimits, Error, ErrorKind, Limits, Resource, Statistics};

pub(in crate::observation) struct Work<'a> {
    pub limits: Limits,
    pub construction: ConstructionLimits,
    pub cancellation: &'a Cancellation,
    pub statistics: Statistics,
    pub site: ProgramSite,
    pub local_bytes: u128,
}

impl Work<'_> {
    pub fn error(&self, kind: ErrorKind) -> Error {
        Error::new(kind, self.site, self.statistics)
    }

    pub fn check(&self, resource: Resource, observed: u128, limit: u128) -> Result<(), Error> {
        if observed > limit {
            return Err(self.error(ErrorKind::Limit {
                resource,
                observed,
                limit,
            }));
        }
        Ok(())
    }

    pub fn step(&mut self, count: u128) -> Result<(), Error> {
        self.cancellation
            .poll()
            .map_err(|stop| self.error(ErrorKind::Stopped(stop)))?;
        let observed = u128::from(self.statistics.work) + count;
        self.check(Resource::Work, observed, u128::from(self.limits.max_work))?;
        self.statistics.work = u64::try_from(observed).expect("work checked against u64 ceiling");
        Ok(())
    }

    pub fn reserve<T>(&self, count: usize) -> Result<Vec<T>, Error> {
        let mut values = Vec::new();
        values
            .try_reserve_exact(count)
            .map_err(|_| self.error(ErrorKind::Allocation))?;
        Ok(values)
    }
}
