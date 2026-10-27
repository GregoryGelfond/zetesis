//! Bounded borrowed-term inspection shared by logical formula validation.
//!
//! The cursor retains one remaining-siblings slice per open term or symbol,
//! using O(depth) space. Node, depth and text charges precede child traversal.

use std::fmt;

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
