//! Monotone producer searches inherited with original-region knowledge.
//!
//! A producer becomes unavailable when its body is known false or another
//! ordinary head is held. Neither condition can disappear in a descendant.
//! Two positions retain exactly that eliminated coverage: everything before
//! `first`, and strictly between `first` and `second`, is unavailable. Endpoints
//! are rechecked against current knowledge; an open head needs one witness, a
//! held head two. When `first` fails it can jump to `second`. Reaching the end
//! proves zero or sole support only after every omitted position was checked.
//!
//! The positions are part of Knowledge, never worker scratch. Independent
//! clones preserve sibling isolation; `clone_from` restores the source's complete
//! state even after a refused destination. Subject/ancestor authority is the
//! same precondition as the inherited truth masks. No frozen subject uses this
//! operation and a competing held head never invalidates an atomic choice.
//!
//! Rows of at most two producers keep the stateless scan. The first longer-row
//! recheck fallibly prepares one vector for the atom universe, charged before
//! publication. Two u16 positions suffice when the complete producer count fits;
//! otherwise u32 or usize retains the same algorithm. Thus typical payload is
//! four bytes per atom, including unused short rows, with no sparse lookup map.
//! Initialization is O(A); across K rechecks of a head with R producers, at most
//! O(K + R) producer rows are read, each with its existing head-mask checks.
//! Clone cost includes the retained vector; its actual capacity is included in
//! `Knowledge::retained_bytes`.
//!
//! An interrupted scan may retain proved unavailable prefixes but derives no
//! truth. Only a completed outcome may cut a head or force its sole body. The
//! caller still abandons interrupted closure knowledge and pending work.

use super::{Producers, Work, bit, counters::Count};
use zetesis_cpu::Stop;

#[cfg(test)]
mod tests;

/// A completed support search, not a membership verdict.
pub(super) enum Support {
    Unavailable,
    Sole(Option<usize>),
    Sufficient,
}

/// The complete row population bounds both positions, including its end.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) enum Progress {
    #[default]
    Vacant,
    Small(Vec<[u16; 2]>),
    Medium(Vec<[u32; 2]>),
    Wide(Vec<[usize; 2]>),
}

impl Clone for Progress {
    fn clone(&self) -> Self {
        match self {
            Self::Vacant => Self::Vacant,
            Self::Small(rows) => Self::Small(rows.clone()),
            Self::Medium(rows) => Self::Medium(rows.clone()),
            Self::Wide(rows) => Self::Wide(rows.clone()),
        }
    }

    fn clone_from(&mut self, source: &Self) {
        match (self, source) {
            (Self::Small(rows), Self::Small(source)) => rows.clone_from(source),
            (Self::Medium(rows), Self::Medium(source)) => rows.clone_from(source),
            (Self::Wide(rows), Self::Wide(source)) => rows.clone_from(source),
            (destination, source) => *destination = source.clone(),
        }
    }
}

impl Progress {
    pub(super) fn allocated_bytes(&self) -> u128 {
        match self {
            Self::Vacant => 0,
            Self::Small(rows) => rows.capacity() as u128 * size_of::<[u16; 2]>() as u128,
            Self::Medium(rows) => rows.capacity() as u128 * size_of::<[u32; 2]>() as u128,
            Self::Wide(rows) => rows.capacity() as u128 * size_of::<[usize; 2]>() as u128,
        }
    }

    fn prepare(&mut self, producers: &Producers, work: &mut Work<'_>) -> Result<(), Stop> {
        if matches!(self, Self::Vacant) {
            let atoms = producers.by_head.len();
            let bound = producers.rules.len();
            *self = if u16::try_from(bound).is_ok() && size_of::<u16>() < size_of::<usize>() {
                Self::Small(zeros(atoms, work)?)
            } else if u32::try_from(bound).is_ok() && size_of::<u32>() < size_of::<usize>() {
                Self::Medium(zeros(atoms, work)?)
            } else {
                Self::Wide(zeros(atoms, work)?)
            };
        }
        Ok(())
    }

    pub(super) fn check(
        &mut self,
        producers: &Producers,
        atom: usize,
        held: bool,
        never: &[u64],
        atom_sure: &[u64],
        work: &mut Work<'_>,
    ) -> Result<Support, Stop> {
        let rows = &producers.by_head[atom];
        if rows.len() > 2 {
            self.prepare(producers, work)?;
        }
        let mut possible = |position: usize| {
            work.tick()?;
            let producer = &producers.rules[rows[position]];
            let body_impossible = producer.body.is_some_and(|body| bit(never, body));
            let other_held = !producer.choice
                && producer
                    .heads
                    .iter()
                    .any(|&head| head != atom && bit(atom_sure, head));
            Ok(!body_impossible && !other_held)
        };
        // A fresh two-position scan has the same early exits as the ordinary
        // loop. Short rows need no retained state or preparation allocation.
        let sole = if rows.len() <= 2 {
            scan(&mut [0_usize; 2], rows.len(), held, &mut possible)?
        } else {
            match self {
                Self::Small(state) => scan(&mut state[atom], rows.len(), held, &mut possible)?,
                Self::Medium(state) => scan(&mut state[atom], rows.len(), held, &mut possible)?,
                Self::Wide(state) => scan(&mut state[atom], rows.len(), held, &mut possible)?,
                Self::Vacant => unreachable!("completed support preparation"),
            }
        };
        Ok(match sole {
            Outcome::None => Support::Unavailable,
            Outcome::Sole(position) => Support::Sole(producers.rules[rows[position]].body),
            Outcome::Enough => Support::Sufficient,
        })
    }
}

fn zeros<C: Count>(atoms: usize, work: &mut Work<'_>) -> Result<Vec<[C; 2]>, Stop> {
    // No allocation occurs after a refused initial work request. Only the
    // completed private vector becomes a source of inherited cursor authority.
    work.tick()?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(atoms)
        .map_err(|_| Stop::Allocation)?;
    for _ in 0..atoms {
        work.tick()?;
        rows.push([C::default(); 2]);
    }
    Ok(rows)
}

#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    None,
    Sole(usize),
    Enough,
}

fn scan<C: Count>(
    positions: &mut [C; 2],
    length: usize,
    held: bool,
    mut possible: impl FnMut(usize) -> Result<bool, Stop>,
) -> Result<Outcome, Stop> {
    let [mut first, mut second] = (*positions).map(Count::get);
    let result = (|| {
        while first < length {
            if possible(first)? {
                if !held {
                    return Ok(Outcome::Enough);
                }
                second = second.max(first + 1);
                while second < length {
                    if possible(second)? {
                        return Ok(Outcome::Enough);
                    }
                    second += 1;
                }
                return Ok(Outcome::Sole(first));
            }
            // Only the endpoints may remain possible; the intervening prefix
            // was eliminated under this knowledge or one of its ancestors.
            first = second.max(first + 1);
            second = first;
        }
        Ok(Outcome::None)
    })();
    // A stopped producer read leaves its own position unread. Earlier completed
    // failures may be retained, but no completed support outcome is manufactured.
    *positions = [C::default().plus(first), C::default().plus(second)];
    result
}
