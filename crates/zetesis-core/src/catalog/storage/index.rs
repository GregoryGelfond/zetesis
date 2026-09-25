//! Collision-exact candidate chains. Only IDs, links and hash filters are owned;
//! equality is always checked against the authoritative storage by the caller.

use std::{collections::HashMap, mem::size_of};

use super::{Failure, Fault, budget::Budget, control::Work};

#[derive(Debug, Default)]
pub(super) struct Index {
    heads: HashMap<u64, u32>,
    next: Vec<Option<u32>>,
}

impl Index {
    pub(super) fn find_with<E>(
        &self,
        hash: u64,
        work: &mut Work<'_, E>,
        mut equal: impl FnMut(u32, &mut Work<'_, E>) -> Result<bool, Failure<E>>,
    ) -> Result<Option<u32>, Failure<E>> {
        work.step()?;
        let mut candidate = self.heads.get(&hash).copied();
        while let Some(id) = candidate {
            work.step()?;
            if equal(id, work)? {
                return Ok(Some(id));
            }
            work.step()?;
            candidate = self.next[id as usize];
        }
        Ok(None)
    }

    pub(super) fn reserve_with<E>(
        &mut self,
        hash: u64,
        budget: &mut Budget,
        work: &mut Work<'_, E>,
    ) -> Result<(), Failure<E>> {
        work.reserve(&mut self.next, 1, budget)?;
        work.step()?;
        if !self.heads.contains_key(&hash) && self.heads.len() == self.heads.capacity() {
            let before = self.bytes();
            let count = self.heads.len().checked_add(1).ok_or(Fault::Overflow)?;
            budget.check_extra(count as u128 * size_of::<(u64, u32)>() as u128)?;
            work.steps(self.heads.len())?;
            work.step()?;
            self.heads.try_reserve(1).map_err(|_| Fault::Allocation)?;
            budget.observed(before, self.bytes())?;
        }
        Ok(())
    }

    // Caller has reserved both components and publishes IDs consecutively.
    pub(super) fn insert(&mut self, hash: u64, id: u32) {
        debug_assert_eq!(self.next.len(), id as usize);
        let previous = self.heads.insert(hash, id);
        self.next.push(previous);
    }

    pub(super) fn buffer_bytes(&self) -> u128 {
        self.bytes() + super::budget::capacity(&self.next)
    }

    fn bytes(&self) -> u128 {
        // HashMap does not expose bucket/control allocation layout. This is
        // addressable key/value capacity, not an assertion about allocator bytes.
        self.heads.capacity() as u128 * size_of::<(u64, u32)>() as u128
    }

    #[cfg(test)]
    pub(super) fn find(&self, hash: u64, mut equal: impl FnMut(u32) -> bool) -> Option<u32> {
        let mut before = || Ok::<(), std::convert::Infallible>(());
        super::control::uncontrolled(
            self.find_with(hash, &mut Work::new(&mut before), |id, _| Ok(equal(id))),
        )
        .expect("lookup has no storage refusal")
    }

    #[cfg(test)]
    pub(super) fn reserve(&mut self, hash: u64, budget: &mut Budget) -> Result<(), Fault> {
        let mut before = || Ok::<(), std::convert::Infallible>(());
        super::control::uncontrolled(self.reserve_with(hash, budget, &mut Work::new(&mut before)))
    }
}
