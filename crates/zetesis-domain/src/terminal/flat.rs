//! The producer profile is separate from complete semantic occurrence scanning.

use std::collections::BTreeSet;

use themelios_program::program::{
    Arguments, BodyElement, DefaultNegation, Head, LiteralInner, Rule,
};
use themelios_program::symbol::{Sign, Symbol, VarName};
use themelios_program::term::{Term, Variable};

use super::{Engine, Failure};

impl<'p> Engine<'p> {
    pub(super) fn flat(&mut self, rule: &'p Rule) -> Result<bool, Failure> {
        self.work()?;
        let Head::Literal(head) = rule.head().get() else {
            return Ok(false);
        };
        let LiteralInner::Atom(atom) = &head.inner else {
            return Ok(false);
        };
        if head.negation != DefaultNegation::None || atom.get().sign != Sign::Positive {
            return Ok(false);
        }
        let Arguments::Single(head_terms) = &atom.get().arguments else {
            return Ok(false);
        };
        let mut bound = BTreeSet::new();
        for element in rule.body().get().elements() {
            self.work()?;
            let BodyElement::Literal(literal) = element.get() else {
                return Ok(false);
            };
            if literal.negation != DefaultNegation::None {
                return Ok(false);
            }
            let LiteralInner::Atom(atom) = &literal.inner else {
                return Ok(false);
            };
            let Arguments::Single(terms) = &atom.get().arguments else {
                return Ok(false);
            };
            for term in terms {
                if !self.argument(term, &mut bound)? {
                    return Ok(false);
                }
            }
        }
        for term in head_terms {
            self.work()?;
            match term {
                Term::Symbolic(symbol) if self.closed_symbol(symbol)? => {}
                Term::Variable(Variable::Named(name)) => {
                    self.bytes(name.as_str().len())?;
                    if !bound.contains(name) {
                        return Ok(false);
                    }
                }
                _ => return Ok(false),
            }
        }
        Ok(true)
    }

    fn argument(
        &mut self,
        term: &'p Term,
        bound: &mut BTreeSet<&'p VarName>,
    ) -> Result<bool, Failure> {
        self.work()?;
        match term {
            Term::Symbolic(symbol) => self.closed_symbol(symbol),
            Term::Variable(Variable::Named(name)) => {
                self.bytes(name.as_str().len())?;
                self.link()?;
                bound.insert(name);
                Ok(true)
            }
            Term::Variable(Variable::Anonymous) => Ok(true),
            _ => Ok(false),
        }
    }

    fn closed_symbol(&mut self, symbol: &Symbol) -> Result<bool, Failure> {
        if self.limits.max_symbol_nodes == 0 || self.limits.max_symbol_depth == 0 {
            return Ok(false);
        }
        let mut pending = vec![(symbol, 1_usize)];
        let mut nodes = 0_usize;
        let mut bytes = 0_u128;
        while let Some((symbol, depth)) = pending.pop() {
            self.work()?;
            nodes += 1;
            let text = match symbol {
                Symbol::String(text) => text.len(),
                Symbol::Function { name, .. } => name.as_str().len(),
                _ => 0,
            };
            self.bytes(text)?;
            bytes += text as u128;
            if bytes > u128::from(self.limits.max_symbol_bytes) {
                return Ok(false);
            }
            let (Symbol::Function {
                arguments: children,
                ..
            }
            | Symbol::Tuple(children)) = symbol
            else {
                continue;
            };
            if children.is_empty() {
                continue;
            }
            if depth >= self.limits.max_symbol_depth
                || children.len() as u128 + pending.len() as u128 + nodes as u128
                    > self.limits.max_symbol_nodes as u128
            {
                return Ok(false);
            }
            // The population check precedes copying child references into scratch.
            for child in children {
                self.work()?;
                pending.push((child, depth + 1));
            }
        }
        Ok(true)
    }
}
