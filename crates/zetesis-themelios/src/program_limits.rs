//! Bounded inspection of borrowed logical rules before relational compilation.
//!
//! The cursor borrows themelios values; it does not reconstruct an AST. It keeps
//! one remaining-siblings slice per open term or symbol, so retained traversal
//! space is O(depth), including for a wide argument list. Every visited node is
//! charged before its children are scheduled. Work is O(visited nodes); text is
//! charged by byte length before the compiler can scan or copy it.
//!
//! This is a resource check, not capability admission. Head, body and argument
//! shapes the shared compiler rejects without inspecting their children remain
//! opaque. Every term shape is traversed, including unsupported operators: the
//! compiler can evaluate a closed operand of unary negation, so its whole input
//! must be bounded before evaluation.

use std::fmt;

use themelios_program::program::{Arguments, Atom, BodyElement, Head, Literal, LiteralInner, Rule};
use themelios_program::symbol::Symbol;
use themelios_program::term::{Term, Variable};

/// The logical resource whose configured ceiling was exceeded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resource {
    /// Visited logical nodes. Formula preparation also counts directive,
    /// provenance and annotation structure.
    Nodes,
    /// Nesting of visited logical nodes, with each statement at depth one.
    Depth,
    /// Canonical elements in one body or, for formula preparation, condition.
    BodyElements,
    /// UTF-8 bytes in visited logical names and strings. Formula preparation
    /// additionally counts annotation text and transformation tags.
    TextBytes,
}

impl fmt::Display for Resource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Nodes => "logical nodes",
            Self::Depth => "logical depth",
            Self::BodyElements => "body elements",
            Self::TextBytes => "logical text bytes",
        })
    }
}

/// A logical resource refusal before compilation of the offending statement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limit {
    /// The resource being counted.
    pub resource: Resource,
    /// The configured maximum.
    pub limit: usize,
    /// The count at refusal, saturated at `usize::MAX` if addition overflowed.
    pub observed: usize,
}

impl fmt::Display for Limit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} limit {} exceeded (observed {})",
            self.resource, self.limit, self.observed
        )
    }
}

impl std::error::Error for Limit {}

#[derive(Clone, Copy)]
pub(crate) struct Limits {
    pub(crate) nodes: usize,
    pub(crate) depth: usize,
    pub(crate) body_elements: usize,
    pub(crate) text_bytes: usize,
}

/// Node and text counts are cumulative across calls; depth and body width are
/// checked separately for each rule. A failed check must abort admission.
pub(crate) struct Budget {
    limits: Limits,
    nodes: usize,
    text_bytes: usize,
}

impl Budget {
    pub(crate) const fn new(limits: Limits) -> Self {
        Self {
            limits,
            nodes: 0,
            text_bytes: 0,
        }
    }

    pub(crate) fn check_rule(&mut self, rule: &Rule) -> Result<(), Limit> {
        self.node(1)?;
        self.head(rule.head().get(), 2)?;
        self.node(2)?;
        for (index, element) in rule.body().get().elements().enumerate() {
            check(
                Resource::BodyElements,
                self.limits.body_elements,
                index.saturating_add(1),
            )?;
            self.node(3)?;
            if let BodyElement::Literal(literal) = element.get() {
                self.literal(literal, 4)?;
            }
        }
        Ok(())
    }

    pub(crate) fn node(&mut self, depth: usize) -> Result<(), Limit> {
        check(Resource::Depth, self.limits.depth, depth)?;
        charge(&mut self.nodes, 1, Resource::Nodes, self.limits.nodes)
    }

    pub(crate) fn text(&mut self, text: &str) -> Result<(), Limit> {
        charge(
            &mut self.text_bytes,
            text.len(),
            Resource::TextBytes,
            self.limits.text_bytes,
        )
    }

    /// Inspect bounded borrowed terms without a second recursive traversal.
    /// The callback runs after each term's node/depth charge and before children.
    pub(crate) fn inspect_term(
        &mut self,
        term: &Term,
        depth: usize,
        inspect: impl FnMut(&Term),
    ) -> Result<(), Limit> {
        self.values(Frame::Term(term, depth), inspect)
    }

    pub(crate) fn body_elements(&self, observed: usize) -> Result<(), Limit> {
        check(Resource::BodyElements, self.limits.body_elements, observed)
    }

    fn head(&mut self, head: &Head, depth: usize) -> Result<(), Limit> {
        self.node(depth)?;
        match head {
            Head::Literal(literal) => self.literal(literal, depth + 1),
            Head::Choice(choice) => {
                if choice.left_guard().is_some() || choice.right_guard().is_some() {
                    return Ok(());
                }
                let mut elements = choice.elements();
                let Some(element) = elements.next() else {
                    return Ok(());
                };
                if elements.next().is_some() || !element.get().condition().is_empty() {
                    return Ok(());
                }
                self.node(depth + 1)?; // The single choice element.
                self.node(depth + 2)?; // Its empty condition.
                self.literal(element.get().literal(), depth + 2)
            }
            _ => Ok(()),
        }
    }

    fn literal(&mut self, literal: &Literal, depth: usize) -> Result<(), Limit> {
        self.node(depth)?;
        match &literal.inner {
            LiteralInner::Atom(atom) => self.atom(atom.get(), depth + 1),
            LiteralInner::Comparison(comparison) => {
                self.node(depth + 1)?;
                let mut steps = comparison.get().steps();
                let (_, right) = steps.next().expect("a comparison has at least one step");
                if steps.next().is_some() {
                    return Ok(());
                }
                self.values(Frame::Term(comparison.get().first(), depth + 2), |_| {})?;
                self.values(Frame::Term(right, depth + 2), |_| {})
            }
            LiteralInner::True | LiteralInner::False => Ok(()),
        }
    }

    fn atom(&mut self, atom: &Atom, depth: usize) -> Result<(), Limit> {
        self.node(depth)?;
        self.text(atom.name.as_str())?;
        if let Arguments::Single(arguments) = &atom.arguments {
            self.node(depth + 1)?;
            self.values(Frame::Terms(arguments, depth + 2), |_| {})?;
        }
        Ok(())
    }

    fn values(&mut self, first: Frame<'_>, mut inspect: impl FnMut(&Term)) -> Result<(), Limit> {
        let mut pending = vec![first];
        while let Some(frame) = pending.pop() {
            match frame {
                Frame::Terms(terms, depth) => {
                    if let Some((first, rest)) = terms.split_first() {
                        pending.push(Frame::Terms(rest, depth));
                        pending.push(Frame::Term(first, depth));
                    }
                }
                Frame::Symbols(symbols, depth) => {
                    if let Some((first, rest)) = symbols.split_first() {
                        pending.push(Frame::Symbols(rest, depth));
                        pending.push(Frame::Symbol(first, depth));
                    }
                }
                Frame::Term(term, depth) => {
                    self.node(depth)?;
                    inspect(term);
                    let child_depth = depth.saturating_add(1);
                    match term {
                        Term::Variable(Variable::Named(name)) => self.text(name.as_str())?,
                        Term::Variable(Variable::Anonymous) => {}
                        Term::Symbolic(symbol) => {
                            pending.push(Frame::Symbol(symbol, child_depth));
                        }
                        Term::Function { name, arguments } | Term::External { name, arguments } => {
                            self.text(name.as_str())?;
                            pending.push(Frame::Terms(arguments, child_depth));
                        }
                        Term::Tuple(terms) | Term::Pool(terms) => {
                            pending.push(Frame::Terms(terms, child_depth));
                        }
                        Term::UnaryOperation { argument, .. } | Term::Absolute(argument) => {
                            pending.push(Frame::Term(argument, child_depth));
                        }
                        Term::BinaryOperation { left, right, .. }
                        | Term::Interval {
                            lower: left,
                            upper: right,
                        } => {
                            pending.push(Frame::Term(right, child_depth));
                            pending.push(Frame::Term(left, child_depth));
                        }
                    }
                }
                Frame::Symbol(symbol, depth) => {
                    self.node(depth)?;
                    let child_depth = depth.saturating_add(1);
                    match symbol {
                        Symbol::String(text) => self.text(text)?,
                        Symbol::Function {
                            name, arguments, ..
                        } => {
                            self.text(name.as_str())?;
                            pending.push(Frame::Symbols(arguments, child_depth));
                        }
                        Symbol::Tuple(symbols) => {
                            pending.push(Frame::Symbols(symbols, child_depth));
                        }
                        Symbol::Infimum | Symbol::Number(_) | Symbol::Supremum => {}
                    }
                }
            }
        }
        Ok(())
    }
}

/// A pending borrowed value, or the untouched suffix of one child sequence.
/// Sibling frames do not count as logical nodes or increase logical depth.
enum Frame<'a> {
    Term(&'a Term, usize),
    Symbol(&'a Symbol, usize),
    Terms(&'a [Term], usize),
    Symbols(&'a [Symbol], usize),
}

fn check(resource: Resource, limit: usize, observed: usize) -> Result<(), Limit> {
    if observed > limit {
        Err(Limit {
            resource,
            limit,
            observed,
        })
    } else {
        Ok(())
    }
}

fn charge(used: &mut usize, amount: usize, resource: Resource, limit: usize) -> Result<(), Limit> {
    let observed = used.checked_add(amount).ok_or(Limit {
        resource,
        limit,
        observed: usize::MAX,
    })?;
    check(resource, limit, observed)?;
    *used = observed;
    Ok(())
}
