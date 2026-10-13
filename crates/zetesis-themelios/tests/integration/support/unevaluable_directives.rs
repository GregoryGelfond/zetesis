//! Directives whose terms cannot be evaluated, 1/0, for checking that an
//! operation never evaluates them.

use zetesis_themelios::logical::{
    Name, Term,
    program::{
        Atom, Body, Condition, Direction, Optimize, OptimizeElement, Project, Statement, weight,
    },
};

/// A minimize statement with one element weighted 1/0.
pub(crate) fn objective() -> Statement {
    Optimize::new(
        Direction::Minimize,
        [OptimizeElement::new(
            weight(Term::from(1) / Term::from(0)),
            [],
            Condition::empty(),
        )],
    )
    .into()
}

/// A projection onto `selected(1/0)`.
pub(crate) fn projection() -> Statement {
    Project::atom_body(
        Atom::new(
            Name::new("selected").unwrap(),
            [Term::from(1) / Term::from(0)],
        ),
        Body::empty(),
    )
    .into()
}
