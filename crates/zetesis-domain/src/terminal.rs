//! Terminal positive definitions over one exact borrowed program.
//!
//! Selected predicates have only flat, normal positive producers and no rule or
//! constraint consumer anywhere in the input. All their producers are returned together.
//! Their positive bodies therefore mention only unselected predicates. This is
//! a structural certificate, not source admission, a grounding transformation,
//! or a proof that a reconstructed interpretation is an answer set.
//!
//! The scan includes ordinary, conditional, choice, disjunctive and aggregate
//! occurrences. A complementary strong-sign occurrence blocks selection. Show
//! and diagnostic declarations do not control eligibility; objectives and
//! projection make the whole result Unknown in this initial profile.
//!
//! Limits count inspected nodes, predicate-name/variable/symbol text, registered
//! signed signatures and argument positions, and submitted occurrence/reference
//! links. Only those fields of [`Limits`], plus its per-symbol bounds, apply;
//! finite-domain widths and fixed-point rounds do not. Standard collection
//! comparisons do not charge repeated text comparisons inside tree lookups;
//! text accounting measures the logical text submitted to each operation.
//! The classifier provides no cancellation callback. Collection
//! allocations are bounded by these logical populations, not by a fallible
//! physical-byte or RSS contract. Names and values remain borrowed. The scan
//! does not recurse through input terms; closed `Symbol` traversal uses a bounded
//! explicit stack. Unsupported source terms disqualify a producer without
//! evaluating it. Unknown and Stopped discard every selected definition.
//!
//! ```
//! use themelios_program::program::{Atom, Program, Rule};
//! use themelios_program::symbol::Name;
//! use zetesis_domain::{Limits, terminal};
//!
//! let atom = Atom::new(Name::new("completed").unwrap(), []);
//! let program = Program::of([Rule::fact(atom)]);
//! let result = terminal::analyze(&program, Limits::default());
//! assert_eq!(result.status(), terminal::Status::Complete);
//! assert!(result.belongs_to(&program));
//! assert_eq!(result.definitions().len(), 1);
//! ```

use std::collections::BTreeMap;

use themelios_program::program::{Atom, Program, Statement};
use themelios_program::provenance::WithProvenance;
use themelios_program::symbol::Sign;

use crate::limits::check;
use crate::{Context, Limits, Resource, Statistics, Stop};

mod flat;
mod scan;

/// Context outside this classifier's complete semantic-occurrence contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnknownReason {
    /// Unresolved or unsupported source context, using the domain crate's reason.
    Source(crate::UnknownReason),
    /// Objective dependencies are outside the initial terminal profile.
    Objectives,
    /// Explicit projection requires a separate full-answer correspondence.
    Projection,
}

/// Completion does not imply that any eligible terminal definition exists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// Every semantic occurrence was classified; selection may be empty.
    Complete,
    /// Unsupported context; no partial selection is published.
    Unknown(UnknownReason),
    /// An inclusive logical ceiling stopped the scan before its next operation.
    Stopped(Stop),
}

/// Selected original carriers, tied to the identical immutable input instance.
#[derive(Debug)]
pub struct Analysis<'p> {
    program: &'p Program,
    definitions: Vec<&'p WithProvenance<Statement>>,
    status: Status,
    context: Option<Context<'p>>,
    statistics: Statistics,
}

impl<'p> Analysis<'p> {
    /// The exact input; no normalization, source replay or copy is performed.
    #[must_use]
    pub fn program(&self) -> &'p Program {
        self.program
    }

    /// Whether `program` is this input instance, not merely equal content.
    #[must_use]
    pub fn belongs_to(&self, program: &Program) -> bool {
        std::ptr::eq(self.program, program)
    }

    /// Complete, unsupported and stopped results remain distinct.
    #[must_use]
    pub const fn status(&self) -> Status {
        self.status
    }

    /// Source context at the first global fallback; absent on completion.
    #[must_use]
    pub const fn context(&self) -> Option<Context<'p>> {
        self.context
    }

    /// Charged logical populations, including accepted work before a fallback.
    #[must_use]
    pub const fn statistics(&self) -> Statistics {
        self.statistics
    }

    /// Every selected normal rule carrier, in the input Program's iteration
    /// order, with its original provenance. Every producer of each selected
    /// signature occurs here. Upstream-coalesced rules remain one carrier with
    /// their combined provenance; this does not recreate textual occurrences.
    /// Empty after Unknown/Stopped. Source/IR correspondence is a caller duty.
    #[must_use]
    pub fn definitions(&self) -> &[&'p WithProvenance<Statement>] {
        &self.definitions
    }
}

/// Classify complete terminal positive definitions without interpreting source.
///
/// A selected rule has a positive atomic head, positive ordinary body atoms,
/// whole-variable or closed `Symbol` arguments, and a positive body binder for
/// every head variable. Source arithmetic, ranges, pools and constructors that
/// still need evaluation disqualify that producer. Facts and already closed
/// compound symbols are allowed. No filename, spelling or shown predicate has
/// special treatment. The result neither removes rules nor changes budgets.
#[must_use]
pub fn analyze(program: &Program, limits: Limits) -> Analysis<'_> {
    let mut engine = Engine {
        result: Analysis {
            program,
            definitions: Vec::new(),
            status: Status::Complete,
            context: None,
            statistics: Statistics::default(),
        },
        limits,
        signatures: BTreeMap::new(),
        candidates: Vec::new(),
    };
    match engine.scan() {
        Ok(()) => engine.result.context = None,
        Err(Failure::Unknown(reason)) => {
            engine.result.status = Status::Unknown(reason);
            engine.result.definitions.clear();
        }
        Err(Failure::Stopped(stop)) => {
            engine.result.status = Status::Stopped(stop);
            engine.result.definitions.clear();
        }
    }
    engine.result
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Signature<'p> {
    name: &'p str,
    arity: u32,
    sign: Sign,
}

struct Usage {
    read: bool,
    qualified: bool,
}

enum Failure {
    Unknown(UnknownReason),
    Stopped(Stop),
}
impl From<Stop> for Failure {
    fn from(stop: Stop) -> Self {
        Self::Stopped(stop)
    }
}

struct Engine<'p> {
    result: Analysis<'p>,
    limits: Limits,
    signatures: BTreeMap<Signature<'p>, Usage>,
    candidates: Vec<(Signature<'p>, &'p WithProvenance<Statement>)>,
}

impl<'p> Engine<'p> {
    fn work(&mut self) -> Result<(), Failure> {
        let observed = u128::from(self.result.statistics.work) + 1;
        check(Resource::Work, observed, u128::from(self.limits.max_work))?;
        self.result.statistics.work += 1;
        Ok(())
    }

    fn bytes(&mut self, count: usize) -> Result<(), Failure> {
        let observed = u128::from(self.result.statistics.inspected_bytes) + count as u128;
        check(
            Resource::InspectedBytes,
            observed,
            u128::from(self.limits.max_inspected_bytes),
        )?;
        self.result.statistics.inspected_bytes =
            u64::try_from(observed).expect("bounded by u64 limit");
        Ok(())
    }

    fn link(&mut self) -> Result<(), Failure> {
        let observed = self.result.statistics.links as u128 + 1;
        check(Resource::Links, observed, self.limits.max_links as u128)?;
        self.result.statistics.links += 1;
        Ok(())
    }

    fn signature(&mut self, atom: &'p Atom, arity: usize) -> Result<Signature<'p>, Failure> {
        self.work()?;
        self.bytes(atom.name.as_str().len())?;
        let arity = u32::try_from(arity)
            .map_err(|_| Failure::Unknown(UnknownReason::Source(crate::UnknownReason::Head)))?;
        let signature = Signature {
            name: atom.name.as_str(),
            arity,
            sign: atom.sign,
        };
        if !self.signatures.contains_key(&signature) {
            let predicates = self.result.statistics.predicates as u128 + 1;
            let positions = self.result.statistics.positions as u128 + u128::from(arity);
            check(
                Resource::Predicates,
                predicates,
                self.limits.max_predicates as u128,
            )?;
            check(
                Resource::Positions,
                positions,
                self.limits.max_positions as u128,
            )?;
            self.signatures.insert(
                signature,
                Usage {
                    read: false,
                    qualified: true,
                },
            );
            self.result.statistics.predicates += 1;
            self.result.statistics.positions =
                usize::try_from(positions).expect("bounded by usize limit");
        }
        Ok(signature)
    }
}
