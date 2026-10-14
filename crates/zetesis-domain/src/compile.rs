//! Direct statement inspection; no unpooling, source parsing, or term evaluation.

use themelios_program::program::{
    Arguments, Atom, BodyElement, DefaultNegation, Head, Literal, LiteralInner, Rule, Statement,
};
use themelios_program::symbol::Signature;
use themelios_program::term::{Term, Variable};

use crate::analysis::Symbols;
use crate::numeric::{self, ArgumentId, Binding, Error, Expression, Input, PredicateId};
use crate::{Context, UnknownReason, Widening};
use std::collections::BTreeMap;
type Failure = Error<UnknownReason>;
pub(super) struct Compiler<'p, 'i, 'r> {
    pub program: &'p themelios_program::program::Program,
    pub input: &'i mut Input<'r, Symbols<'p>, Context<'p>>,
    pub signatures: &'i mut BTreeMap<Signature, (PredicateId, usize)>,
    pub positions: usize,
}

impl<'p> Compiler<'p, '_, '_> {
    pub(crate) fn compile(&mut self) -> Result<(), Failure> {
        for part in self.program.parts() {
            self.input.context(Context::Part(part));
            self.work()?;
            if part.key().name.as_str() != "base" || !part.key().formals.is_empty() {
                return Err(Error::Read(UnknownReason::ProgramPart));
            }
            for source in part.statements() {
                self.input.context(Context::Statement(source));
                self.work()?;
                match source.get() {
                    Statement::Rule(rule) => self.rule(rule)?,
                    Statement::Const(_) => return Err(Error::Read(UnknownReason::Constants)),
                    Statement::Include(_) => return Err(Error::Read(UnknownReason::Include)),
                    Statement::External(_) => {
                        return Err(Error::Read(UnknownReason::External));
                    }
                    Statement::TheoryDefinition(_) => {
                        return Err(Error::Read(UnknownReason::Theory));
                    }
                    Statement::Script(_) => return Err(Error::Read(UnknownReason::Script)),
                    Statement::Optimize(_)
                    | Statement::WeakConstraint(_)
                    | Statement::Show(_)
                    | Statement::Project(_)
                    | Statement::Defined(_)
                    | Statement::Edge(_)
                    | Statement::Heuristic(_)
                    | Statement::Query(_) => {}
                    _ => return Err(Error::Read(UnknownReason::Statement)),
                }
            }
        }
        Ok(())
    }
    fn work(&mut self) -> Result<(), Failure> {
        self.input.work(1)
    }
    fn bytes(&mut self, bytes: usize) -> Result<(), Failure> {
        self.input
            .bytes(u64::try_from(bytes).map_err(|_| Error::InvalidInput)?)
    }
    fn register(&mut self, atom: &Atom, arity: usize) -> Result<PredicateId, Failure> {
        self.work()?;
        self.bytes(atom.name.as_str().len())?;
        let arity32 = u32::try_from(arity).map_err(|_| Error::Read(UnknownReason::Head))?;
        let signature = Signature {
            sign: atom.sign,
            name: atom.name.clone(),
            arity: arity32,
        };
        if let Some(&(id, _)) = self.signatures.get(&signature) {
            return Ok(id);
        }
        let id = self.input.predicate(arity)?;
        self.signatures.insert(signature, (id, self.positions));
        self.positions += arity;
        Ok(id)
    }
    fn rule(&mut self, rule: &'p Rule) -> Result<(), Failure> {
        match rule.head().get() {
            Head::Literal(literal) => self.head(literal, rule, false),
            Head::Choice(choice) => {
                for element in choice.elements() {
                    self.work()?;
                    self.head(element.get().literal(), rule, true)?;
                }
                Ok(())
            }
            Head::Disjunction(disjunction) => {
                for element in disjunction.elements() {
                    self.work()?;
                    self.head(element.get().literal(), rule, true)?;
                }
                Ok(())
            }
            Head::Falsum | Head::Verum => Ok(()),
            Head::TheoryAtom(_) => Err(Error::Read(UnknownReason::Theory)),
            Head::Aggregate(_) => Err(Error::Read(UnknownReason::Head)),
        }
    }
    fn head(
        &mut self,
        literal: &'p Literal,
        rule: &'p Rule,
        uncertain: bool,
    ) -> Result<(), Failure> {
        let LiteralInner::Atom(atom) = &literal.inner else {
            return Err(Error::Read(UnknownReason::Head));
        };
        let atom = atom.get();
        for terms in atom.alternatives() {
            self.work()?;
            let predicate = self.register(atom, terms.len())?;
            for (index, term) in terms.iter().enumerate() {
                self.work()?;
                let argument = self.input.argument(predicate, index)?;
                if !uncertain
                    && literal.negation == DefaultNegation::None
                    && !atom.is_pooled()
                    && rule.body().get().is_empty()
                    && let Term::Symbolic(value) = term
                {
                    let value = self.input.constant(value)?;
                    self.input.seed_value(argument, value)?;
                    continue;
                }
                let mut plan = numeric::Rule::default();
                let unknown = |reason| Expression::Unknown {
                    integer: false,
                    reason,
                };
                let value = if uncertain || literal.negation != DefaultNegation::None {
                    unknown(Widening::HeadForm)
                } else if atom.is_pooled() {
                    unknown(Widening::HeadTerm)
                } else {
                    match term {
                        Term::Symbolic(value) => Expression::Constant(self.input.constant(value)?),
                        Term::Variable(Variable::Named(variable)) => {
                            self.bytes(variable.as_str().len())?;
                            let inputs = self.inputs(variable, rule)?;
                            if inputs.is_empty() {
                                unknown(Widening::UnboundVariable)
                            } else {
                                plan.slots = 1;
                                plan.bindings = inputs
                                    .into_iter()
                                    .map(|argument| Binding::Positive { slot: 0, argument })
                                    .collect();
                                Expression::Slot(0)
                            }
                        }
                        _ => unknown(Widening::HeadTerm),
                    }
                };
                let value = self.input.expression(value)?;
                plan.heads.push(numeric::Head { argument, value });
                self.input.rule(plan)?;
            }
        }
        Ok(())
    }
    fn inputs(
        &mut self,
        variable: &themelios_program::symbol::VarName,
        rule: &'p Rule,
    ) -> Result<Vec<ArgumentId>, Failure> {
        let mut inputs = Vec::new();
        for element in rule.body().get().elements() {
            self.work()?;
            let BodyElement::Literal(literal) = element.get() else {
                continue;
            };
            if literal.negation != DefaultNegation::None {
                continue;
            }
            let LiteralInner::Atom(atom) = &literal.inner else {
                continue;
            };
            let atom = atom.get();
            let Arguments::Single(terms) = &atom.arguments else {
                continue;
            };
            for (index, term) in terms.iter().enumerate() {
                self.work()?;
                if let Term::Variable(Variable::Named(bound)) = term {
                    self.bytes(bound.as_str().len())?;
                    if bound == variable {
                        let predicate = self.register(atom, terms.len())?;
                        inputs.try_reserve(1)?;
                        inputs.push(self.input.argument(predicate, index)?);
                    }
                }
            }
        }
        Ok(inputs)
    }
}
