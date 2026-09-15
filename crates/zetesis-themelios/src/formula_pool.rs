//! Bounded source-occurrence pools before ordinary formula compilation.
//!
//! An outer cursor selects each pooled literal independently, emitting complete
//! rules. Within a choice, selections instead add elements to the same group.
//! No constructor closure, textual substitution or upstream unpooling is used.
//! Pool-free alternatives retain the existing scalar/range admission contract.

mod local;
mod terms;
use local::{literal_width, select_comparison};
pub(super) use terms::distribute;

use themelios_base::span::Location;
use themelios_program::program::{
    Aggregate, Arguments, Atom, Body, BodyElement, Choice, ChoiceElement, Comparison, Condition,
    DefaultNegation, Disjunction, DisjunctionElement, FunctionAggregate, Guard, HasGuards, Head,
    HeadAggregate, Literal, LiteralInner, Program, SetAggregate, Statement,
};
use themelios_program::provenance::{TransformTag, WithProvenance};
use themelios_program::term::Term;
use themelios_program::transform::{Rewrite, Visit, rewrite};

use crate::diagnostic::unsupported;
use crate::expansion::Budget;
use crate::formula_ir::{Compiler, RuleIr};
use crate::{ExpansionResource, FormulaFailure, FormulaLimits, FormulaResource, ProfileFeature};

impl Compiler<'_> {
    /// Weak constraints have the same outer body-product law as rules. Local
    /// aggregate and conditional scopes remain untouched for their consumers.
    pub(super) fn body_alternatives(&mut self, body: &Body) -> Result<Vec<Body>, FormulaFailure> {
        let mut payload = Footprint::default();
        payload.visit_body(body);
        self.budget.charge(
            ExpansionResource::TermWork,
            payload.nodes.saturating_mul(2),
            self.location,
        )?;
        self.budget.charge(
            ExpansionResource::ScalarBytes,
            payload.bytes.saturating_mul(2),
            self.location,
        )?;
        let source = WithProvenance::constructed(Statement::Rule(
            themelios_program::program::Rule::new(Head::Falsum, body.clone()),
        ));
        let mut nodes = 0;
        let Some(mut cursor) = Cursor::new(
            &source,
            0,
            self.limits,
            &mut nodes,
            self.budget,
            self.location,
        )?
        else {
            return Ok(vec![body.clone()]);
        };
        let mut result = Vec::new();
        while let Some(statement) = cursor.next(self.budget, self.location)? {
            let Statement::Rule(rule) = statement.get() else {
                unreachable!("body cursor retains rule")
            };
            result.push(rule.body().get().clone());
        }
        Ok(result)
    }

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
        // Local atom pooling reconstructs choice element carriers. Boolean
        // literals and their conditions remain equal, so their exact source
        // keys are recovered from this same rule before that reconstruction.
        let choice_source = match statement.get() {
            Statement::Rule(rule) => match rule.head().get() {
                Head::Choice(choice) => Some(choice),
                _ => None,
            },
            _ => None,
        };
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
                rules.push(self.rule(rule, origins.to_vec(), choice_source)?);
                analyzed.extend(self.conditional_projection(&statement, projection_nodes)?);
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
            rules.push(self.rule(rule, origins.to_vec(), choice_source)?);
            analyzed.extend(self.conditional_projection(statement, projection_nodes)?);
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
            local_copies,
        } = shape(rule, scan.pools);
        // Local scopes validate and expand their own alternatives during IR
        // compilation. Only ordinary head/body occurrences drive this cursor.
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
            Program::of_nodes([self.source.clone()]),
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
    local_copies: u128,
}

fn shape(rule: &themelios_program::program::Rule, capacity: usize) -> Shape {
    let mut widths = Vec::with_capacity(capacity);
    let mut local_copies = 0_u128;
    match rule.head().get() {
        Head::Literal(literal) => {
            outer(literal, &mut widths);
        }
        Head::Disjunction(head) => {
            for element in head.elements() {
                if unconditional(element.get().condition()) {
                    outer(element.get().literal(), &mut widths);
                }
            }
        }
        Head::Choice(choice) => {
            guard_width(choice.left_guard(), &mut widths);
            guard_width(choice.right_guard(), &mut widths);
            for element in choice.elements() {
                if let LiteralInner::Atom(atom) = &element.get().literal().inner {
                    let (count, _) = atom_width(atom.get());
                    local_copies = local_copies.saturating_add(count.saturating_sub(1));
                }
            }
        }
        Head::Aggregate(aggregate) => {
            guard_width(aggregate.left_guard(), &mut widths);
            guard_width(aggregate.right_guard(), &mut widths);
        }
        _ => {}
    }
    for element in rule.body().get().elements() {
        if let BodyElement::Literal(literal) = element.get() {
            outer(literal, &mut widths);
        } else if let BodyElement::Aggregate { aggregate, .. } = element.get() {
            let guarded: &dyn HasGuards = match aggregate {
                Aggregate::Function(value) => value,
                Aggregate::Set(value) => value,
            };
            guard_width(guarded.left_guard(), &mut widths);
            guard_width(guarded.right_guard(), &mut widths);
        }
    }
    Shape {
        widths,
        local_copies,
    }
}

fn outer(literal: &Literal, widths: &mut Vec<u128>) {
    let (width, pools) = literal_width(literal);
    if pools != 0 {
        widths.push(width);
    }
}

fn unconditional(condition: &Condition) -> bool {
    condition.literals().all(|literal| {
        matches!(
            (&literal.get().inner, literal.get().negation),
            (
                LiteralInner::True,
                DefaultNegation::None | DefaultNegation::NotNot
            ) | (LiteralInner::False, DefaultNegation::Not)
        )
    })
}

fn guard_width(guard: Option<&WithProvenance<Guard>>, widths: &mut Vec<u128>) {
    if let Some(guard) = guard
        && matches!(guard.get().term, Term::Pool(_))
    {
        widths.push(term_width(&guard.get().term) as u128);
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
impl Select<'_> {
    fn guard(&mut self, guard: Option<&WithProvenance<Guard>>) -> Option<Guard> {
        guard.map(|guard| {
            let mut guard = guard.get().clone();
            if let Term::Pool(items) = &guard.term {
                guard.term =
                    items[*self.positions.next().expect("guard occurrence position")].clone();
            }
            guard
        })
    }
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
        let pooled = std::iter::once(comparison.first())
            .chain(comparison.steps().map(|(_, term)| term))
            .any(|term| matches!(term, Term::Pool(_)));
        if pooled {
            select_comparison(
                &comparison,
                *self
                    .positions
                    .next()
                    .expect("comparison occurrence position"),
            )
        } else {
            comparison
        }
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
            BodyElement::Aggregate {
                negation,
                aggregate,
            } => {
                let aggregate = match aggregate {
                    Aggregate::Function(value) => Aggregate::Function(FunctionAggregate::new(
                        self.selector.guard(value.left_guard()),
                        value.function(),
                        value.elements().map(|element| element.get().clone()),
                        self.selector.guard(value.right_guard()),
                    )),
                    Aggregate::Set(value) => Aggregate::Set(SetAggregate::new(
                        self.selector.guard(value.left_guard()),
                        value.elements().map(|element| element.get().clone()),
                        self.selector.guard(value.right_guard()),
                    )),
                };
                BodyElement::Aggregate {
                    negation: *negation,
                    aggregate,
                }
            }
            other => other.clone(),
        }))
    }
    fn rewrite_head(&mut self, head: Head) -> Head {
        if let Head::Disjunction(disjunction) = head {
            return Head::Disjunction(Disjunction::new(disjunction.elements().map(|element| {
                let literal = element.get().literal();
                DisjunctionElement::new(
                    if unconditional(element.get().condition()) {
                        self.selector.rewrite_literal(literal.clone())
                    } else {
                        literal.clone()
                    },
                    element.get().condition().clone(),
                )
            })));
        }
        if let Head::Aggregate(value) = head {
            return Head::Aggregate(HeadAggregate::new(
                self.selector.guard(value.left_guard()),
                value.function(),
                value.elements().map(|element| element.get().clone()),
                self.selector.guard(value.right_guard()),
            ));
        }
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
            self.selector.guard(choice.left_guard()),
            elements,
            self.selector.guard(choice.right_guard()),
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
