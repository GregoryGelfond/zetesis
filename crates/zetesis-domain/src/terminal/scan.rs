//! Complete predicate occurrences; program node types are grammar-bounded.

use themelios_program::program::{
    Aggregate, Atom, BodyElement, Condition, Head, Literal, LiteralInner, SetElement, Statement,
};
use themelios_program::symbol::Sign;

use super::{Engine, Failure, Signature, UnknownReason};
use crate::Context;

impl<'p> Engine<'p> {
    pub(super) fn scan(&mut self) -> Result<(), Failure> {
        for part in self.result.program.parts() {
            self.result.context = Some(Context::Part(part));
            self.work()?;
            self.bytes(part.key().name.as_str().len())?;
            if part.key().name.as_str() != "base" || !part.key().formals.is_empty() {
                return Err(unknown(crate::UnknownReason::ProgramPart));
            }
            for source in part.statements() {
                self.result.context = Some(Context::Statement(source));
                self.work()?;
                match source.get() {
                    Statement::Rule(rule) => {
                        let qualified = self.flat(rule)?;
                        self.head(rule.head().get(), qualified, source)?;
                        for element in rule.body().get().elements() {
                            self.body(element.get())?;
                        }
                    }
                    Statement::Show(_) | Statement::Defined(_) => {}
                    Statement::Optimize(_) | Statement::WeakConstraint(_) => {
                        return Err(Failure::Unknown(UnknownReason::Objectives));
                    }
                    Statement::Project(_) => {
                        return Err(Failure::Unknown(UnknownReason::Projection));
                    }
                    Statement::Const(_) => return Err(unknown(crate::UnknownReason::Constants)),
                    Statement::Include(_) => return Err(unknown(crate::UnknownReason::Include)),
                    Statement::External(_) => return Err(unknown(crate::UnknownReason::External)),
                    Statement::Script(_) => return Err(unknown(crate::UnknownReason::Script)),
                    Statement::TheoryDefinition(_) => {
                        return Err(unknown(crate::UnknownReason::Theory));
                    }
                    _ => return Err(unknown(crate::UnknownReason::Statement)),
                }
            }
        }
        self.select()
    }

    fn select(&mut self) -> Result<(), Failure> {
        for index in 0..self.candidates.len() {
            let (signature, source) = self.candidates[index];
            self.result.context = Some(Context::Statement(source));
            self.work()?;
            self.bytes(signature.name.len())?;
            let usage = &self.signatures[&signature];
            if usage.read || !usage.qualified {
                continue;
            }
            self.work()?;
            self.bytes(signature.name.len())?;
            let complement = Signature {
                sign: Sign::Negative,
                ..signature
            };
            if !self.signatures.contains_key(&complement) {
                self.link()?;
                self.result.definitions.push(source);
            }
        }
        Ok(())
    }

    fn head(
        &mut self,
        head: &'p Head,
        qualified: bool,
        source: &'p themelios_program::provenance::WithProvenance<Statement>,
    ) -> Result<(), Failure> {
        self.work()?;
        match head {
            Head::Literal(literal) => {
                if let LiteralInner::Atom(atom) = &literal.inner {
                    for arguments in atom.get().alternatives() {
                        let signature = self.producer(atom.get(), arguments.len(), qualified)?;
                        if qualified {
                            self.link()?;
                            self.candidates.push((signature, source));
                        }
                    }
                }
            }
            Head::Choice(choice) => {
                for element in choice.elements() {
                    self.head_element(element.get().literal(), element.get().condition())?;
                }
            }
            Head::Disjunction(disjunction) => {
                for element in disjunction.elements() {
                    self.head_element(element.get().literal(), element.get().condition())?;
                }
            }
            Head::Aggregate(aggregate) => {
                for element in aggregate.elements() {
                    self.head_element(element.get().literal(), element.get().condition())?;
                }
            }
            Head::TheoryAtom(_) => return Err(unknown(crate::UnknownReason::Theory)),
            Head::Falsum | Head::Verum => {}
        }
        Ok(())
    }

    fn producer(
        &mut self,
        atom: &'p Atom,
        arity: usize,
        qualified: bool,
    ) -> Result<Signature<'p>, Failure> {
        self.link()?;
        let signature = self.signature(atom, arity)?;
        self.signatures
            .get_mut(&signature)
            .expect("registered signature")
            .qualified &= qualified;
        Ok(signature)
    }

    fn head_element(
        &mut self,
        literal: &'p Literal,
        condition: &'p Condition,
    ) -> Result<(), Failure> {
        self.work()?;
        if let LiteralInner::Atom(atom) = &literal.inner {
            for arguments in atom.get().alternatives() {
                self.producer(atom.get(), arguments.len(), false)?;
            }
        }
        self.condition(condition)
    }

    fn condition(&mut self, condition: &'p Condition) -> Result<(), Failure> {
        for literal in condition.literals() {
            self.read(literal.get())?;
        }
        Ok(())
    }

    fn read(&mut self, literal: &'p Literal) -> Result<(), Failure> {
        self.work()?;
        if let LiteralInner::Atom(atom) = &literal.inner {
            for arguments in atom.get().alternatives() {
                self.link()?;
                let signature = self.signature(atom.get(), arguments.len())?;
                self.signatures
                    .get_mut(&signature)
                    .expect("registered signature")
                    .read = true;
            }
        }
        Ok(())
    }

    fn body(&mut self, element: &'p BodyElement) -> Result<(), Failure> {
        self.work()?;
        match element {
            BodyElement::Literal(literal) => self.read(literal),
            BodyElement::Conditional(conditional) => {
                self.read(&conditional.literal)?;
                self.condition(&conditional.condition)
            }
            BodyElement::Aggregate {
                aggregate: Aggregate::Function(aggregate),
                ..
            } => {
                for element in aggregate.elements() {
                    self.work()?;
                    self.condition(element.get().condition())?;
                }
                Ok(())
            }
            BodyElement::Aggregate {
                aggregate: Aggregate::Set(aggregate),
                ..
            } => {
                for element in aggregate.elements() {
                    self.work()?;
                    match element.get() {
                        SetElement::Literal(literal) => self.read(literal)?,
                        SetElement::ConditionalLiteral(conditional) => {
                            self.read(&conditional.literal)?;
                            self.condition(&conditional.condition)?;
                        }
                    }
                }
                Ok(())
            }
            BodyElement::TheoryAtom { .. } => Err(unknown(crate::UnknownReason::Theory)),
            _ => Err(unknown(crate::UnknownReason::Statement)),
        }
    }
}

fn unknown(reason: crate::UnknownReason) -> Failure {
    Failure::Unknown(UnknownReason::Source(reason))
}
