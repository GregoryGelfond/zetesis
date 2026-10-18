//! Split ordering over already-maintained unknown-parent counts.

use super::{ChoiceHeads, Knowledge, NarrowingQuota, PreferenceAttempt, Region, Width, Work};
use zetesis_cpu::{Cancellation, Stop};

impl Knowledge {
    fn unknown_count(&self, atom: usize) -> usize {
        match &self.width {
            Width::Compact16(known) => known.unknown.get(atom),
            Width::Compact32(known) => known.unknown.get(atom),
            Width::Native(known) => known.unknown.get(atom),
        }
    }

    /// Select by original choice marks, this subject's unknown-parent count,
    /// then `secondary`'s count; remaining ties select the lowest atom. Both
    /// knowledge values must use the region's atom identities and be closed
    /// for it. This only orders an exhaustive split and changes neither value.
    ///
    /// The scan takes `O(A / 64 + E)` time for `A` atoms and `E` eligible atoms,
    /// with constant additional space. Eligible atoms are the open choice heads
    /// when any remain, otherwise all open atoms. Each primary count is charged
    /// once; a secondary count is read at most once, only when a primary tie
    /// requires it. An uncontested winner needs no secondary read. Packed mask
    /// traversal is not separately charged.
    /// Cancellation and quota reservations follow
    /// [`Self::preferred_atom_reserved`]; every failure retains its exact work
    /// prefix and returns no partial preference.
    ///
    /// # Errors
    /// Retains cancellation, a quota refusal or a zero-grant work refusal.
    ///
    /// # Panics
    /// Panics if an open atom is outside either knowledge's universe.
    pub fn preferred_atom_with_secondary_reserved(
        &self,
        region: &Region,
        choices: &ChoiceHeads,
        secondary: &Self,
        cancellation: &Cancellation,
        quota: &mut dyn NarrowingQuota,
    ) -> PreferenceAttempt {
        let mut work = Work::reserved(quota);
        let result = cancellation
            .poll()
            .and_then(|()| self.preferred_with_secondary(region, choices, secondary, &mut work));
        work.refund();
        PreferenceAttempt {
            result,
            work: work.spent,
        }
    }

    fn read_unknown(&self, atom: usize, work: &mut Work<'_>) -> Result<usize, Stop> {
        work.tick()?;
        Ok(self.unknown_count(atom))
    }

    fn preferred_with_secondary(
        &self,
        region: &Region,
        choices: &ChoiceHeads,
        secondary: &Self,
        work: &mut Work<'_>,
    ) -> Result<Option<usize>, Stop> {
        let mut best: Option<(usize, usize, Option<usize>)> = None;
        for atom in choices.eligible(region) {
            let primary = self.read_unknown(atom, work)?;
            let Some((previous, degree, cached)) = best.as_mut() else {
                best = Some((atom, primary, None));
                continue;
            };
            match primary.cmp(degree) {
                std::cmp::Ordering::Greater => {
                    *previous = atom;
                    *degree = primary;
                    *cached = None;
                }
                std::cmp::Ordering::Equal => {
                    let previous_count = if let Some(count) = *cached {
                        count
                    } else {
                        let count = secondary.read_unknown(*previous, work)?;
                        *cached = Some(count);
                        count
                    };
                    let count = secondary.read_unknown(atom, work)?;
                    if count > previous_count {
                        *previous = atom;
                        *cached = Some(count);
                    }
                }
                std::cmp::Ordering::Less => {}
            }
        }
        Ok(best.map(|(atom, _, _)| atom))
    }
}
