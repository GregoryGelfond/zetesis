//! One bounded Cartesian occurrence cursor, composed inside a caller-owned scope.

use themelios_program::program::{Comparison, Condition, Literal, LiteralInner};
use themelios_program::term::Term;
use themelios_program::transform::Visit;

use super::{Footprint, atom_width, select_atom, term_width};
use crate::formula_ir::Compiler;
use crate::{ExpansionResource, FormulaFailure};

impl Compiler<'_> {
    pub(crate) fn literal_alternatives(
        &mut self,
        literal: &Literal,
    ) -> Result<Vec<Literal>, FormulaFailure> {
        Ok(self
            .condition_alternatives(&Condition::new([literal.clone()]))?
            .into_iter()
            .map(|condition| {
                condition
                    .literals()
                    .next()
                    .expect("one selected literal")
                    .get()
                    .clone()
            })
            .collect())
    }

    /// Local condition alternatives retain independent variable environments.
    pub(crate) fn condition_alternatives(
        &mut self,
        condition: &Condition,
    ) -> Result<Vec<Condition>, FormulaFailure> {
        Ok(self
            .local_alternatives(&[], condition)?
            .into_iter()
            .map(|(_, condition)| condition)
            .collect())
    }

    /// Fields and conditions form a product within one original element. The
    /// caller decides whether those occurrences contribute a union, conjunction
    /// or separate rules; this cursor never chooses that semantic composition.
    pub(crate) fn local_alternatives(
        &mut self,
        fields: &[&Term],
        condition: &Condition,
    ) -> Result<Vec<(Vec<Term>, Condition)>, FormulaFailure> {
        let mut scan = Footprint::default();
        for term in fields.iter().flat_map(|term| term.subterms()) {
            scan.visit_term(term);
        }
        scan.visit_condition(condition);
        let count = fields
            .iter()
            .fold(1_u128, |n, term| n.saturating_mul(term_width(term) as u128));
        let count = condition.literals().fold(count, |n, literal| {
            n.saturating_mul(literal_width(literal.get()).0)
        });
        self.budget
            .charge(ExpansionResource::TermWork, scan.nodes, self.location)?;
        self.budget
            .charge(ExpansionResource::Values, count, self.location)?;
        self.budget.charge(
            ExpansionResource::TermWork,
            count.saturating_mul(scan.nodes + 1),
            self.location,
        )?;
        let bytes = count.saturating_mul(
            scan.bytes.saturating_mul(4)
                + std::mem::size_of::<Condition>() as u128
                + fields.len() as u128 * std::mem::size_of::<Term>() as u128,
        );
        self.budget
            .charge(ExpansionResource::ScalarBytes, bytes, self.location)?;
        self.budget.check_family(
            bytes.saturating_add(count.saturating_mul(std::mem::size_of::<Vec<Term>>() as u128)),
            self.location,
        )?;
        let count = usize::try_from(count).expect("finite value allowance bounds occurrences");
        let mut result = Vec::with_capacity(count);
        for mut position in 0..count {
            self.budget.poll(self.location)?;
            let mut stride = count;
            let mut terms = Vec::with_capacity(fields.len());
            for term in fields {
                let width = term_width(term);
                stride /= width;
                terms.push(select_term(term, position / stride));
                position %= stride;
            }
            let condition = Condition::new(condition.literals().map(|literal| {
                let width =
                    usize::try_from(literal_width(literal.get()).0).expect("bounded literal width");
                stride /= width;
                let selected = select_literal(literal.get(), position / stride);
                position %= stride;
                selected
            }));
            result.push((terms, condition));
        }
        Ok(result)
    }
}

pub(super) fn literal_width(literal: &Literal) -> (u128, usize) {
    match &literal.inner {
        LiteralInner::Atom(atom) => atom_width(atom.get()),
        LiteralInner::Comparison(comparison) => std::iter::once(comparison.get().first())
            .chain(comparison.get().steps().map(|(_, term)| term))
            .fold((1_u128, 0), |(width, pools), term| {
                (
                    width.saturating_mul(term_width(term) as u128),
                    pools + usize::from(matches!(term, Term::Pool(_))),
                )
            }),
        LiteralInner::True | LiteralInner::False => (1, 0),
    }
}

pub(super) fn select_literal(literal: &Literal, position: usize) -> Literal {
    Literal {
        negation: literal.negation,
        inner: match &literal.inner {
            LiteralInner::Atom(atom) => {
                LiteralInner::Atom(atom.clone().map(|atom| select_atom(atom, position)))
            }
            LiteralInner::Comparison(comparison) => LiteralInner::Comparison(
                comparison
                    .clone()
                    .map(|comparison| select_comparison(&comparison, position)),
            ),
            other => other.clone(),
        },
    }
}

pub(super) fn select_comparison(comparison: &Comparison, mut position: usize) -> Comparison {
    let mut select = |term: &Term| {
        let width = term_width(term);
        let term = select_term(term, position % width);
        position /= width;
        term
    };
    let first = select(comparison.first());
    let mut steps = comparison.steps();
    let (relation, term) = steps.next().expect("comparison has a step");
    let mut result = Comparison::new(first, relation, select(term));
    for (relation, term) in steps {
        result = result.chain(relation, select(term));
    }
    result
}

fn select_term(term: &Term, position: usize) -> Term {
    if let Term::Pool(items) = term {
        items[position].clone()
    } else {
        term.clone()
    }
}
