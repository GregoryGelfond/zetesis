//! Bounded source-occurrence pools before ordinary formula compilation.
//!
//! An outer cursor selects each pooled literal independently, emitting complete
//! rules. Within a choice, selections instead add elements to the same group.
//! No constructor closure, textual substitution or upstream unpooling is used.
//! Pool-free alternatives retain the existing scalar/range admission contract.

use themelios_base::span::Location;
use themelios_program::program::{
    Arguments, Atom, Body, BodyElement, Choice, ChoiceElement, Comparison, DefaultNegation, Head,
    Literal, LiteralInner, Program, Relation, Statement,
};
use themelios_program::provenance::{TransformTag, WithProvenance};
use themelios_program::term::Term;
use themelios_program::transform::{Rewrite, Visit, rewrite};

use crate::diagnostic::unsupported;
use crate::expansion::Budget;
use crate::formula_ir::{Compiler, RuleIr};
use crate::{ExpansionResource, FormulaFailure, FormulaLimits, FormulaResource, ProfileFeature};

impl Compiler<'_> {
    /// Append a bounded family of whole rules, retaining every source occurrence
    /// even when the separate analyzed program later deduplicates equal rules.
    pub(super) fn source_rules(
        &mut self,
        statement: &WithProvenance<Statement>,
        origins: &[Location],
        projection_nodes: &mut u128,
        rules: &mut Vec<RuleIr>,
        analyzed: &mut Vec<WithProvenance<Statement>>,
    ) -> Result<(), FormulaFailure> {
        if let Some(mut cursor) = Cursor::new(
            statement,
            origins.len(),
            self.limits,
            projection_nodes,
            self.budget,
            self.location,
        )? {
            while let Some(statement) = cursor.next(self.budget, self.location)? {
                let Statement::Rule(rule) = statement.get() else {
                    unreachable!("pool expansion retains a rule")
                };
                rules.push(self.rule(rule, origins.to_vec())?);
                analyzed.push(self.conditional_projection(&statement, projection_nodes)?);
            }
        } else {
            self.budget
                .charge(ExpansionResource::Templates, 1, self.location)?;
            self.budget.charge(
                ExpansionResource::Origins,
                origins.len() as u128,
                self.location,
            )?;
            let Statement::Rule(rule) = statement.get() else {
                return Err(unsupported(ProfileFeature::Statement, self.location).into());
            };
            rules.push(self.rule(rule, origins.to_vec())?);
            analyzed.push(self.conditional_projection(statement, projection_nodes)?);
        }
        Ok(())
    }
}

/// Owns only occurrence positions, never the Cartesian output. Equal values do
/// not merge these positions. Each returned statement is compiled separately.
struct Cursor<'a> {
    source: &'a WithProvenance<Statement>,
    widths: Vec<usize>,
    positions: Vec<usize>,
    remaining: usize,
    nodes: u128,
    bytes: u128,
}

impl<'a> Cursor<'a> {
    fn new(
        source: &'a WithProvenance<Statement>,
        origins: usize,
        limits: &FormulaLimits,
        projection_nodes: &mut u128,
        budget: &mut Budget,
        location: Location,
    ) -> Result<Option<Self>, FormulaFailure> {
        let mut scan = Footprint {
            nodes: 1,
            ..Footprint::default()
        };
        scan.visit_statement(source.get());
        budget.charge(ExpansionResource::TermWork, scan.nodes, location)?;
        if scan.pools == 0 {
            return Ok(None);
        }
        let Statement::Rule(rule) = source.get() else {
            return Err(unsupported(ProfileFeature::Term, location).into());
        };
        budget.charge(
            ExpansionResource::ScalarBytes,
            (scan.pools as u128).saturating_mul(
                (std::mem::size_of::<u128>() + 2 * std::mem::size_of::<usize>()) as u128,
            ),
            location,
        )?;
        let Shape {
            widths,
            admitted,
            local_copies,
        } = shape(rule, scan.pools);
        // Every residual pool must belong to one of the contexts above. In
        // particular, conditions/aggregates/body atoms and nested arithmetic
        // pools do not acquire a guessed expansion law through this pass.
        if admitted != scan.pools {
            return Err(unsupported(ProfileFeature::Term, location).into());
        }
        let count = widths
            .iter()
            .fold(1_u128, |count, width| count.saturating_mul(*width));
        budget.charge(ExpansionResource::Templates, count, location)?;
        budget.charge(
            ExpansionResource::Origins,
            count.saturating_mul(origins as u128),
            location,
        )?;
        let copies = local_copies.saturating_add(1);
        let nodes = scan.nodes.saturating_mul(copies);
        // A conservative cumulative projection bound is checked before cloning;
        // final analysis independently counts the exact, deduplicated program.
        *projection_nodes = projection_nodes.saturating_add(nodes.saturating_mul(count));
        crate::formula::ceiling(
            FormulaResource::AnalysisNodes,
            *projection_nodes,
            limits.max_analysis_nodes as u128,
            location,
        )?;
        budget.charge(
            ExpansionResource::Values,
            scan.nodes.saturating_mul(copies).saturating_mul(count),
            location,
        )?;
        let positions = vec![0; widths.len()];
        let widths = widths
            .into_iter()
            .map(|width| usize::try_from(width).expect("bounded pool width"))
            .collect();
        Ok(Some(Self {
            source,
            widths,
            positions,
            remaining: usize::try_from(count).expect("bounded by usize template limit"),
            nodes: nodes.saturating_mul(4),
            bytes: scan.bytes.saturating_mul(copies).saturating_mul(4),
        }))
    }

    fn next(
        &mut self,
        budget: &mut Budget,
        location: Location,
    ) -> Result<Option<WithProvenance<Statement>>, FormulaFailure> {
        if self.remaining == 0 {
            return Ok(None);
        }
        // Reserve four equivalents of counted term cells and text payload for
        // the owned rewrite, selected children and retained analysis statement.
        // Other AST carriers, provenance and allocator overhead are excluded;
        // source/syntax and projection-node limits bound structural expansion.
        budget.charge(ExpansionResource::TermWork, self.nodes, location)?;
        budget.charge(ExpansionResource::ScalarBytes, self.bytes, location)?;
        let mut selector = Select {
            positions: self.positions.iter(),
        };
        let expanded = rewrite(
            Program::of([self.source.clone()]),
            &mut Expand {
                selector: &mut selector,
            },
        );
        assert!(
            selector.positions.next().is_none(),
            "all pool occurrences selected"
        );
        let statement = expanded
            .statements()
            .next()
            .expect("one rule remains one rule")
            .clone();
        self.remaining -= 1;
        for (position, width) in self.positions.iter_mut().zip(&self.widths).rev() {
            *position += 1;
            if *position < *width {
                break;
            }
            *position = 0;
        }
        Ok(Some(statement))
    }
}

struct Shape {
    widths: Vec<u128>,
    admitted: usize,
    local_copies: u128,
}

fn shape(rule: &themelios_program::program::Rule, capacity: usize) -> Shape {
    let mut widths = Vec::with_capacity(capacity);
    let mut admitted = 0;
    let mut local_copies = 0_u128;
    match rule.head().get() {
        Head::Literal(literal) => {
            outer(literal, &mut widths, &mut admitted);
        }
        Head::Disjunction(head) => {
            for element in head.elements() {
                outer(element.get().literal(), &mut widths, &mut admitted);
            }
        }
        Head::Choice(choice) => {
            for element in choice.elements() {
                if let LiteralInner::Atom(atom) = &element.get().literal().inner {
                    let (count, pools) = atom_width(atom.get());
                    admitted += pools;
                    local_copies = local_copies.saturating_add(count.saturating_sub(1));
                }
            }
        }
        _ => {}
    }
    for element in rule.body().get().elements() {
        if let BodyElement::Conditional(conditional) = element.get() {
            let mut scan = Footprint::default();
            scan.visit_literal(&conditional.literal);
            admitted += scan.pools;
        }
        if let BodyElement::Literal(literal) = element.get()
            && literal.negation == DefaultNegation::None
            && let LiteralInner::Comparison(comparison) = &literal.inner
        {
            let comparison = comparison.get();
            let mut steps = comparison.steps();
            let (relation, right) = steps.next().expect("comparison has a step");
            if relation == Relation::Eq && steps.next().is_none() {
                let pooled = match (comparison.first(), right) {
                    (Term::Variable(_), Term::Pool(items))
                    | (Term::Pool(items), Term::Variable(_)) => Some(items.len()),
                    _ => None,
                };
                if let Some(count) = pooled {
                    widths.push(count as u128);
                    admitted += 1;
                }
            }
        }
    }
    Shape {
        widths,
        admitted,
        local_copies,
    }
}

fn outer(literal: &Literal, widths: &mut Vec<u128>, admitted: &mut usize) {
    if let LiteralInner::Atom(atom) = &literal.inner {
        let (width, pools) = atom_width(atom.get());
        if pools != 0 {
            // Keep widened cardinalities until the cumulative template check;
            // narrowing first could hide an overflowing product.
            widths.push(width);
            *admitted += pools;
        }
    }
}

fn term_width(term: &Term) -> usize {
    if let Term::Pool(items) = term {
        items.len()
    } else {
        1
    }
}

pub(super) fn atom_width(atom: &Atom) -> (u128, usize) {
    let mut pools = usize::from(matches!(atom.arguments, Arguments::Pooled(_)));
    let count = atom.alternatives().fold(0_u128, |sum, arguments| {
        let product = arguments.iter().fold(1_u128, |product, term| {
            pools += usize::from(matches!(term, Term::Pool(_)));
            product.saturating_mul(term_width(term) as u128)
        });
        sum.saturating_add(product)
    });
    (count, pools)
}

pub(super) fn select_atom(mut atom: Atom, mut position: usize) -> Atom {
    let arguments = atom
        .alternatives()
        .find_map(|arguments| {
            let width = arguments
                .iter()
                .fold(1_usize, |width, term| width * term_width(term));
            if position >= width {
                position -= width;
                None
            } else {
                let mut selected = arguments.to_vec();
                for term in selected.iter_mut().rev() {
                    if let Term::Pool(items) = term {
                        let index = position % items.len();
                        position /= items.len();
                        *term = items[index].clone();
                    }
                }
                Some(selected)
            }
        })
        .expect("bounded occurrence position");
    atom.arguments = Arguments::Single(arguments);
    atom
}

struct Select<'a> {
    positions: std::slice::Iter<'a, usize>,
}
impl Rewrite for Select<'_> {
    fn tag(&self) -> TransformTag {
        TransformTag::new("zetesis-finite-pools")
    }
    fn rewrite_atom(&mut self, atom: Atom) -> Atom {
        if atom_width(&atom).1 == 0 {
            atom
        } else {
            select_atom(
                atom,
                *self.positions.next().expect("atom occurrence position"),
            )
        }
    }
    fn rewrite_comparison(&mut self, comparison: Comparison) -> Comparison {
        let mut steps = comparison.steps();
        let (relation, right) = steps.next().expect("comparison has a step");
        let select = |term: &Term, positions: &mut std::slice::Iter<'_, usize>| {
            if let Term::Pool(items) = term {
                items[*positions.next().expect("equality occurrence position")].clone()
            } else {
                term.clone()
            }
        };
        let mut result = Comparison::new(
            select(comparison.first(), &mut self.positions),
            relation,
            select(right, &mut self.positions),
        );
        for (relation, term) in steps {
            result = result.chain(relation, term.clone());
        }
        result
    }
}

struct Expand<'a, 'b> {
    selector: &'a mut Select<'b>,
}
impl Rewrite for Expand<'_, '_> {
    fn tag(&self) -> TransformTag {
        self.selector.tag()
    }
    fn rewrite_body(&mut self, body: Body) -> Body {
        // The conditional compiler owns inner-OR alternatives. Outer pool
        // positions never select or duplicate a universal consequent.
        Body::new(body.elements().map(|element| match element.get() {
            BodyElement::Literal(literal) => {
                BodyElement::Literal(self.selector.rewrite_literal(literal.clone()))
            }
            other => other.clone(),
        }))
    }
    fn rewrite_head(&mut self, head: Head) -> Head {
        let Head::Choice(choice) = head else {
            return self.selector.rewrite_head(head);
        };
        let mut elements = Vec::new();
        for element in choice.elements() {
            let literal = element.get().literal();
            if let LiteralInner::Atom(atom) = &literal.inner {
                let (count, _) = atom_width(atom.get());
                for index in 0..usize::try_from(count).expect("bounded local copies") {
                    elements.push(ChoiceElement::new(
                        Literal {
                            negation: literal.negation,
                            inner: LiteralInner::Atom(
                                atom.clone().map(|atom| select_atom(atom, index)),
                            ),
                        },
                        element.get().condition().clone(),
                    ));
                }
            } else {
                elements.push(element.get().clone());
            }
        }
        // The enclosing rule/head retains parsed origins. Synthesized element
        // nodes truthfully carry constructed provenance, like expanded facts.
        Head::Choice(Choice::new(
            choice.left_guard().map(|g| g.get().clone()),
            elements,
            choice.right_guard().map(|g| g.get().clone()),
        ))
    }
}

#[derive(Default)]
struct Footprint {
    nodes: u128,
    bytes: u128,
    pools: usize,
}
impl Visit for Footprint {
    fn visit_atom(&mut self, atom: &Atom) {
        self.nodes += 1;
        self.bytes += atom.name.as_str().len() as u128;
        self.pools += usize::from(matches!(atom.arguments, Arguments::Pooled(_)));
        for term in atom.argument_terms() {
            for node in term.subterms() {
                self.visit_term(node);
            }
        }
    }
    fn visit_term(&mut self, term: &Term) {
        self.nodes += 1;
        self.bytes += std::mem::size_of::<Term>() as u128;
        self.pools += usize::from(matches!(term, Term::Pool(_)));
        if let Term::Function { name, .. } = term {
            self.bytes += name.as_str().len() as u128;
        }
        if let Term::Symbolic(symbol) = term {
            self.bytes += crate::structural_value::symbol_bytes(symbol);
        }
        if let Term::Variable(themelios_program::term::Variable::Named(name)) = term {
            self.bytes += name.as_str().len() as u128;
        }
    }
}
