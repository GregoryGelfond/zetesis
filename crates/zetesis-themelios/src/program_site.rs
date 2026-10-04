//! Statement identity is independent of source coordinates.

use std::{fmt, num::NonZeroUsize};

use themelios_base::span::Location;

/// A statement's position in its retained original canonical program.
/// IDs belong to that program owner; they are not portable across inputs.
// Store the position plus one so an optional identity occupies one word.
// Absence remains `Option::None`; every stored identity decodes to a position.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StatementId(NonZeroUsize);

impl StatementId {
    pub(crate) const fn new(index: usize) -> Self {
        // A valid position is strictly below its canonical program's usize length.
        Self(
            NonZeroUsize::MIN
                .checked_add(index)
                .expect("an original statement index is below usize::MAX"),
        )
    }

    /// Position in the owner's canonical statement order.
    #[must_use]
    pub const fn index(self) -> usize {
        self.0.get() - 1
    }
}

impl fmt::Debug for StatementId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("StatementId").field(&self.index()).finish()
    }
}

/// The subject of compilation or grounding work, with a real source location
/// when one exists. A whole-program site has no statement identity. Constructed
/// statements need no source location; no coordinate is fabricated for them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProgramSite {
    statement: Option<StatementId>,
    location: Option<Location>,
}

impl ProgramSite {
    /// A whole canonical program, independent of source text.
    #[must_use]
    pub const fn program() -> Self {
        Self {
            statement: None,
            location: None,
        }
    }

    /// Source work before an individual statement has been identified.
    #[must_use]
    pub const fn source(location: Location) -> Self {
        Self {
            statement: None,
            location: Some(location),
        }
    }

    /// An original statement, optionally carrying its parsed coordinate.
    #[must_use]
    pub const fn statement(statement: StatementId, location: Option<Location>) -> Self {
        Self {
            statement: Some(statement),
            location,
        }
    }

    /// Original statement identity, resolved through the retained program.
    #[must_use]
    pub const fn statement_id(self) -> Option<StatementId> {
        self.statement
    }

    /// Actual parsed coordinate, absent for constructed input.
    #[must_use]
    pub const fn location(self) -> Option<Location> {
        self.location
    }

    /// Retain the statement identity while selecting a real parsed coordinate.
    #[must_use]
    pub const fn with_location(self, location: Location) -> Self {
        Self {
            location: Some(location),
            ..self
        }
    }

    /// Retain a real coordinate while identifying its original statement.
    #[must_use]
    pub const fn with_statement(self, statement: StatementId) -> Self {
        Self {
            statement: Some(statement),
            ..self
        }
    }
}

impl From<Location> for ProgramSite {
    fn from(location: Location) -> Self {
        Self::source(location)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use themelios_base::source::SourceId;
    use themelios_base::span::{ByteOffset, Span};

    #[test]
    fn statement_indices_round_trip_at_boundaries() {
        for index in [0, 1, usize::MAX - 1] {
            let id = StatementId::new(index);
            assert_eq!(id.index(), index);
            assert_eq!(format!("{id:?}"), format!("StatementId({index})"));
        }
    }

    #[test]
    #[should_panic(expected = "an original statement index is below usize::MAX")]
    fn unrepresentable_statement_index_refuses_wrapping() {
        StatementId::new(usize::MAX);
    }

    #[test]
    fn site_coordinates_do_not_define_statement_identity() {
        let id = StatementId::new(0);
        let location = Location {
            source: SourceId::new(7),
            span: Span::empty(ByteOffset::new(0)),
        };
        let program = ProgramSite::program();
        assert_eq!(program.statement_id(), None);
        assert_eq!(program.location(), None);
        let constructed = ProgramSite::statement(id, None);
        assert_eq!(constructed.statement_id(), Some(id));
        assert_eq!(constructed.location(), None);
        let source = ProgramSite::source(location);
        assert_eq!(source.statement_id(), None);
        assert_eq!(source.location(), Some(location));
        let located = constructed.with_location(location);
        assert_eq!(located, source.with_statement(id));
        assert_eq!(located.statement_id(), Some(id));
        assert_eq!(located.location(), Some(location));
    }
}
