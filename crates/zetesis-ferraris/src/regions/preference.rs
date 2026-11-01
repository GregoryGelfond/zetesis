//! Split ordering over already-maintained unknown-parent counts.

use zetesis_algebra::{Failure, Quota};

use super::{ChoiceHeads, Knowledge, Meter, PreferenceAttempt, Region, Width};
use zetesis_algebra::Cancellation;

impl<C: zetesis_algebra::Class> Knowledge<C> {
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
    pub fn preferred_atom_with_secondary_reserved<Q: Quota + ?Sized>(
        &self,
        region: &Region,
        choices: &ChoiceHeads,
        secondary: &Self,
        cancellation: &Cancellation,
        work: &mut Meter<'_, Q>,
    ) -> PreferenceAttempt<Failure<Q::Error>> {
        let before = work.charged();
        let result = cancellation
            .poll()
            .map_err(Failure::from)
            .and_then(|()| self.preferred_with_secondary(region, choices, secondary, work));
        work.refund();
        PreferenceAttempt {
            result,
            work: work.charged() - before,
        }
    }

    fn read_unknown<Q: Quota + ?Sized>(
        &self,
        atom: usize,
        work: &mut Meter<'_, Q>,
    ) -> Result<usize, Failure<Q::Error>> {
        work.tick()?;
        Ok(self.unknown_count(atom))
    }

    fn preferred_with_secondary<Q: Quota + ?Sized>(
        &self,
        region: &Region,
        choices: &ChoiceHeads,
        secondary: &Self,
        work: &mut Meter<'_, Q>,
    ) -> Result<Option<usize>, Failure<Q::Error>> {
        let mut best: Option<(usize, usize, Option<usize>)> = None;
        choices.try_for_each_eligible(region, |atom| -> Result<(), Failure<Q::Error>> {
            let primary = self.read_unknown(atom, work)?;
            let Some((previous, degree, cached)) = best.as_mut() else {
                best = Some((atom, primary, None));
                return Ok(());
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
            Ok(())
        })?;
        Ok(best.map(|(atom, _, _)| atom))
    }
}
