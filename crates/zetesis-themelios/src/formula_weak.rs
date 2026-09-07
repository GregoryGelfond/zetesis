//! Bounded positive weak-constraint normalization through upstream owned syntax.
//!
//! Public upstream constructors mark synthesized condition children constructed.
//! Their original body/element provenance is retained on the enclosing normalized
//! statement and in the frontend's objective evidence, never fabricated on nodes.

use themelios_base::span::Location;
use themelios_program::program::{
    Atom, BodyElement, Condition, Direction, Optimize, OptimizeElement, Statement,
};
use themelios_program::provenance::{Origin, Provenance, TransformTag, WithProvenance};
use themelios_program::symbol::Symbol;
use themelios_program::term::{Term, Variable};
use themelios_program::transform::Visit;

use crate::diagnostic::unsupported;
use crate::expansion::Budget;
use crate::{ExpansionResource, FormulaFailure, ProfileFeature};

pub(super) fn normalize(
    source: &WithProvenance<Statement>,
    budget: &mut Budget,
    location: Location,
) -> Result<Option<WithProvenance<Statement>>, FormulaFailure> {
    let Statement::WeakConstraint(weak) = source.get() else {
        return Ok(None);
    };
    let mut size = CloneSize::default();
    size.visit_statement(source.get());
    budget.charge(
        ExpansionResource::TermWork,
        size.nodes.saturating_add(1),
        location,
    )?;
    budget.charge(
        ExpansionResource::ScalarBytes,
        size.bytes.saturating_mul(3),
        location,
    )?;
    let origin_count = source.provenance().origins().count() as u128
        + weak.body().provenance().origins().count() as u128
        + weak
            .body()
            .get()
            .elements()
            .map(|element| element.provenance().origins().count() as u128)
            .sum::<u128>();
    budget.charge(
        ExpansionResource::Origins,
        origin_count.saturating_add(1),
        location,
    )?;
    let mut provenance = source
        .provenance()
        .clone()
        .merge(weak.body().provenance().clone())
        .merge(Provenance::from(Origin::Transformed(TransformTag::new(
            "zetesis-positive-weak-objective",
        ))));
    let mut literals = Vec::new();
    for element in weak.body().get().elements() {
        budget.charge(ExpansionResource::TermWork, 1, location)?;
        let BodyElement::Literal(literal) = element.get() else {
            return Err(unsupported(ProfileFeature::Objective, location).into());
        };
        provenance = provenance.merge(element.provenance().clone());
        literals.push(literal.clone());
    }
    let element = OptimizeElement::new(
        weak.weight().clone(),
        weak.terms().cloned(),
        Condition::new(literals),
    );
    Ok(Some(WithProvenance::new(
        Statement::Optimize(Optimize::new(Direction::Minimize, [element])),
        provenance,
    )))
}

#[derive(Default)]
struct CloneSize {
    nodes: u128,
    bytes: u128,
}
impl Visit for CloneSize {
    fn visit_atom(&mut self, atom: &Atom) {
        self.nodes = self.nodes.saturating_add(1);
        self.bytes = self.bytes.saturating_add(atom.name.as_str().len() as u128);
        for arguments in atom.alternatives() {
            for term in arguments {
                for node in term.subterms() {
                    self.visit_term(node);
                }
            }
        }
    }
    fn visit_term(&mut self, term: &Term) {
        self.nodes = self.nodes.saturating_add(1);
        match term {
            Term::Variable(Variable::Named(name)) => {
                self.bytes = self.bytes.saturating_add(name.as_str().len() as u128);
            }
            Term::Function { name, .. } | Term::External { name, .. } => {
                self.bytes = self.bytes.saturating_add(name.as_str().len() as u128);
            }
            Term::Symbolic(symbol) => {
                for symbol in symbol.subsymbols() {
                    self.nodes = self.nodes.saturating_add(1);
                    self.bytes = self.bytes.saturating_add(match symbol {
                        Symbol::String(text) => text.len() as u128,
                        Symbol::Function { name, .. } => name.as_str().len() as u128,
                        _ => 0,
                    });
                }
            }
            _ => {}
        }
    }
}
