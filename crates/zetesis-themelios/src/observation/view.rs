//! Borrowed semantic models and bounded JSON views; no solver or output stream.

use std::fmt;

use themelios_program::symbol::{Sign as SymbolSign, Symbol};
use zetesis_core::catalog::{AtomRef, TermRef};
use zetesis_core::{Model, Sign, ValueNodeRef};
use zetesis_cpu::{Cancellation, Stop};
use zetesis_objective::Score;

use super::json::AtomTable;
use super::{ConstructionLimits, Error, Evaluation, Limits, ObservationProgram, Statistics};
use crate::OutputSelection;

/// A full supplied model beside its independent observation and objective channels.
/// Construction evaluates observations only; the caller establishes stability and
/// the score. It does not clone, project, or deduplicate the supplied model, or
/// invoke a solver. The evaluated, deduplicated term channel is owned by the view;
/// the full model, atom selection, and optional score are borrowed.
pub struct ModelView<'a> {
    model: &'a Model,
    selection: &'a OutputSelection,
    terms: Evaluation,
    score: Option<&'a Score>,
}
impl ObservationProgram {
    /// Evaluate a view over a supplied full model and optional already computed score.
    ///
    /// # Cost
    /// An empty observation program takes constant time and allocates no term
    /// storage. Otherwise, construction performs one observation evaluation: its
    /// relational binding enumeration, term construction, and sorted term
    /// deduplication determine the cost under `limits`. These operations can cost
    /// more than a scan of the supplied model. The view retains the evaluated terms
    /// and their statistics; JSON encoding is a separate, later operation.
    ///
    /// # Errors
    /// Returns the observation evaluator's typed refusal without a partial view.
    pub fn view<'a>(
        &self,
        model: &'a Model,
        selection: &'a OutputSelection,
        score: Option<&'a Score>,
        limits: Limits,
        cancellation: &Cancellation,
    ) -> Result<ModelView<'a>, Error> {
        self.view_with_construction_limits(
            model,
            selection,
            score,
            limits,
            ConstructionLimits::default(),
            cancellation,
        )
    }

    /// Construct a view with an independent observation-construction ceiling.
    /// JSON encoding remains a separate operation with its own accounting.
    ///
    /// # Errors
    /// Returns the same typed refusals as observation evaluation.
    pub fn view_with_construction_limits<'a>(
        &self,
        model: &'a Model,
        selection: &'a OutputSelection,
        score: Option<&'a Score>,
        limits: Limits,
        construction: ConstructionLimits,
        cancellation: &Cancellation,
    ) -> Result<ModelView<'a>, Error> {
        let terms = if self.is_empty() {
            Evaluation {
                symbols: Vec::new(),
                statistics: Statistics::default(),
            }
        } else {
            self.evaluate_with_construction_limits(model, limits, construction, cancellation)?
        };
        Ok(ModelView {
            model,
            selection,
            terms,
            score,
        })
    }
}
impl ModelView<'_> {
    /// Spell selected atoms and the already evaluated term channel as one line.
    /// Equal symbols in the two channels remain repeated. The full model is not
    /// changed, and observation expressions are not evaluated again.
    ///
    /// Rendering resumes the view's observation work counters under `limits`;
    /// the returned statistics include evaluation and this spelling pass. Space
    /// is one bounded UTF-8 line plus the existing explicit symbol cursor.
    ///
    /// # Errors
    /// Returns the same spelling, resource and control refusals as
    /// [`ObservationProgram::render`], without exposing a partial line.
    pub fn render(
        &self,
        limits: Limits,
        cancellation: &Cancellation,
    ) -> Result<super::Rendered, Error> {
        super::render::render_evaluated(
            self.model,
            self.selection,
            self.shown_terms(),
            super::evaluate::Work {
                limits,
                construction: ConstructionLimits::default(),
                cancellation,
                statistics: self.terms.statistics(),
                local_bytes: 0,
                location: None,
            },
        )
    }

    /// Complete semantic identity, unaffected by display directives.
    #[must_use]
    pub const fn model(&self) -> &Model {
        self.model
    }

    /// Selected original atoms in full-model order; no term-channel deduplication.
    /// Constructing the iterator takes constant time and allocates nothing. A full
    /// traversal scans all atoms and, for explicit selection, performs a binary
    /// lookup per atom; predicate comparison also inspects name bytes.
    pub fn shown_atoms(&self) -> impl Iterator<Item = AtomRef<'_>> {
        self.model
            .atoms()
            .iter()
            .filter(|atom| self.selection.includes(*atom))
    }

    /// Distinct enabled terms. Equal symbols in the atom channel remain separate.
    #[must_use]
    pub fn shown_terms(&self) -> &[Symbol] {
        self.terms.symbols()
    }

    /// The caller's already evaluated score; absence differs from an empty score.
    #[must_use]
    pub const fn score(&self) -> Option<&Score> {
        self.score
    }

    /// Observation work, independent of JSON view work.
    #[must_use]
    pub fn statistics(&self) -> Statistics {
        self.observation_statistics()
    }

    /// Work performed while evaluating observations; excludes subsequent encoding.
    #[must_use]
    pub fn observation_statistics(&self) -> Statistics {
        self.terms.statistics()
    }

    /// Encode the model as a record of a document, without writing external
    /// bytes: the atoms the document has not spelled are spelled and entered
    /// into `table`, and every atom of the model, shown or not, is referred
    /// to by its index in the table.
    ///
    /// A spelled atom is a typed name/sign/argument record whose terms use a
    /// flat preorder node sequence: constructor arities determine their
    /// children, and scalars have one node. Shown atom indices address the
    /// table, and shown terms form their own channel, which preserves
    /// identity without parsing ASP text. Costs retain descending
    /// priority/value pairs, or `null` when absent.
    ///
    /// # Cost and space
    /// Encoding looks every atom of the model up in the table once and
    /// traverses the value nodes of the atoms it spells, every shown term
    /// node and every cost entry, together with their emitted UTF-8 bytes.
    /// Signature selection uses the same binary lookup as
    /// [`OutputSelection::includes`], with at most `floor(log2(S)) + 1`
    /// comparisons per atom for nonempty `S` signatures. Each probe charges
    /// one unit plus both predicate-name byte lengths. Observation
    /// evaluation has already occurred and is not repeated here.
    ///
    /// The returned record retains `B` bytes, bounded by `max_bytes`. Encoding
    /// additionally holds a cursor of at most `D` frames for one shown term, bounded
    /// by `max_depth`; auxiliary storage is `O(B + D)`, excluding the already owned
    /// terms, borrowed inputs, and allocator overhead. Record growth is geometric.
    /// Cursor growth reserves one frame at a time, so a term reaching depth `D`
    /// can incur `O(D^2)` cumulative frame copies when allocations move storage.
    /// That allocation cost is separate from the traversal/byte/comparison work
    /// charged by `max_work`; the complete operation is not uniformly linear in `B`.
    ///
    /// # Errors
    /// Refuses before exceeding the record, work or traversal-depth ceilings,
    /// and with [`ViewError::Table`] when the table would exceed its ceiling;
    /// a refused record enters no atom, and allocation/control failures
    /// return no partial JSON value.
    pub fn record(
        &self,
        table: &mut AtomTable,
        limits: ViewLimits,
        cancellation: &Cancellation,
    ) -> Result<String, ViewError> {
        self.encode_record(table, limits, cancellation)
            .map(super::json::Encoded::into_text)
            .map_err(|failure| failure.cause())
    }

    /// Encode a document record with retained work accounting; the data is
    /// byte-identical to [`Self::record`]. The returned statistics exclude
    /// observation evaluation and external writes. A failure contains
    /// accounting for the discarded private prefix, never that prefix itself.
    ///
    /// # Errors
    /// Returns the same causes as [`Self::record`], with work charged before
    /// refusal.
    pub fn encode_record(
        &self,
        table: &mut AtomTable,
        limits: super::json::Limits,
        cancellation: &Cancellation,
    ) -> Result<super::json::Encoded, super::json::Failure> {
        let mut out = Buffer::new(limits, cancellation);
        // The atoms this record spells are entered as it spells them, so a
        // lookup is one hash probe; a refused record withdraws its entries.
        let mut added = Vec::new();
        let mut deferred = false;
        let result = self.encode_record_into(&mut out, table, &mut added, &mut deferred);
        let statistics = super::json::Statistics {
            work: out.work,
            buffered_bytes: out.text.len(),
        };
        match result {
            Ok(()) => Ok(super::json::Encoded::new(out.text, statistics)),
            Err(cause) => {
                table.retract(&added, deferred);
                Err(super::json::Failure::new(cause, statistics))
            }
        }
    }

    /// `deferred` says whether this record was the document's first, whose
    /// whole model the table defers.
    fn encode_record_into<'m>(
        &'m self,
        out: &mut Buffer<'_>,
        table: &mut AtomTable,
        added: &mut Vec<AtomRef<'m>>,
        deferred: &mut bool,
    ) -> Result<(), ViewError> {
        // One lookup per atom: the index of each atom of the model, in model
        // order, serves the spelling pass and both index lists. The first
        // record of a document spells every atom and is deferred whole.
        let mut indices = Vec::new();
        indices
            .try_reserve_exact(self.model.atoms().len())
            .map_err(|_| ViewError::Allocation)?;
        out.text("{\"atoms\":[")?;
        if table.is_empty() {
            for (position, atom) in self.model.atoms().iter().enumerate() {
                if position != 0 {
                    out.text(",")?;
                }
                out.atom(atom)?;
                added.try_reserve(1).map_err(|_| ViewError::Allocation)?;
                added.push(atom);
                indices.push(position);
            }
            table.defer(self.model)?;
            *deferred = true;
        } else {
            for (position, atom) in self.model.atoms().iter().enumerate() {
                let index = if let Some(index) = table.index(atom)? {
                    index
                } else {
                    if !added.is_empty() {
                        out.text(",")?;
                    }
                    out.atom(atom)?;
                    added.try_reserve(1).map_err(|_| ViewError::Allocation)?;
                    let index = table.enter(self.model, position)?;
                    added.push(atom);
                    index
                };
                indices.push(index);
            }
        }
        out.text("],\"full_model\":[")?;
        for (position, &index) in indices.iter().enumerate() {
            if position != 0 {
                out.text(",")?;
            }
            out.number(index)?;
        }
        out.text("],\"shown\":{\"atom_indices\":[")?;
        let mut first = true;
        for (atom, &index) in self.model.atoms().iter().zip(&indices) {
            if self.selection.try_includes(atom, |units| out.step(units))? {
                if !first {
                    out.text(",")?;
                }
                first = false;
                out.number(index)?;
            }
        }
        out.text("],\"terms\":[")?;
        self.encode_terms_and_costs(out)
    }

    /// The shown terms and the costs, closing the shown object and the value.
    fn encode_terms_and_costs(&self, out: &mut Buffer<'_>) -> Result<(), ViewError> {
        for (index, symbol) in self.shown_terms().iter().enumerate() {
            if index != 0 {
                out.text(",")?;
            }
            out.symbol(symbol)?;
        }
        out.text("]},\"costs\":")?;
        if let Some(score) = self.score {
            out.text("[")?;
            for (index, (priority, value)) in score.costs().iter().enumerate() {
                if index != 0 {
                    out.text(",")?;
                }
                out.text("{\"priority\":")?;
                out.text(&priority.to_string())?;
                out.text(",\"value\":")?;
                out.text(&value.to_string())?;
                out.text("}")?;
            }
            out.text("]")?;
        } else {
            out.text("null")?;
        }
        out.text("}")?;
        Ok(())
    }
}

/// Independent bounds for JSON encoding, excluding allocator overhead.
/// Also available as [`super::json::Limits`].
#[derive(Clone, Copy, Debug)]
pub struct ViewLimits {
    /// Maximum retained UTF-8 JSON bytes; zero means zero.
    pub max_bytes: usize,
    /// Maximum charged output bytes, node visits and selection comparisons.
    pub max_work: u64,
    /// Maximum symbol traversal depth; bounds the explicit cursor stack.
    pub max_depth: usize,
}
impl Default for ViewLimits {
    fn default() -> Self {
        Self {
            max_bytes: 8_388_608,
            max_work: 100_000_000,
            max_depth: 128,
        }
    }
}

/// JSON encoding failed without returning partial JSON.
/// Also available as [`super::json::Error`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewError {
    /// Complete view would exceed the configured retained-byte ceiling.
    Bytes,
    /// Charged traversal/encoding work would exceed its ceiling.
    Work,
    /// Explicit traversal stack would exceed the depth ceiling.
    Depth,
    /// Fallible storage reservation failed.
    Allocation,
    /// The document's atom table would exceed its ceiling of distinct atoms.
    Table,
    /// Caller cancellation or deadline stopped the view.
    Stopped(Stop),
}
impl fmt::Display for ViewError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "model JSON view refused: {self:?}")
    }
}
impl std::error::Error for ViewError {}

fn sign(value: Sign) -> &'static str {
    match value {
        Sign::Positive => "positive",
        Sign::Negative => "negative",
    }
}

struct Buffer<'a> {
    text: String,
    limits: ViewLimits,
    cancellation: &'a Cancellation,
    work: u64,
}
impl<'a> Buffer<'a> {
    fn new(limits: ViewLimits, cancellation: &'a Cancellation) -> Self {
        Self {
            text: String::new(),
            limits,
            cancellation,
            work: 0,
        }
    }
    fn step(&mut self, count: u128) -> Result<(), ViewError> {
        self.cancellation.poll().map_err(ViewError::Stopped)?;
        let work = u128::from(self.work) + count;
        if work > u128::from(self.limits.max_work) {
            return Err(ViewError::Work);
        }
        self.work = u64::try_from(work).expect("checked u64 work ceiling");
        Ok(())
    }
    /// A typed atom: its predicate, sign and arguments.
    fn atom(&mut self, atom: AtomRef<'_>) -> Result<(), ViewError> {
        self.text("{\"predicate\":")?;
        self.quoted(atom.predicate().name())?;
        self.text(",\"sign\":")?;
        self.quoted(sign(atom.predicate().sign()))?;
        self.text(",\"arguments\":[")?;
        for (index, value) in atom.values().iter().enumerate() {
            if index != 0 {
                self.text(",")?;
            }
            self.value(value)?;
        }
        self.text("]}")
    }
    /// A decimal index, formatted without an allocation.
    fn number(&mut self, value: usize) -> Result<(), ViewError> {
        let mut digits = [0_u8; 20];
        let mut end = digits.len();
        let mut rest = value;
        loop {
            end -= 1;
            digits[end] = b'0' + u8::try_from(rest % 10).expect("a digit");
            rest /= 10;
            if rest == 0 {
                break;
            }
        }
        self.text(std::str::from_utf8(&digits[end..]).expect("ASCII digits"))
    }
    fn text(&mut self, text: &str) -> Result<(), ViewError> {
        self.step(text.len() as u128 + 1)?;
        let needed = self
            .text
            .len()
            .checked_add(text.len())
            .ok_or(ViewError::Bytes)?;
        if needed > self.limits.max_bytes {
            return Err(ViewError::Bytes);
        }
        if needed > self.text.capacity() {
            let capacity = needed
                .max(self.text.capacity().saturating_mul(2))
                .min(self.limits.max_bytes);
            self.text
                .try_reserve_exact(capacity - self.text.len())
                .map_err(|_| ViewError::Allocation)?;
        }
        self.text.push_str(text);
        Ok(())
    }
    fn quoted(&mut self, text: &str) -> Result<(), ViewError> {
        self.text("\"")?;
        for character in text.chars() {
            match character {
                '"' => self.text("\\\"")?,
                '\\' => self.text("\\\\")?,
                '\u{0000}'..='\u{001f}' => {
                    const HEX: &[u8; 16] = b"0123456789abcdef";
                    let byte = u8::try_from(u32::from(character)).expect("ASCII control character");
                    let escape = [
                        b'\\',
                        b'u',
                        b'0',
                        b'0',
                        HEX[usize::from(byte >> 4)],
                        HEX[usize::from(byte & 15)],
                    ];
                    self.text(std::str::from_utf8(&escape).expect("ASCII hexadecimal escape"))?;
                }
                other => self.text(other.encode_utf8(&mut [0; 4]))?,
            }
        }
        self.text("\"")
    }
    fn leaf(
        &mut self,
        kind: &str,
        value: Option<&str>,
        number: Option<i32>,
    ) -> Result<(), ViewError> {
        self.text("{\"kind\":")?;
        self.quoted(kind)?;
        if let Some(value) = value {
            self.text(",\"value\":")?;
            self.quoted(value)?;
        }
        if let Some(value) = number {
            self.text(",\"value\":")?;
            self.text(&value.to_string())?;
        }
        self.text("}")
    }
    fn constructor(
        &mut self,
        name: Option<&str>,
        negative: bool,
        arity: usize,
    ) -> Result<(), ViewError> {
        if let Some(name) = name {
            if arity == 0 && !negative {
                return self.leaf("symbol", Some(name), None);
            }
            self.text("{\"kind\":\"function\",\"name\":")?;
            self.quoted(name)?;
            self.text(",\"sign\":")?;
            self.quoted(if negative { "negative" } else { "positive" })?;
        } else {
            self.text("{\"kind\":\"tuple\"")?;
        }
        self.text(",\"arity\":")?;
        self.text(&arity.to_string())?;
        self.text("}")
    }
    fn node(&mut self, node: ValueNodeRef<'_>) -> Result<(), ViewError> {
        match node {
            ValueNodeRef::Infimum => self.leaf("infimum", None, None),
            ValueNodeRef::Supremum => self.leaf("supremum", None, None),
            ValueNodeRef::Number(value) => self.leaf("number", None, Some(value)),
            ValueNodeRef::String(value) => self.leaf("string", Some(value), None),
            ValueNodeRef::Symbol(value) => self.leaf("symbol", Some(value), None),
            ValueNodeRef::Function { name, sign, arity } => {
                self.constructor(Some(name), sign == Sign::Negative, arity)
            }
            ValueNodeRef::Tuple { arity } => self.constructor(None, false, arity),
        }
    }
    fn value(&mut self, value: TermRef<'_>) -> Result<(), ViewError> {
        if value.depth() > self.limits.max_depth {
            return Err(ViewError::Depth);
        }
        self.text("[")?;
        let mut nodes = value.nodes();
        let mut first = true;
        while let Some(node) = nodes.next_with(|| self.step(1))? {
            if !first {
                self.text(",")?;
            }
            first = false;
            self.node(node)?;
        }
        self.text("]")
    }
    fn symbol(&mut self, symbol: &Symbol) -> Result<(), ViewError> {
        // Each frame holds an ancestor's children and the first child not yet
        // entered. Keeping this continuation on the bounded heap cursor makes
        // traversal depth independent of the Rust call stack.
        let mut frames: Vec<(&[Symbol], usize)> = Vec::new();
        let mut current = symbol;
        self.text("[")?;
        // Invariant: current is the next unencoded preorder node; frames are its
        // ancestors, so its depth is frames.len() + 1. This array's output prefix
        // contains exactly the preceding nodes. Each iteration encodes one unvisited
        // node of the finite Symbol tree, or returns a typed refusal.
        loop {
            if frames.len() >= self.limits.max_depth {
                return Err(ViewError::Depth);
            }
            let children = match current {
                Symbol::Infimum => {
                    self.leaf("infimum", None, None)?;
                    None
                }
                Symbol::Supremum => {
                    self.leaf("supremum", None, None)?;
                    None
                }
                Symbol::Number(value) => {
                    self.leaf("number", None, Some(*value))?;
                    None
                }
                Symbol::String(value) => {
                    self.leaf("string", Some(value), None)?;
                    None
                }
                Symbol::Function {
                    name,
                    sign,
                    arguments,
                } => {
                    self.constructor(
                        Some(name.as_str()),
                        *sign == SymbolSign::Negative,
                        arguments.len(),
                    )?;
                    Some(arguments.as_slice())
                }
                Symbol::Tuple(arguments) => {
                    self.constructor(None, false, arguments.len())?;
                    Some(arguments.as_slice())
                }
            };
            if let Some(arguments) = children.filter(|args| !args.is_empty()) {
                frames
                    .try_reserve_exact(1)
                    .map_err(|_| ViewError::Allocation)?;
                frames.push((arguments, 1));
                current = &arguments[0];
                self.text(",")?;
                continue;
            }
            // The current subtree is complete. In the top frame, children before
            // next are complete. An iteration either selects the next unvisited
            // child or pops an exhausted ancestor; thus ascent terminates. With
            // no ancestor left, every node was emitted once and the array closes.
            loop {
                let Some((arguments, next)) = frames.last_mut() else {
                    return self.text("]");
                };
                if let Some(argument) = arguments.get(*next) {
                    *next += 1;
                    current = argument;
                    self.text(",")?;
                    break;
                }
                frames.pop();
            }
        }
    }
}
