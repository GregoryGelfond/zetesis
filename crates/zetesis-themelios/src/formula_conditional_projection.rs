//! Bounded alternative syntax for compilation and dependency-only analysis.
//!
//! A conjunction of projected conditional literals is deliberately not asserted
//! equivalent to the runtime inner disjunction. Its purpose is to retain every
//! signature, polarity and parsed atom origin without passing pools upstream.

use themelios_program::program::{
    Arguments, Atom, Body, BodyElement, ConditionalLiteral, Literal, LiteralInner, Program,
    Statement,
};
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
    ) -> Result<WithProvenance<Statement>, FormulaFailure> {
        let Statement::Rule(rule) = statement.get() else {
            unreachable!("formula source rule")
        };
        let pooled = rule.body().get().elements().any(|element| {
            matches!(element.get(), BodyElement::Conditional(conditional)
                if matches!(&conditional.literal.inner, LiteralInner::Atom(atom) if atom_width(atom.get()).1 != 0))
        });
        if !pooled {
            return Ok(statement.clone());
        }
        let mut elements = Vec::new();
        let mut payload = Payload::default();
        payload.visit_statement(statement.get());
        self.budget.charge(
            ExpansionResource::TermWork,
            payload.nodes.saturating_mul(2),
            self.location,
        )?;
        // Reserve selected source payload plus each expanded conditional's
        // copied condition before allocating the corresponding body elements.
        self.budget.charge(
            ExpansionResource::ScalarBytes,
            payload.bytes.saturating_mul(2),
            self.location,
        )?;
        for element in rule.body().get().elements() {
            if let BodyElement::Conditional(conditional) = element.get()
                && let LiteralInner::Atom(atom) = &conditional.literal.inner
                && atom_width(atom.get()).1 != 0
            {
                // The outer pool cursor already reserved one source-copy
                // equivalent. Extra conditional copies share its cumulative
                // allowance across every statement and outer substitution.
                let count = atom_width(atom.get()).0;
                *projection_nodes = projection_nodes
                    .saturating_add(count.saturating_sub(1).saturating_mul(payload.nodes + 1));
                ceiling(
                    FormulaResource::AnalysisNodes,
                    *projection_nodes,
                    self.limits.max_analysis_nodes as u128,
                    self.location,
                )?;
                let alternatives = self.conditional_atoms(atom.get())?;
                let mut condition_payload = Payload::default();
                condition_payload.visit_condition(&conditional.condition);
                self.budget.charge(
                    ExpansionResource::TermWork,
                    condition_payload.nodes,
                    self.location,
                )?;
                self.budget.charge(
                    ExpansionResource::ScalarBytes,
                    alternatives.len() as u128
                        * (condition_payload.bytes + std::mem::size_of::<BodyElement>() as u128),
                    self.location,
                )?;
                self.budget.charge(
                    ExpansionResource::TermWork,
                    alternatives.len() as u128 * condition_payload.nodes,
                    self.location,
                )?;
                for alternative in alternatives {
                    elements.push(BodyElement::Conditional(ConditionalLiteral {
                        literal: Literal {
                            negation: conditional.literal.negation,
                            inner: LiteralInner::Atom(atom.clone().map(|_| alternative)),
                        },
                        condition: conditional.condition.clone(),
                    }));
                }
            } else {
                elements.push(element.get().clone());
            }
        }
        self.dependency_projection = true;
        let mut projection = Projection {
            body: Some(Body::new(elements)),
        };
        Ok(rewrite(Program::of([statement.clone()]), &mut projection)
            .statements()
            .next()
            .expect("projection retains rule")
            .clone())
    }
}

struct Projection {
    body: Option<Body>,
}
impl Rewrite for Projection {
    fn tag(&self) -> TransformTag {
        TransformTag::new("zetesis-conditional-dependencies")
    }
    fn rewrite_body(&mut self, _: Body) -> Body {
        self.body.take().expect("one rule body")
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
