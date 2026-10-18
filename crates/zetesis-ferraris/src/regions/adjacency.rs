//! Immutable ordered incidence rows, stored as offsets into one entry vector.
//!
//! Both passes read the same finite edge stream. Counting determines disjoint
//! row intervals; scattering appends to each interval in the stream's order.
//! Neither equal entries nor equal rows are coalesced. Row lookup is constant
//! time and borrows exactly the original ordered occurrences.

use std::ops::Index;

use zetesis_cpu::Stop;

#[derive(Clone, Debug)]
pub(super) struct Adjacency {
    offsets: Vec<usize>,
    entries: Vec<usize>,
}

impl Adjacency {
    /// Count, prefix-sum and scatter an immutable, repeatable edge stream.
    /// Placement takes O(rows + edges) work, plus the cost of enumerating the
    /// supplied stream twice. There are also linear initialization/row scans.
    /// Retained storage is rows + 1 offsets and one entry per occurrence;
    /// construction additionally holds one cursor per row. All reservations
    /// and offset arithmetic are checked before the compact value is returned.
    pub(super) fn build(
        rows: usize,
        edges: impl Iterator<Item = (usize, usize)> + Clone,
    ) -> Result<Self, Stop> {
        Self::try_build(rows, edges.map(Ok))
    }

    /// Build the same ordered rows from fallible reads. Either pass preserves
    /// the original read failure and publishes no partial adjacency.
    pub(super) fn try_build(
        rows: usize,
        edges: impl Iterator<Item = Result<(usize, usize), Stop>> + Clone,
    ) -> Result<Self, Stop> {
        Self::try_build_in(rows, edges.map(|edge| edge.map(Some)), &mut Unmetered)
    }

    /// The same construction under a caller's storage and work authority.
    /// Empty events charge inspection of an input with no incidence. Every
    /// event, initialized cell and cursor inspection has a bounded charge.
    pub(super) fn try_build_in(
        rows: usize,
        edges: impl Iterator<Item = Result<Option<(usize, usize)>, Stop>> + Clone,
        construction: &mut impl Construction,
    ) -> Result<Self, Stop> {
        let length = rows.checked_add(1).ok_or(Stop::Allocation)?;
        let mut offsets = construction.zeros(length)?;
        for edge in edges.clone() {
            construction.tick()?;
            let Some((row, _)) = edge? else { continue };
            let after = row.checked_add(1).ok_or(Stop::Allocation)?;
            let count = offsets.get_mut(after).ok_or(Stop::Allocation)?;
            *count = count.checked_add(1).ok_or(Stop::Allocation)?;
        }
        for row in 0..rows {
            construction.tick()?;
            offsets[row + 1] = offsets[row + 1]
                .checked_add(offsets[row])
                .ok_or(Stop::Allocation)?;
        }
        let mut entries = construction.zeros(offsets[rows])?;
        let mut cursors = construction.reserve(rows)?;
        for &offset in &offsets[..rows] {
            construction.tick()?;
            cursors.push(offset);
        }
        for edge in edges {
            construction.tick()?;
            let Some((row, entry)) = edge? else { continue };
            let cursor = cursors.get_mut(row).ok_or(Stop::Allocation)?;
            if *cursor >= offsets[row + 1] {
                return Err(Stop::Allocation);
            }
            entries[*cursor] = entry;
            *cursor += 1;
        }
        for (cursor, end) in cursors.iter().zip(&offsets[1..]) {
            construction.tick()?;
            if cursor != end {
                return Err(Stop::Allocation);
            }
        }
        construction.release(cursors)?;
        Ok(Self { offsets, entries })
    }

    pub(super) fn release_in(self, construction: &mut impl Construction) -> Result<(), Stop> {
        construction.release(self.offsets)?;
        construction.release(self.entries)
    }

    pub(super) fn len(&self) -> usize {
        self.offsets.len() - 1
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = &[usize]> {
        self.offsets
            .windows(2)
            .map(|range| &self.entries[range[0]..range[1]])
    }

    /// Owned header and actual vector capacity, without allocator metadata.
    pub(super) fn retained_bytes(&self) -> u128 {
        std::mem::size_of::<Self>() as u128
            + (self.offsets.capacity() as u128 + self.entries.capacity() as u128)
                * std::mem::size_of::<usize>() as u128
    }
}

impl Index<usize> for Adjacency {
    type Output = [usize];

    fn index(&self, row: usize) -> &Self::Output {
        &self.entries[self.offsets[row]..self.offsets[row + 1]]
    }
}

fn zeros(length: usize) -> Result<Vec<usize>, Stop> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(length)
        .map_err(|_| Stop::Allocation)?;
    values.resize(length, 0);
    Ok(values)
}

#[cfg(test)]
mod tests;

/// The caller owns construction admission and every retained allocation.
pub(super) trait Construction {
    fn tick(&mut self) -> Result<(), Stop>;
    fn zeros(&mut self, count: usize) -> Result<Vec<usize>, Stop>;
    fn reserve(&mut self, count: usize) -> Result<Vec<usize>, Stop>;
    fn release(&mut self, values: Vec<usize>) -> Result<(), Stop>;
}

struct Unmetered;

impl Construction for Unmetered {
    fn tick(&mut self) -> Result<(), Stop> {
        Ok(())
    }
    fn zeros(&mut self, count: usize) -> Result<Vec<usize>, Stop> {
        zeros(count)
    }
    fn reserve(&mut self, count: usize) -> Result<Vec<usize>, Stop> {
        let mut values = Vec::new();
        values
            .try_reserve_exact(count)
            .map_err(|_| Stop::Allocation)?;
        Ok(values)
    }
    fn release(&mut self, values: Vec<usize>) -> Result<(), Stop> {
        drop(values);
        Ok(())
    }
}
