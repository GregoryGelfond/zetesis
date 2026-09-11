//! An atom pool offers alternative relations, each with its own captures.
//!
//! Only captures available in every alternative may supply another body element.
//! Evaluated positive arguments read the row after all structural captures exist;
//! default-negated tests expand first and negate each complete atom alternative.

use super::{
    Arguments, AtomTest, BTreeSet, Binder, Compiler, Error, ErrorKind, Feature, Name, Operand,
    Pattern, Predicate, Query, Sign, Term, UnaryOp, Variable,
};
use themelios_program::program::Atom;

fn captures(operand: &Operand, slots: &mut BTreeSet<usize>) {
    match operand {
        Operand::Variable(slot) => {
            slots.insert(*slot);
        }
        Operand::Function(_, _, arguments) | Operand::Tuple(arguments) => {
            for argument in arguments {
                captures(argument, slots);
            }
        }
        _ => {}
    }
}
fn provided(pattern: &Pattern) -> BTreeSet<usize> {
    let mut slots = BTreeSet::new();
    for term in &pattern.terms {
        captures(term, &mut slots);
    }
    slots
}
fn evaluated(operand: &Operand) -> bool {
    match operand {
        Operand::Expression(_) => true,
        Operand::Function(_, _, arguments) | Operand::Tuple(arguments) => {
            arguments.iter().any(evaluated)
        }
        _ => false,
    }
}
impl Compiler<'_> {
    fn operand(
        &mut self,
        term: &Term,
        anonymous: bool,
        bind: bool,
        depth: usize,
    ) -> Result<Operand, Error> {
        self.node(depth)?;
        Ok(match term {
            Term::Variable(Variable::Anonymous) if anonymous => Operand::Any,
            Term::Variable(Variable::Anonymous) => {
                return Err(self.unsupported(Feature::UnsafeVariable));
            }
            Term::Variable(variable) => Operand::Variable(self.variable(variable)?),
            Term::Symbolic(symbol) => {
                let symbol = self.symbol(symbol, depth, true)?;
                Operand::Value(
                    crate::structural_value::from_symbol(&symbol)
                        .map_err(|_| self.unsupported(Feature::Comparison))?,
                )
            }
            Term::Function { name, arguments } => {
                self.operand_function(name, arguments, anonymous, bind, depth, Sign::Positive)?
            }
            Term::UnaryOperation {
                operator: UnaryOp::Negate,
                argument,
            } if matches!(argument.as_ref(), Term::Function { .. }) => {
                let Term::Function { name, arguments } = argument.as_ref() else {
                    unreachable!()
                };
                self.operand_function(name, arguments, anonymous, bind, depth + 1, Sign::Negative)?
            }
            Term::Tuple(arguments) => {
                self.arity(arguments.len())?;
                let mut values = Vec::new();
                for argument in arguments {
                    values.push(self.operand(argument, anonymous, bind, depth + 1)?);
                }
                Operand::Tuple(values)
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
        anonymous: bool,
        bind: bool,
        depth: usize,
        sign: Sign,
    ) -> Result<Operand, Error> {
        self.text(name.as_str())?;
        self.arity(arguments.len())?;
        let mut values = Vec::new();
        for argument in arguments {
            values.push(self.operand(argument, anonymous, bind, depth + 1)?);
        }
        Ok(Operand::Function(sign, name.clone(), values))
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
            terms.push(self.operand(argument, bind || atom.sign != Sign::Negative, bind, 1)?);
        }
        let predicate = Predicate::with_sign(
            atom.name.as_str(),
            terms.len(),
            crate::coherence::core_sign(atom.sign),
        )
        .map_err(|_| self.error(ErrorKind::InvalidSymbol))?;
        Ok(Pattern {
            predicate,
            evaluated: terms.iter().any(evaluated),
            terms,
        })
    }
    pub(super) fn patterns(&mut self, atom: &Atom) -> Result<Vec<Pattern>, Error> {
        let previous = self.pool_binding;
        self.pool_binding = matches!(atom.arguments, Arguments::Pooled(_));
        let result = (|| {
            let mut patterns = Vec::new();
            for arguments in atom.alternatives() {
                self.node(1)?;
                patterns.push(self.pattern_terms(atom, arguments, true)?);
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
                    .map(|(slot, term)| Binder::Assign(slot, term))
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
            Operand::Function(_, _, arguments) | Operand::Tuple(arguments) => arguments
                .iter()
                .all(|argument| self.operand_ready(argument, captures)),
            _ => true,
        }
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
