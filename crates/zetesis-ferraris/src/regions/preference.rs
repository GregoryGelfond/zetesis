//! Split ordering over already-maintained unknown-parent counts.

use super::{ChoiceHeads, Knowledge, NarrowingQuota, PreferenceAttempt, Region, Width, Work};
use zetesis_cpu::Cancellation;

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
    /// The scan takes `O(A / 64 + U)` time for `A` atoms and `U` open atoms,
    /// with constant additional space and two charged count reads per open atom.
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
        let result = cancellation.poll().and_then(|()| {
            let mut best = None;
            for atom in region.open() {
                work.tick()?;
                let primary = self.unknown_count(atom);
                work.tick()?;
                let secondary = secondary.unknown_count(atom);
                let rank = (choices.contains(atom), primary, secondary);
                if best.is_none_or(|(_, previous)| rank > previous) {
                    best = Some((atom, rank));
                }
            }
            Ok(best.map(|(atom, _)| atom))
        });
        work.refund();
        PreferenceAttempt {
            result,
            work: work.spent,
        }
    }
}
