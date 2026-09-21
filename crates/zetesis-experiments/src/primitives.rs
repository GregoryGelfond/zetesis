//! Typed dispatch over the primitive profiles with structured observations.
//!
//! The observer sees preparation, every retained population and completion.
//! Measurement and parity remain owned by each profile; no formatted output is
//! parsed, and selecting a physical device never permits a CPU fallback.

use std::io;

use crate::{
    aggregate_measurement, command::Error, lazy_measurement, relation_measurement,
    tight_measurement,
};

/// A profile request, independent of command-line parsing. Each measurement
/// owner validates its finite configuration before performing work.
#[derive(Clone, Debug)]
pub enum Request {
    /// Packed equality masks and typed row reconstruction.
    Relation(relation_measurement::Configuration),
    /// Native aggregate reductions over original and frozen eligibility.
    Aggregate(aggregate_measurement::Configuration),
    /// Complete-theory tight classification and exact residual completion.
    Tight(tight_measurement::Configuration),
    /// Exact lazy source rounds over matched candidates.
    Lazy(lazy_measurement::Configuration),
}

/// A borrowed domain event. Serializing it preserves the selected profile's
/// original structured event schema; human consumers inspect the typed variant.
#[derive(serde::Serialize)]
#[serde(untagged)]
pub enum Event<'a, 'event> {
    /// Relation preparation, sample or completion.
    Relation(&'event relation_measurement::Event<'a>),
    /// Aggregate preparation, sample or completion.
    Aggregate(&'event aggregate_measurement::Event<'a>),
    /// Tight preparation, sample or completion.
    Tight(&'event tight_measurement::Event<'a>),
    /// Lazy preparation, sample or completion.
    Lazy(&'event lazy_measurement::Event<'a>),
}

/// Measure one profile through its existing bounded parity protocol.
///
/// The synchronous observer is called outside the measured interval. It may
/// retain owned summaries or serialize events, and can refuse further output.
/// No completion event is offered after a measurement or observer failure.
///
/// # Errors
/// Returns the original typed profile error, including observer I/O failures.
pub fn measure(
    request: &Request,
    mut observe: impl FnMut(Event<'_, '_>) -> io::Result<()>,
) -> Result<(), Error> {
    match request {
        Request::Relation(configuration) => relation_measurement::measure(
            *configuration,
            &zetesis_cpu::Control::default(),
            |event| observe(Event::Relation(event)),
        )
        .map_err(Error::Relation),
        Request::Aggregate(configuration) => {
            aggregate_measurement::measure(configuration, |event| observe(Event::Aggregate(event)))
                .map_err(Error::Aggregate)
        }
        Request::Tight(configuration) => {
            tight_measurement::measure(configuration, |event| observe(Event::Tight(event)))
                .map_err(Error::Tight)
        }
        Request::Lazy(configuration) => {
            lazy_measurement::measure(configuration, |event| observe(Event::Lazy(event)))
                .map_err(Error::Lazy)
        }
    }
}
