//! A settled numeric output leaves no additional child restriction to propagate.
//!
//! Occurrence assignments independently define the relaxed output mask. Coherent
//! physical-slot assignments then check that this relaxation remains sound under
//! stale observations and aliases. This is a finite semantic law, not a host copy
//! of the shader transfer; the native device fixtures check its implementation.

use super::gate_transfer::{domains, fits};
use zetesis_ferraris::ExtremumCutRule;

#[derive(Clone, Copy, Debug)]
enum Row<'a> {
    Threshold {
        weights: &'a [u64],
        bound: i128,
    },
    Extremum {
        ranks: &'a [u64],
        rank_count: u64,
        cut: ExtremumCutRule,
    },
}

impl Row<'_> {
    fn holds(self, selected: &[bool]) -> bool {
        match self {
            Self::Threshold { weights, bound } => {
                weights
                    .iter()
                    .zip(selected)
                    .filter(|(_, selected)| **selected)
                    .map(|(&weight, _)| i128::from(weight))
                    .sum::<i128>()
                    >= bound
            }
            Self::Extremum {
                ranks,
                rank_count,
                cut,
            } => {
                let selected_ranks = ranks
                    .iter()
                    .zip(selected)
                    .filter(|(_, selected)| **selected)
                    .map(|(&rank, _)| rank);
                match cut {
                    // Empty min/max retain their respective ordinal endpoints.
                    ExtremumCutRule::Prefix(boundary) => {
                        selected_ranks.min().unwrap_or(rank_count - 1) < boundary
                    }
                    ExtremumCutRule::Suffix(boundary) => {
                        selected_ranks.max().unwrap_or(0) >= boundary
                    }
                }
            }
        }
    }
}

fn assignment(bits: u8) -> [bool; 3] {
    [bits & 1 != 0, bits & 2 != 0, bits & 4 != 0]
}

// Occurrences deliberately do not share slots here. Independent finite choices
// give the same possible threshold outputs as minimum/maximum bounds, and the
// same interior extremum outputs as witness existence. No child forcing is copied.
fn observed_outputs(row: Row<'_>, observed: &[u8]) -> u8 {
    let mut allowed = 0;
    for bits in 0..(1_u8 << observed.len()) {
        let selected = assignment(bits);
        if observed
            .iter()
            .zip(selected)
            .all(|(&mask, value)| mask & (1 << u8::from(value)) != 0)
        {
            allowed |= 1 << u8::from(row.holds(&selected[..observed.len()]));
        }
    }
    allowed
}

#[derive(Default)]
struct Checked {
    settled: usize,
    conflicts: usize,
    stale_aliases: usize,
}

fn check_row(row: Row<'_>, slots: &[usize], checked: &mut Checked) {
    for code in 0..64 {
        let observed = domains(code).masks();
        let allowed = observed_outputs(row, &observed[..slots.len()]);
        for observed_output in 0..4 {
            for current in (0..64).map(domains) {
                let current = current.masks();
                if current[2] & !observed_output != 0
                    || slots
                        .iter()
                        .zip(observed)
                        .any(|(&slot, mask)| current[slot] & !mask != 0)
                {
                    continue;
                }
                let mut narrowed = current;
                narrowed[2] &= allowed;
                let mut exact_support = [0; 3];
                for bits in 0..8 {
                    let values = assignment(bits);
                    let mut occurrence_values = [false; 3];
                    for (&slot, selected) in slots.iter().zip(&mut occurrence_values) {
                        *selected = values[slot];
                    }
                    if !fits(current, values)
                        || row.holds(&occurrence_values[..slots.len()]) != values[2]
                    {
                        continue;
                    }
                    // Check preservation for every compatible coherent completion,
                    // including unsettled masks as the negative/control population.
                    assert!(fits(narrowed, values), "{row:?}: {slots:?}");
                    for (support, value) in exact_support.iter_mut().zip(values) {
                        *support |= 1 << u8::from(value);
                    }
                }
                if allowed != 3 && !narrowed.contains(&0) {
                    // Exact relational projection adds no child restriction after
                    // output intersection. Therefore a sound skipped child transfer
                    // cannot have narrowed any surviving physical slot either.
                    assert_eq!(exact_support, narrowed, "{row:?}: {slots:?}");
                    checked.settled += 1;
                    checked.stale_aliases +=
                        usize::from(slots.iter().enumerate().any(|(index, &slot)| {
                            slots[..index].contains(&slot) && current[slot] != observed[index]
                        }));
                }
                if !current[..2].contains(&0) && current[2] != 0 && narrowed[2] == 0 && allowed != 0
                {
                    assert_eq!(exact_support, [0; 3]);
                    checked.conflicts += 1;
                }
            }
        }
    }
}

#[test]
fn settled_numeric_masks_leave_children_unrestricted() {
    let mut checked = Checked::default();
    let wide = i128::from(u64::MAX);
    for slots in [[0, 0, 0], [0, 0, 1], [0, 1, 0], [0, 1, 1]] {
        for weights in [[0, 1, 2], [u64::MAX, u64::MAX, 0]] {
            for bound in [-1, 0, 1, 2, wide + 1, 2 * wide + 1] {
                check_row(
                    Row::Threshold {
                        weights: &weights,
                        bound,
                    },
                    &slots,
                    &mut checked,
                );
            }
        }
        for (ranks, rank_count) in [([0, 2, 4], 5), ([0, 1, u64::MAX - 1], u64::MAX)] {
            for boundary in [0, 1, rank_count - 1, rank_count] {
                for cut in [
                    ExtremumCutRule::Prefix(boundary),
                    ExtremumCutRule::Suffix(boundary),
                ] {
                    check_row(
                        Row::Extremum {
                            ranks: &ranks,
                            rank_count,
                            cut,
                        },
                        &slots,
                        &mut checked,
                    );
                }
            }
        }
    }
    assert!(checked.settled > 0);
    assert!(checked.conflicts > 0);
    assert!(checked.stale_aliases > 0);
}

#[test]
fn empty_numeric_rows_retain_their_endpoint_outputs() {
    let mut checked = Checked::default();
    for bound in [-1, 0, 1] {
        check_row(
            Row::Threshold {
                weights: &[],
                bound,
            },
            &[],
            &mut checked,
        );
    }
    for rank_count in [2, u64::MAX] {
        for boundary in [0, 1, rank_count - 1, rank_count] {
            for cut in [
                ExtremumCutRule::Prefix(boundary),
                ExtremumCutRule::Suffix(boundary),
            ] {
                check_row(
                    Row::Extremum {
                        ranks: &[],
                        rank_count,
                        cut,
                    },
                    &[],
                    &mut checked,
                );
            }
        }
    }
    assert!(checked.settled > 0);
    assert!(checked.conflicts > 0);
    assert_eq!(checked.stale_aliases, 0);
}
