//! Explicit shared fixtures for testing the borrowed GPU candidate doors.
use zetesis_core::{Seed, SeedSelection};

pub(crate) fn selections(seeds: &[Seed]) -> Vec<SeedSelection> {
    seeds
        .iter()
        .map(|seed| {
            SeedSelection::from_carrier_atoms(
                seed.program(),
                seed.atoms()
                    .iter()
                    .map(|atom| seed.program().locate_atom(atom, true).unwrap().unwrap()),
            )
            .unwrap()
        })
        .collect()
}
