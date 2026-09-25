//! An atom pool offers alternative relations, each with its own captures.
//!
//! Only captures available in every alternative may supply another body element.
//! Evaluated positive arguments read the row after all structural captures exist;
//! default-negated tests expand first and negate each complete atom alternative.

mod pools;

use super::{
    Arguments, AtomTest, BTreeSet, Compiler, Error, Name, Operand, Pattern, Query, Sign, Term,
    UnaryOp, Variable,
};
use themelios_program::program::Atom;

pub(super) fn captures(operand: &Operand, slots: &mut BTreeSet<usize>) {
    match operand {
        Operand::Variable(slot) | Operand::Inverse { slot, .. } => {
            slots.insert(*slot);
        }
        Operand::Construct(_, arguments) => {
            for argument in arguments {
                captures(argument, slots);
            }
        }
        _ => {}
    }
}
fn provided(pattern: &Pattern) -> BTreeSet<usize> {
    let mut slots = BTreeSet::new();
    slots.extend(pattern.key);
    for term in &pattern.terms {
        captures(term, &mut slots);
    }
    slots
}
fn evaluated(operand: &Operand) -> bool {
    match operand {
        Operand::Expression(_) | Operand::Inverse { .. } => true,
        Operand::Construct(_, arguments) => arguments.iter().any(evaluated),
        _ => false,
    }
}
impl Compiler<'_> {
    fn operand(&mut self, term: &Term, bind: bool, depth: usize) -> Result<Operand, Error> {
        self.node(depth)?;
        Ok(match term {
            Term::Variable(Variable::Anonymous) => Operand::Any,
            Term::Variable(variable) => Operand::Variable(self.variable(variable)?),
            Term::Symbolic(symbol) => Operand::Constant(self.symbol(symbol, depth, true)?),
            Term::Function { name, arguments } => {
                self.operand_function(name, arguments, bind, depth, Sign::Positive)?
            }
            Term::UnaryOperation {
                operator: UnaryOp::Negate,
                argument,
            } if matches!(argument.as_ref(), Term::Function { .. }) => {
                let Term::Function { name, arguments } = argument.as_ref() else {
                    unreachable!()
                };
                self.operand_function(name, arguments, bind, depth + 1, Sign::Negative)?
            }
            Term::Tuple(arguments) => {
                self.arity(arguments.len())?;
                let mut values = Vec::new();
                for argument in arguments {
                    values.push(self.operand(argument, bind, depth + 1)?);
                }
                Operand::Construct(self.shape(None, Sign::Positive, values.len())?, values)
            }
            _ => {
                let expression = self.template(term, depth)?;
                Operand::Expression(if bind {
                    expression
                } else {
                    self.lift(expression)?
                })
            }
        })
    }
    fn operand_function(
        &mut self,
        name: &Name,
        arguments: &[Term],
        bind: bool,
        depth: usize,
        sign: Sign,
    ) -> Result<Operand, Error> {
        self.text(name.as_str())?;
        self.arity(arguments.len())?;
        let mut values = Vec::new();
        for argument in arguments {
            values.push(self.operand(argument, bind, depth + 1)?);
        }
        Ok(Operand::Construct(
            self.shape(Some(name.as_str()), sign, values.len())?,
            values,
        ))
    }

    pub(super) fn pattern_terms(
        &mut self,
        atom: &Atom,
        arguments: &[Term],
        bind: bool,
    ) -> Result<Pattern, Error> {
        self.text(atom.name.as_str())?;
        self.arity(arguments.len())?;
        let mut terms = Vec::new();
        for argument in arguments {
            terms.push(self.operand(argument, bind, 1)?);
        }
        if bind {
            self.prepare_inverses(&mut terms);
        }
        let predicate = self.signature(atom.name.as_str(), terms.len(), atom.sign)?;
        Ok(Pattern {
            predicate,
            evaluated: terms.iter().any(evaluated),
            terms,
            key: None,
        })
    }
    pub(super) fn patterns(&mut self, atom: &Atom) -> Result<Vec<Pattern>, Error> {
        let previous = self.pool_binding;
        self.pool_binding = matches!(atom.arguments, Arguments::Pooled(_));
        let result = (|| {
            let mut patterns = Vec::new();
            for arguments in atom.alternatives() {
                self.node(1)?;
                patterns.extend(self.pattern_variants(atom, arguments)?);
            }
            Ok(patterns)
        })();
        self.pool_binding = previous;
        result
    }
    pub(super) fn atom_test(&mut self, atom: &Atom, arguments: &[Term]) -> Result<AtomTest, Error> {
        let outer = std::mem::take(&mut self.generated);
        let pattern = self.pattern_terms(atom, arguments, false);
        let generated = std::mem::replace(&mut self.generated, outer);
        for (slot, _) in &generated {
            self.used.remove(slot);
        }
        Ok(AtomTest {
            pattern: pattern?,
            expansion: Query {
                binders: generated
                    .into_iter()
                    .map(|(slot, term)| term.binder(slot))
                    .collect(),
                conditions: Vec::new(),
                variables: self.slots,
                inputs: Vec::new(),
            },
        })
    }
    pub(super) fn atom_tests(&mut self, atom: &Atom) -> Result<Vec<AtomTest>, Error> {
        let mut tests = Vec::new();
        for arguments in atom.alternatives() {
            self.node(1)?;
            tests.push(self.atom_test(atom, arguments)?);
        }
        Ok(tests)
    }
    fn operand_ready(&self, operand: &Operand, captures: &BTreeSet<usize>) -> bool {
        match operand {
            Operand::Expression(expression) => self.ready_with(expression, captures),
            Operand::Construct(_, arguments) => arguments
                .iter()
                .all(|argument| self.operand_ready(argument, captures)),
            _ => true,
        }
    }
    pub(super) fn prepare_inverses(&self, terms: &mut [Operand]) {
        let mut available = BTreeSet::new();
        for term in terms.iter() {
            captures(term, &mut available);
        }
        self.inverse_operands(terms, &available);
    }
    pub(super) fn patterns_ready(&self, patterns: &[Pattern]) -> bool {
        patterns.iter().all(|pattern| {
            let captures = provided(pattern);
            pattern
                .terms
                .iter()
                .all(|term| self.operand_ready(term, &captures))
        })
    }
    pub(super) fn common_captures(patterns: &[Pattern]) -> BTreeSet<usize> {
        let Some(first) = patterns.first() else {
            return BTreeSet::new();
        };
        let mut common = provided(first);
        for pattern in &patterns[1..] {
            common = common.intersection(&provided(pattern)).copied().collect();
        }
        common
    }
}
