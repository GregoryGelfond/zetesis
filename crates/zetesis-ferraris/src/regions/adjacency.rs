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
        let length = rows.checked_add(1).ok_or(Stop::Allocation)?;
        let mut offsets = zeros(length)?;
        for (row, _) in edges.clone() {
            let after = row.checked_add(1).ok_or(Stop::Allocation)?;
            let count = offsets.get_mut(after).ok_or(Stop::Allocation)?;
            *count = count.checked_add(1).ok_or(Stop::Allocation)?;
        }
        for row in 0..rows {
            offsets[row + 1] = offsets[row + 1]
                .checked_add(offsets[row])
                .ok_or(Stop::Allocation)?;
        }
        let mut entries = zeros(offsets[rows])?;
        let mut cursors = Vec::new();
        cursors
            .try_reserve_exact(rows)
            .map_err(|_| Stop::Allocation)?;
        cursors.extend_from_slice(&offsets[..rows]);
        for (row, entry) in edges {
            let cursor = cursors.get_mut(row).ok_or(Stop::Allocation)?;
            if *cursor >= offsets[row + 1] {
                return Err(Stop::Allocation);
            }
            entries[*cursor] = entry;
            *cursor += 1;
        }
        if cursors
            .iter()
            .zip(&offsets[1..])
            .any(|(cursor, end)| cursor != end)
        {
            return Err(Stop::Allocation);
        }
        Ok(Self { offsets, entries })
    }

    pub(super) fn entry_count(&self) -> usize {
        self.entries.len()
    }

    pub(super) fn len(&self) -> usize {
        self.offsets.len() - 1
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = &[usize]> {
        self.offsets
            .windows(2)
            .map(|range| &self.entries[range[0]..range[1]])
    }

    #[cfg(test)]
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
