//! Typed discovery order is sorted on demand; the interner keeps no ordered index.

use zetesis_algebra::words::Words;

use super::{AtomAppender, Failure, Limits, admit, cells, population, reserve};

impl AtomAppender<'_> {
    /// Prospective total named storage for dense selected ordering.
    ///
    /// Returns `None` when the strategy is inapplicable: for discovery population
    /// n and selected count d, it requires 2 <= d <= n <= 2*d. This keeps the
    /// n-bit duplicate mask proportional to the selection. It does not inspect
    /// the selected positions or establish their validity.
    ///
    /// Includes this owner's current storage, an n-bit mask, two d-position
    /// merge buffers, a run-start buffer and their headers. The caller's selected slice and other
    /// live owners are excluded; compare the result with the allowance remaining
    /// for this owner. The estimate uses requested capacities. Actual allocator
    /// slack is checked by [`Self::order_selected_with`] and can still cause a
    /// typed refusal. This read neither allocates nor changes a peak receipt;
    /// it is not evidence that the operation completed.
    #[must_use]
    pub fn selected_order_storage(&self, selected: usize) -> Option<u128> {
        let count = self.len();
        if selected < 2 || selected > count || count > selected.saturating_mul(2) {
            return None;
        }
        Some(
            self.storage_bytes()
                + size_of::<Words<u64>>() as u128
                + cells::<u64>(count.div_ceil(64))
                + 3 * size_of::<Vec<usize>>() as u128
                + 3 * cells::<usize>(selected)
                + cells::<usize>(1),
        )
    }

    /// Replace distinct discovery positions by their exact typed atom order.
    ///
    /// Positions may select any committed or pending discoveries, including old
    /// identities that a caller has not previously selected. Canonical rows with
    /// no discovery position are outside this operation. An n-bit mask rejects
    /// duplicates; the typed atom comparison then orders the d selected
    /// positions by a merge sort over two d-position buffers. No atom or term
    /// payload is copied.
    ///
    /// All replacement writes receive permits before any caller position changes.
    /// On refusal or a callback panic the caller slice is unchanged. The peak
    /// includes actual temporary buffers and growth overlap even when they are
    /// dropped on failure/unwind. Empty and singleton selections need no
    /// temporary buffers.
    ///
    /// # Errors
    /// Rejects duplicate or out-of-range positions as a canonical shape error,
    /// and propagates population, storage, allocation and caller-work refusals.
    /// [`Self::selected_order_storage`] is an optional strategy preflight; once
    /// chosen, a refusal is not a signal to silently retry another algorithm.
    pub fn order_selected_with<E>(
        &mut self,
        selected: &mut [usize],
        limits: Limits,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<(), Failure<E>> {
        population(self.len(), limits)?;
        admit(self.storage_bytes(), limits)?;
        if selected.len() > self.len() {
            return Err(shape());
        }
        if selected.len() < 2 {
            if !selected.is_empty() {
                before().map_err(Failure::Stopped)?;
                if selected[0] >= self.len() {
                    return Err(shape());
                }
            }
            return Ok(());
        }
        let mask = self.selection_mask(selected, limits, &mut before)?;
        let extra = size_of::<Words<u64>>() as u128 + cells::<u64>(mask.capacity());
        let output = self.sort_positions(selected, extra, limits, &mut before)?;
        drop(mask);
        for _ in 0..selected.len() {
            before().map_err(Failure::Stopped)?;
        }
        selected.copy_from_slice(&output);
        Ok(())
    }

    fn selection_mask<E>(
        &mut self,
        selected: &[usize],
        limits: Limits,
        before: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<Words<u64>, Failure<E>> {
        let mut checked = || before().map_err(Failure::Stopped);
        let mut mask = Vec::new();
        let words = self.len().div_ceil(64);
        let live = self.storage_bytes() + size_of::<Words<u64>>() as u128;
        reserve(
            &mut mask,
            words,
            words,
            live,
            self.peak,
            limits,
            &mut checked,
        )?;
        for _ in 0..words {
            checked()?;
            mask.push(0_u64);
        }
        let mut mask = Words::<u64>::new(mask);
        for position in selected {
            checked()?;
            let position = *position;
            if position >= self.len() {
                return Err(shape());
            }
            if mask.contains(position) {
                return Err(shape());
            }
            checked()?;
            mask.insert(position);
        }
        Ok(mask)
    }

    pub(super) fn ordered_prefix_with<E>(
        &mut self,
        count: usize,
        limits: Limits,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<Vec<usize>, Failure<E>> {
        population(self.len(), limits)?;
        let mut checked = || before().map_err(Failure::Stopped);
        let mut output = Vec::new();
        let live = self.storage_bytes() + size_of::<Vec<usize>>() as u128;
        reserve(
            &mut output,
            count,
            count,
            live,
            self.peak,
            limits,
            &mut checked,
        )?;
        for position in 0..count {
            checked()?;
            output.push(position);
        }
        let extra = size_of::<Vec<usize>>() as u128 + cells::<usize>(output.capacity());
        let sorted = self.sort_positions(&output, extra, limits, &mut before)?;
        Ok(sorted)
    }

    /// Return `positions` in exact typed atom order without changing them.
    ///
    /// A natural merge sort: one pass splits the positions into maximal
    /// ascending runs, then adjacent runs merge pairwise until one remains.
    /// Discoveries mostly arrive in typed order, so a sorted selection costs
    /// n - 1 comparisons and r runs cost at most n * ceil(log2 r) more. Three
    /// admitted buffers (positions, merge output and run starts); one permit
    /// per copied position, one per run merge and one per comparison, whose
    /// typed comparator admits its own navigation. Refusal drops the buffers
    /// after their actual capacities enter the operation's peak.
    fn sort_positions<E>(
        &mut self,
        positions: &[usize],
        extra: u128,
        limits: Limits,
        before: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<Vec<usize>, Failure<E>> {
        let count = positions.len();
        let mut checked = || before().map_err(Failure::Stopped);
        let mut live = self.storage_bytes() + extra + 3 * size_of::<Vec<usize>>() as u128;
        let mut values = Vec::new();
        let mut scratch = Vec::new();
        let mut runs = Vec::new();
        for (buffer, cells_needed) in [
            (&mut values, count),
            (&mut scratch, count),
            (&mut runs, count + 1),
        ] {
            reserve(
                buffer,
                cells_needed,
                cells_needed,
                live,
                self.peak,
                limits,
                &mut checked,
            )?;
            live += cells::<usize>(buffer.capacity());
        }
        let compare =
            |left: usize, right: usize, checked: &mut dyn FnMut() -> Result<(), Failure<E>>| {
                checked()?;
                self.get(left)
                    .expect("selected discovery position")
                    .compare_ref_with(
                        self.get(right).expect("selected discovery position"),
                        &mut *checked,
                    )
            };
        runs.push(0);
        for (index, &position) in positions.iter().enumerate() {
            checked()?;
            if let Some(&previous) = values.last()
                && compare(position, previous, &mut checked)?.is_lt()
            {
                runs.push(index);
            }
            values.push(position);
        }
        runs.push(count);
        while runs.len() > 2 {
            scratch.clear();
            let mut kept = 0;
            let mut run = 0;
            while run + 1 < runs.len() {
                checked()?;
                let (start, middle) = (runs[run], runs[run + 1]);
                let end = runs.get(run + 2).copied().unwrap_or(middle);
                let (mut left, mut right) = (start, middle);
                while left < middle && right < end {
                    if compare(values[right], values[left], &mut checked)?.is_lt() {
                        scratch.push(values[right]);
                        right += 1;
                    } else {
                        scratch.push(values[left]);
                        left += 1;
                    }
                }
                scratch.extend_from_slice(&values[left..middle]);
                scratch.extend_from_slice(&values[right..end]);
                runs[kept] = start;
                kept += 1;
                run += 2;
            }
            runs[kept] = count;
            runs.truncate(kept + 1);
            std::mem::swap(&mut values, &mut scratch);
        }
        Ok(values)
    }
}

fn shape<E>() -> Failure<E> {
    Failure::Catalog(crate::catalog::Error::Shape)
}
