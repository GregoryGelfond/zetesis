//! One resource and cancellation account shared by evaluation and rendering.
//! This account retains neither a query program nor a term authority.

use zetesis_algebra::{Meter, MeterWorkError};

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
    pub(in crate::observation) fn error(&self, kind: ErrorKind) -> Error {
        Error::new(kind, self.site, self.statistics)
    }

    pub(in crate::observation) fn check(
        &self,
        resource: Resource,
        observed: u128,
        limit: u128,
    ) -> Result<(), Error> {
        Meter::check_wide_count(observed, 0, limit)
            .map(|_| ())
            .map_err(|_| {
                self.error(ErrorKind::Limit {
                    resource,
                    observed,
                    limit,
                })
            })
    }

    pub(in crate::observation) fn step(&mut self, count: u128) -> Result<(), Error> {
        self.cancellation
            .poll()
            .map_err(|stop| self.error(ErrorKind::Stopped(stop)))?;
        match Meter::charge_receipt(&mut self.statistics.work, count, self.limits.max_work) {
            Ok(()) => Ok(()),
            Err(MeterWorkError::Limit { observed, limit }) => Err(self.error(ErrorKind::Limit {
                resource: Resource::Work,
                observed,
                limit,
            })),
            Err(MeterWorkError::Overflow) => {
                unreachable!("observation work producers fit u128 with the accepted prefix")
            }
        }
    }

    pub(in crate::observation) fn reserve<T>(&self, count: usize) -> Result<Vec<T>, Error> {
        let mut values = Vec::new();
        values
            .try_reserve_exact(count)
            .map_err(|_| self.error(ErrorKind::Allocation))?;
        Ok(values)
    }
}

#[cfg(test)]
mod tests;
