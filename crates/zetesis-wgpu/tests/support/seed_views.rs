//! Explicit shared fixtures for testing the borrowed GPU candidate doors.
use std::sync::Arc;
use zetesis_core::{Seed, SeedSelection};

pub(super) fn selections(seeds: &[Seed]) -> Vec<SeedSelection> {
    seeds
        .iter()
        .map(|seed| {
            SeedSelection::new(seed.program(), seed.atoms().iter().cloned().map(Arc::new)).unwrap()
        })
        .collect()
}
