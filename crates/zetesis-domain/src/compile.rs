//! Direct statement inspection; no unpooling, source parsing, or term evaluation.

use themelios_program::program::{
    Arguments, Atom, BodyElement, DefaultNegation, Head, Literal, LiteralInner, Rule, Statement,
};
use themelios_program::provenance::WithProvenance;
use themelios_program::symbol::Signature;
use themelios_program::term::{Term, Variable};

use crate::analysis::{Engine, Failure, Producer, Transfer, empty_argument};
use crate::limits::check;
use crate::{Context, Resource, UnknownReason, Widening};

impl<'p> Engine<'p> {
    pub fn compile(&mut self) -> Result<(), Failure> {
        for part in self.result.program.parts() {
            self.result.context = Some(Context::Part(part));
            self.work()?;
            if part.key().name.as_str() != "base" || !part.key().formals.is_empty() {
                return Err(Failure::Unknown(UnknownReason::ProgramPart));
            }
            for source in part.statements() {
                self.result.context = Some(Context::Statement(source));
                self.work()?;
                match source.get() {
                    Statement::Rule(rule) => self.rule(rule, source)?,
                    Statement::Const(_) => return Err(Failure::Unknown(UnknownReason::Constants)),
                    Statement::Include(_) => return Err(Failure::Unknown(UnknownReason::Include)),
                    Statement::External(_) => {
                        return Err(Failure::Unknown(UnknownReason::External));
                    }
                    Statement::TheoryDefinition(_) => {
                        return Err(Failure::Unknown(UnknownReason::Theory));
                    }
                    Statement::Script(_) => return Err(Failure::Unknown(UnknownReason::Script)),
                    Statement::Optimize(_)
                    | Statement::WeakConstraint(_)
                    | Statement::Show(_)
                    | Statement::Project(_)
                    | Statement::Defined(_)
                    | Statement::Edge(_)
                    | Statement::Heuristic(_)
                    | Statement::Query(_) => {}
                    _ => return Err(Failure::Unknown(UnknownReason::Statement)),
                }
            }
        }
        Ok(())
    }
    fn register(&mut self, atom: &Atom, arity: usize) -> Result<usize, Failure> {
        self.work()?;
        self.bytes(atom.name.as_str().len())?;
        let Ok(arity32) = u32::try_from(arity) else {
            return Err(Failure::Unknown(UnknownReason::Head));
        };
        let signature = Signature {
            sign: atom.sign,
            name: atom.name.clone(),
            arity: arity32,
        };
        if let Some(&start) = self.result.signatures.get(&signature) {
            return Ok(start);
        }
        let stats = &self.result.statistics;
        check(
            Resource::Predicates,
            stats.predicates as u128 + 1,
            self.limits.max_predicates as u128,
        )?;
        check(
            Resource::Positions,
            stats.positions as u128 + arity as u128,
            self.limits.max_positions as u128,
        )?;
        let start = self.result.arguments.len();
        self.result
            .arguments
            .extend((0..arity).map(|_| empty_argument()));
        self.result.signatures.insert(signature, start);
        self.result.statistics.predicates += 1;
        self.result.statistics.positions += arity;
        Ok(start)
    }
    fn rule(
        &mut self,
        rule: &'p Rule,
        source: &'p WithProvenance<Statement>,
    ) -> Result<(), Failure> {
        match rule.head().get() {
            Head::Literal(literal) => self.head(literal, rule, source, false),
            Head::Choice(choice) => {
                for element in choice.elements() {
                    self.work()?;
                    self.head(element.get().literal(), rule, source, true)?;
                }
                Ok(())
            }
            Head::Disjunction(disjunction) => {
                for element in disjunction.elements() {
                    self.work()?;
                    self.head(element.get().literal(), rule, source, true)?;
                }
                Ok(())
            }
            Head::Falsum | Head::Verum => Ok(()),
            Head::TheoryAtom(_) => Err(Failure::Unknown(UnknownReason::Theory)),
            Head::Aggregate(_) => Err(Failure::Unknown(UnknownReason::Head)),
        }
    }
    fn head(
        &mut self,
        literal: &'p Literal,
        rule: &'p Rule,
        source: &'p WithProvenance<Statement>,
        uncertain: bool,
    ) -> Result<(), Failure> {
        let LiteralInner::Atom(atom) = &literal.inner else {
            return Err(Failure::Unknown(UnknownReason::Head));
        };
        let atom = atom.get();
        for terms in atom.alternatives() {
            self.work()?;
            let start = self.register(atom, terms.len())?;
            for (index, term) in terms.iter().enumerate() {
                self.work()?;
                self.links(2)?;
                let transfer = if uncertain || literal.negation != DefaultNegation::None {
                    Transfer::Unknown(Widening::HeadForm)
                } else if atom.is_pooled() {
                    Transfer::Unknown(Widening::HeadTerm)
                } else {
                    match term {
                        Term::Symbolic(value) if self.symbol(value)? => Transfer::Value(value),
                        Term::Symbolic(_) => Transfer::Unknown(Widening::SymbolSize),
                        Term::Variable(Variable::Named(variable)) => {
                            self.bytes(variable.as_str().len())?;
                            let inputs = self.inputs(variable, rule)?;
                            if inputs.is_empty() {
                                Transfer::Unknown(Widening::UnboundVariable)
                            } else {
                                Transfer::Intersection(inputs)
                            }
                        }
                        _ => Transfer::Unknown(Widening::HeadTerm),
                    }
                };
                self.result.arguments[start + index].producers.push(source);
                self.producers.push(Producer {
                    target: start + index,
                    transfer,
                    source,
                });
            }
        }
        Ok(())
    }
    fn inputs(
        &mut self,
        variable: &themelios_program::symbol::VarName,
        rule: &'p Rule,
    ) -> Result<Vec<usize>, Failure> {
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
                        let start = self.register(atom, terms.len())?;
                        self.links(1)?;
                        inputs.push(start + index);
                    }
                }
            }
        }
        Ok(inputs)
    }
}
