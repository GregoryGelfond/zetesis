//! Bounded alternative syntax for compilation and dependency-only analysis.
//!
//! A conjunction of projected conditional literals is deliberately not asserted
//! equivalent to the runtime inner disjunction. Its purpose is to retain every
//! signature, polarity and parsed atom origin without passing pools upstream.

use themelios_program::program::{Arguments, Atom, Program, Statement};
use themelios_program::provenance::{TransformTag, WithProvenance};
use themelios_program::term::Term;
use themelios_program::transform::{Rewrite, Visit, rewrite};

use crate::diagnostic::unsupported;
use crate::formula::ceiling;
use crate::formula_ir::Compiler;
use crate::formula_pool::{atom_width, select_atom};
use crate::{ExpansionResource, FormulaFailure, FormulaResource, ProfileFeature};

impl Compiler<'_> {
    pub(super) fn conditional_atoms(&mut self, atom: &Atom) -> Result<Vec<Atom>, FormulaFailure> {
        let (count, admitted) = atom_width(atom);
        let mut scan = Payload::default();
        scan.visit_atom(atom);
        self.budget
            .charge(ExpansionResource::TermWork, scan.nodes, self.location)?;
        if admitted != scan.pools {
            return Err(unsupported(ProfileFeature::Term, self.location).into());
        }
        // Values limits bound source alternatives, including duplicate values.
        // Byte accounting reserves four term/text copy equivalents and atom
        // carriers for source selection; the independent analysis ceiling bounds nodes.
        self.budget
            .charge(ExpansionResource::Values, count, self.location)?;
        self.budget.charge(
            ExpansionResource::ScalarBytes,
            count
                .saturating_mul(scan.bytes.saturating_mul(4) + std::mem::size_of::<Atom>() as u128),
            self.location,
        )?;
        self.budget.charge(
            ExpansionResource::TermWork,
            count.saturating_mul(scan.nodes),
            self.location,
        )?;
        ceiling(
            FormulaResource::AnalysisNodes,
            count.saturating_mul(scan.nodes),
            self.limits.max_analysis_nodes as u128,
            self.location,
        )?;
        let count = usize::try_from(count).expect("finite value ceiling proves usize");
        Ok((0..count)
            .map(|position| select_atom(atom.clone(), position))
            .collect())
    }

    pub(super) fn conditional_projection(
        &mut self,
        statement: &WithProvenance<Statement>,
        projection_nodes: &mut u128,
    ) -> Result<Vec<WithProvenance<Statement>>, FormulaFailure> {
        let mut payload = Payload::default();
        payload.visit_statement(statement.get());
        if payload.pools == 0 {
            return Ok(vec![statement.clone()]);
        }
        let mut width = Width(1);
        width.visit_statement(statement.get());
        let count = width.0;
        self.budget
            .charge(ExpansionResource::TermWork, payload.nodes, self.location)?;
        self.budget
            .charge(ExpansionResource::Values, count, self.location)?;
        *projection_nodes =
            projection_nodes.saturating_add(count.saturating_mul(payload.nodes + 1));
        ceiling(
            FormulaResource::AnalysisNodes,
            *projection_nodes,
            self.limits.max_analysis_nodes as u128,
            self.location,
        )?;
        self.budget.charge(
            ExpansionResource::ScalarBytes,
            count.saturating_mul(payload.bytes.saturating_mul(4)),
            self.location,
        )?;
        self.budget.charge(
            ExpansionResource::TermWork,
            count.saturating_mul(payload.nodes.saturating_mul(4)),
            self.location,
        )?;
        self.dependency_projection = true;
        let count = usize::try_from(count).expect("bounded analysis alternatives");
        let mut result = Vec::with_capacity(count);
        for position in 0..count {
            let mut projection = Projection { position };
            let rewritten = rewrite(Program::of_nodes([statement.clone()]), &mut projection);
            debug_assert_eq!(projection.position, 0);
            result.push(
                rewritten
                    .statements()
                    .next()
                    .expect("projection retains statement")
                    .clone(),
            );
        }
        Ok(result)
    }
}

/// Cartesian syntax choices preserve every signed signature and argument arity.
/// Local quantifiers are deliberately not interpreted by this analysis view.
struct Width(u128);
impl Visit for Width {
    fn visit_atom(&mut self, atom: &Atom) {
        if let Arguments::Pooled(alternatives) = &atom.arguments {
            self.0 = self.0.saturating_mul(alternatives.len() as u128);
        }
        for term in atom.argument_terms().flat_map(Term::subterms) {
            self.visit_term(term);
        }
    }
    fn visit_term(&mut self, term: &Term) {
        if let Term::Pool(items) = term {
            self.0 = self.0.saturating_mul(items.len() as u128);
        }
    }
}
struct Projection {
    position: usize,
}
impl Projection {
    fn take(&mut self, width: usize) -> usize {
        let selected = self.position % width;
        self.position /= width;
        selected
    }
}
impl Rewrite for Projection {
    fn tag(&self) -> TransformTag {
        TransformTag::new("zetesis-pool-dependencies")
    }
    fn rewrite_term(&mut self, term: Term) -> Term {
        if let Term::Pool(items) = &term {
            items[self.take(items.len())].clone()
        } else {
            term
        }
    }
    fn rewrite_atom(&mut self, mut atom: Atom) -> Atom {
        // Rewrite's atom hook owns term traversal, as in the upstream default.
        atom.arguments = match atom.arguments {
            Arguments::Single(terms) => Arguments::Single(
                terms
                    .into_iter()
                    .map(|term| term.fold(|parts| self.rewrite_term(Term::from(parts))))
                    .collect(),
            ),
            Arguments::Pooled(alternatives) => {
                let alternatives: Vec<Vec<Term>> = alternatives
                    .into_iter()
                    .map(|terms| {
                        terms
                            .into_iter()
                            .map(|term| term.fold(|parts| self.rewrite_term(Term::from(parts))))
                            .collect()
                    })
                    .collect();
                Arguments::Single(alternatives[self.take(alternatives.len())].clone())
            }
        };
        atom
    }
}

#[derive(Default)]
struct Payload {
    nodes: u128,
    bytes: u128,
    pools: usize,
}
impl Visit for Payload {
    fn visit_atom(&mut self, atom: &Atom) {
        self.nodes += 1;
        self.bytes += atom.name.as_str().len() as u128;
        self.pools += usize::from(matches!(atom.arguments, Arguments::Pooled(_)));
        for term in atom.argument_terms().flat_map(Term::subterms) {
            self.visit_term(term);
        }
    }
    fn visit_term(&mut self, term: &Term) {
        self.nodes += 1;
        self.bytes += std::mem::size_of::<Term>() as u128;
        self.pools += usize::from(matches!(term, Term::Pool(_)));
        self.bytes += match term {
            Term::Function { name, .. } => name.as_str().len() as u128,
            Term::Symbolic(symbol) => crate::structural_value::symbol_bytes(symbol),
            Term::Variable(themelios_program::term::Variable::Named(name)) => {
                name.as_str().len() as u128
            }
            _ => 0,
        };
    }
}
