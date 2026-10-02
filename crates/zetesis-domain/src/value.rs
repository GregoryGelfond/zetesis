//! Borrowed result identity, source evidence, and the finite-set/Unknown lattice.

use std::collections::{BTreeMap, BTreeSet};

use themelios_program::program::{Part, Program, Statement};
use themelios_program::provenance::WithProvenance;
use themelios_program::symbol::{Signature, Symbol};

use crate::{Statistics, Stop};

/// A conservative upper domain, sharing symbols with the original program.
#[derive(Debug, PartialEq, Eq)]
pub enum Domain<'p> {
    /// Every symbol is possible; absence of a finite bound is not emptiness.
    Unknown,
    /// Only these values are possible. An empty set is a genuine upper bound.
    Finite(BTreeSet<&'p Symbol>),
}
impl Domain<'_> {
    /// Whether the upper bound permits this value; Unknown always permits it.
    #[must_use]
    pub fn permits(&self, value: &Symbol) -> bool {
        match self {
            Self::Unknown => true,
            Self::Finite(values) => values.contains(value),
        }
    }
}

/// Why one argument lost precision without stopping the entire fixed point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Widening {
    /// A choice/disjunction or default-negated head is conservatively unrestricted.
    HeadForm,
    /// A generative, external, pooled, or otherwise unsupported head term.
    HeadTerm,
    /// No ordinary positive whole-variable body position binds this variable.
    UnboundVariable,
    /// A borrowed symbol exceeds its structural/payload admission bounds.
    SymbolSize,
    /// The finite set exceeds its per-argument width.
    ValueWidth,
    /// Every positive body position binding this producer's variable is Unknown.
    Dependency,
}

/// One argument domain and direct enclosing producer provenance.
#[derive(Debug)]
pub struct Argument<'p> {
    pub(super) domain: Domain<'p>,
    pub(super) producers: Vec<&'p WithProvenance<Statement>>,
    pub(super) widening: Option<Widening>,
}
impl<'p> Argument<'p> {
    /// The completed upper bound for this argument.
    #[must_use]
    pub fn domain(&self) -> &Domain<'p> {
        &self.domain
    }
    /// Exact original enclosing statements that directly produce this position.
    /// These retain all parsed/constructed origins; they are not a transitive proof.
    #[must_use]
    pub fn producers(&self) -> &[&'p WithProvenance<Statement>] {
        &self.producers
    }
    /// The first reason this argument widened, if any.
    #[must_use]
    pub fn widening(&self) -> Option<Widening> {
        self.widening
    }
}

/// Exact borrowed source context for a whole-analysis fallback.
#[derive(Clone, Copy, Debug)]
pub enum Context<'p> {
    /// A named or parameterized part.
    Part(&'p Part),
    /// An original statement, preserving its complete provenance carrier.
    Statement(&'p WithProvenance<Statement>),
}

/// Context that cannot be resolved by a standalone structural analysis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnknownReason {
    /// A non-base or parameterized part needs an activation/parameter context.
    ProgramPart,
    /// Constants have not been substituted by this analysis.
    Constants,
    /// Include content is not present as a certified complete input.
    Include,
    /// External atoms require an external-assignment context.
    External,
    /// Theory definitions or heads require a semantic extension.
    Theory,
    /// Script execution is outside this analysis.
    Script,
    /// A head's complete ordinary producers cannot be established in this slice.
    Head,
    /// An unrecognized future statement family cannot be silently ignored.
    Statement,
}

/// Abstract convergence and fallback are distinct from language admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// The conservative transfer system reached a fixed point, possibly Unknown.
    FixedPoint,
    /// Unresolved source context makes every query Unknown.
    Unknown(UnknownReason),
    /// A global resource stop makes every query Unknown; no partial finite map escapes.
    Stopped(Stop),
}

/// Argument bounds tied by lifetime and identity to one exact immutable Program.
#[derive(Debug)]
pub struct Analysis<'p> {
    pub(super) program: &'p Program,
    pub(super) signatures: BTreeMap<Signature, usize>,
    pub(super) arguments: Vec<Argument<'p>>,
    pub(super) status: Status,
    pub(super) context: Option<Context<'p>>,
    pub(super) statistics: Statistics,
}
impl<'p> Analysis<'p> {
    /// The exact borrowed input; no normalized copy or reparsed projection exists.
    #[must_use]
    pub fn program(&self) -> &'p Program {
        self.program
    }
    /// Whether the caller holds the identical Program instance, not merely equal content.
    #[must_use]
    pub fn belongs_to(&self, program: &Program) -> bool {
        std::ptr::eq(self.program, program)
    }
    /// Abstract completion/fallback status.
    #[must_use]
    pub fn status(&self) -> Status {
        self.status
    }
    /// The exact source context at a global fallback, where available.
    #[must_use]
    pub fn context(&self) -> Option<Context<'p>> {
        self.context
    }
    /// Committed logical accounting, including work before a global fallback.
    #[must_use]
    pub fn statistics(&self) -> Statistics {
        self.statistics
    }
    /// Query one signed predicate argument. Unknown signatures and invalid indices
    /// return Unknown; callers must not infer a closed world from lookup absence.
    #[must_use]
    pub fn domain(&self, signature: &Signature, index: usize) -> &Domain<'p> {
        self.argument(signature, index)
            .map_or(&Domain::Unknown, Argument::domain)
    }
    /// Retained argument/provenance entry, absent after a global fallback.
    #[must_use]
    pub fn argument(&self, signature: &Signature, index: usize) -> Option<&Argument<'p>> {
        if index >= signature.arity as usize {
            return None;
        }
        self.signatures
            .get(signature)
            .and_then(|start| self.arguments.get(start + index))
    }
    /// Every retained signed argument position, without allocating a new registry.
    pub fn arguments(&self) -> impl Iterator<Item = (&Signature, usize, &Argument<'p>)> {
        self.signatures.iter().flat_map(move |(signature, start)| {
            (0..signature.arity as usize)
                .map(move |index| (signature, index, &self.arguments[start + index]))
        })
    }
}
