//! Single-valued argument consumers share head binding and scalar evaluation.

use themelios_program::program::{Arguments, DefaultNegation, Literal, LiteralInner};
use themelios_program::term::Term;

use crate::diagnostic::unsupported;
use crate::formula_ir::{Compiler, LiteralIr, Operation, Variables};
use crate::{ExpansionResource, FormulaFailure, ProfileFeature};

impl Compiler<'_> {
    /// Positive atoms may capture evaluated positions for a later equality check.
    /// Negative atoms consume independently established bindings. Neither check
    /// nor fresh data slot turns an expression input into a relational producer.
    pub(super) fn literal_into(
        &mut self,
        literal: &Literal,
        variables: &mut Variables,
        body: &mut Vec<LiteralIr>,
    ) -> Result<(), FormulaFailure> {
        if literal.negation == DefaultNegation::None
            && let LiteralInner::Atom(atom) = &literal.inner
            && self.positive_literal(atom.get(), variables, body)?
        {
            return Ok(());
        }
        if literal.negation != DefaultNegation::None
            && let LiteralInner::Atom(atom) = &literal.inner
            && let Arguments::Single(arguments) = &atom.get().arguments
            && arguments
                .iter()
                .any(|term| !matches!(term, Term::Variable(_) | Term::Symbolic(_)))
        {
            // This door is deterministic: intervals/pools need their own family
            // semantics and analyzed projection, and remain located refusals.
            for term in arguments.iter().flat_map(Term::subterms) {
                self.budget
                    .charge(ExpansionResource::TermWork, 1, self.location)?;
                if matches!(term, Term::Pool(_) | Term::Interval { .. }) {
                    return Err(unsupported(ProfileFeature::Term, self.location).into());
                }
            }
            self.generated_arguments(arguments, variables)?;
            let pattern = self.generated_atom(atom.get(), variables, body)?;
            body.push(LiteralIr::Atom(literal.negation, pattern));
        } else {
            body.push(self.literal(literal, variables)?);
        }
        Ok(())
    }

    /// Bound the owned source copy and flat-plan payload before cloning the AST.
    /// This accounting is separate from each evaluated value's construction cap.
    pub(super) fn value_plan_preflight(&mut self, term: &Term) -> Result<(), FormulaFailure> {
        // Scalar-only expressions keep their established charging/error order.
        // The bounded source traversal only selects this newly admitted plan
        // category; construction plans then charge their complete scan and copy.
        if !term
            .subterms()
            .any(|node| matches!(node, Term::Function { .. } | Term::Tuple(_)))
        {
            return Ok(());
        }
        let mut bytes = 0_u128;
        for node in term.subterms() {
            self.budget
                .charge(ExpansionResource::TermWork, 1, self.location)?;
            bytes += (std::mem::size_of::<Term>() + std::mem::size_of::<Operation>()) as u128;
            bytes += match node {
                Term::Function { name, arguments } => {
                    std::mem::size_of::<crate::formula_value::Constructor>() as u128
                        + name.as_str().len() as u128 * 2
                        + arguments.len() as u128 * std::mem::size_of::<usize>() as u128
                }
                Term::Tuple(arguments) => {
                    std::mem::size_of::<crate::formula_value::Constructor>() as u128
                        + arguments.len() as u128 * std::mem::size_of::<usize>() as u128
                }
                Term::Symbolic(symbol) => crate::structural_value::symbol_bytes(symbol),
                Term::Variable(themelios_program::term::Variable::Named(name)) => {
                    name.as_str().len() as u128
                }
                _ => 0,
            };
        }
        self.budget
            .charge(ExpansionResource::ScalarBytes, bytes, self.location)?;
        Ok(())
    }
}
